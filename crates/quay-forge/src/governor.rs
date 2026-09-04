use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use quay_core::Priority;
use reqwest::{Client, Method, StatusCode};
use tokio::sync::Semaphore;

use crate::backoff::Backoff;
use crate::budget::SlidingBudget;
use crate::error::ForgeError;
use crate::locks::DevLocks;
use crate::token::Token;

const DEGRADED_QUOTA_FRACTION: f64 = 0.20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    RestRead,
    RestWrite,
    GraphQlQuery,
    GraphQlMutation,
}

impl RequestKind {
    pub fn points(self) -> u32 {
        match self {
            RequestKind::RestRead | RequestKind::GraphQlQuery => 1,
            RequestKind::RestWrite | RequestKind::GraphQlMutation => 5,
        }
    }

    pub fn is_write(self) -> bool {
        matches!(self, RequestKind::RestWrite | RequestKind::GraphQlMutation)
    }

    pub fn label(self) -> &'static str {
        match self {
            RequestKind::RestRead => "rest read",
            RequestKind::RestWrite => "rest write",
            RequestKind::GraphQlQuery => "graphql query",
            RequestKind::GraphQlMutation => "graphql mutation",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OutboundRequest {
    pub kind: RequestKind,
    pub method: Method,
    pub url: String,
    pub body: Option<String>,
    pub repo: Option<String>,
    pub priority: Priority,
}

impl OutboundRequest {
    pub fn rest_read(url: impl Into<String>, priority: Priority) -> Self {
        Self {
            kind: RequestKind::RestRead,
            method: Method::GET,
            url: url.into(),
            body: None,
            repo: None,
            priority,
        }
    }

    pub fn graphql_query(
        url: impl Into<String>,
        body: impl Into<String>,
        priority: Priority,
    ) -> Self {
        Self {
            kind: RequestKind::GraphQlQuery,
            method: Method::POST,
            url: url.into(),
            body: Some(body.into()),
            repo: None,
            priority,
        }
    }

    pub fn rest_write(
        url: impl Into<String>,
        body: impl Into<String>,
        repo: impl Into<String>,
        priority: Priority,
    ) -> Self {
        Self {
            kind: RequestKind::RestWrite,
            method: Method::POST,
            url: url.into(),
            body: Some(body.into()),
            repo: Some(repo.into()),
            priority,
        }
    }

    pub fn graphql_mutation(
        url: impl Into<String>,
        body: impl Into<String>,
        repo: impl Into<String>,
        priority: Priority,
    ) -> Self {
        Self {
            kind: RequestKind::GraphQlMutation,
            method: Method::POST,
            url: url.into(),
            body: Some(body.into()),
            repo: Some(repo.into()),
            priority,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Healthy,
    Degraded,
    Throttled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitSnapshot {
    pub limit: u32,
    pub remaining: u32,
    pub reset_at: u64,
}

impl RateLimitSnapshot {
    pub fn is_degraded(&self) -> bool {
        self.limit > 0
            && f64::from(self.remaining) < f64::from(self.limit) * DEGRADED_QUOTA_FRACTION
    }
}

#[derive(Debug)]
pub struct ForgeResponse {
    pub status: u16,
    pub body: String,
    pub bytes: usize,
    pub attempts: u32,
    pub elapsed: Duration,
    pub rate_limit: Option<RateLimitSnapshot>,
    pub points: u32,
}

pub struct GovernorConfig {
    pub max_concurrent_requests: usize,
    pub hourly_request_budget: u32,
    pub request_timeout: Duration,
    pub max_attempts: u32,
    pub user_agent: String,
}

impl Default for GovernorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 8,
            hourly_request_budget: 1_500,
            request_timeout: Duration::from_secs(60),
            max_attempts: 5,
            user_agent: "quay/0.1 (measurement harness)".to_owned(),
        }
    }
}

struct GovernorMetrics {
    spent_points: u32,
    issued_requests: u32,
    throttled_responses: u32,
    health: Health,
    rate_limit: Option<RateLimitSnapshot>,
}

pub struct RateGovernor {
    client: Client,
    token: Option<Token>,
    locks: DevLocks,
    permits: Semaphore,
    budget: Mutex<SlidingBudget>,
    backoff: Backoff,
    metrics: Mutex<GovernorMetrics>,
    max_attempts: u32,
}

impl RateGovernor {
    pub fn new(
        config: GovernorConfig,
        token: Option<Token>,
        locks: DevLocks,
    ) -> Result<Self, ForgeError> {
        let client = Client::builder()
            .user_agent(config.user_agent.clone())
            .timeout(config.request_timeout)
            .build()
            .map_err(|error| ForgeError::ClientBuild(error.to_string()))?;
        Ok(Self {
            client,
            token,
            locks,
            permits: Semaphore::new(config.max_concurrent_requests),
            budget: Mutex::new(SlidingBudget::per_hour(config.hourly_request_budget)),
            backoff: Backoff::default(),
            metrics: Mutex::new(GovernorMetrics {
                spent_points: 0,
                issued_requests: 0,
                throttled_responses: 0,
                health: Health::Healthy,
                rate_limit: None,
            }),
            max_attempts: config.max_attempts,
        })
    }

    pub fn health(&self) -> Health {
        self.with_metrics(|metrics| metrics.health)
    }

    pub fn rate_limit(&self) -> Option<RateLimitSnapshot> {
        self.with_metrics(|metrics| metrics.rate_limit)
    }

    pub fn spent_points(&self) -> u32 {
        self.with_metrics(|metrics| metrics.spent_points)
    }

    pub fn issued_requests(&self) -> u32 {
        self.with_metrics(|metrics| metrics.issued_requests)
    }

    pub fn throttled_responses(&self) -> u32 {
        self.with_metrics(|metrics| metrics.throttled_responses)
    }

    pub async fn send(&self, request: OutboundRequest) -> Result<ForgeResponse, ForgeError> {
        self.locks.authorize(&request)?;
        self.reserve_budget()?;

        let _permit = self
            .permits
            .acquire()
            .await
            .map_err(|error| ForgeError::Transport(error.to_string()))?;

        let started = Instant::now();
        let mut attempt = 0u32;

        loop {
            let outcome = self.issue(&request).await?;
            attempt += 1;
            self.record(&request, &outcome);

            if !is_throttled(outcome.status) || attempt >= self.max_attempts {
                if is_throttled(outcome.status) {
                    return Err(ForgeError::Throttled {
                        attempts: attempt,
                        last_status: outcome.status,
                    });
                }
                return Ok(ForgeResponse {
                    status: outcome.status,
                    bytes: outcome.body.len(),
                    body: outcome.body,
                    attempts: attempt,
                    elapsed: started.elapsed(),
                    rate_limit: outcome.rate_limit,
                    points: request.kind.points(),
                });
            }

            self.mark_throttled();
            let delay = self
                .backoff
                .delay(attempt - 1, outcome.retry_after, process_jitter());
            tokio::time::sleep(delay).await;
            self.reserve_budget()?;
        }
    }

    fn reserve_budget(&self) -> Result<(), ForgeError> {
        let mut budget = match self.budget.lock() {
            Ok(budget) => budget,
            Err(poisoned) => poisoned.into_inner(),
        };
        budget
            .reserve(Instant::now())
            .map_err(|wait| ForgeError::BudgetExhausted {
                budget: budget.limit(),
                retry_in_seconds: wait.as_secs(),
            })
    }

    async fn issue(&self, request: &OutboundRequest) -> Result<Attempt, ForgeError> {
        let mut builder = self
            .client
            .request(request.method.clone(), &request.url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = self.token.as_ref() {
            builder = builder.header("Authorization", token.header_value());
        }
        if let Some(body) = request.body.as_ref() {
            builder = builder
                .header("Content-Type", "application/json")
                .body(body.clone());
        }

        let response = builder.send().await.map_err(|error| {
            ForgeError::Transport(self.redact(&strip_credentials(&error.to_string())))
        })?;

        let status = response.status().as_u16();
        let rate_limit = read_rate_limit(response.headers());
        let retry_after = read_retry_after(response.headers());
        let body = response.text().await.map_err(|error| {
            ForgeError::Transport(self.redact(&strip_credentials(&error.to_string())))
        })?;

        Ok(Attempt {
            status,
            body,
            rate_limit,
            retry_after,
        })
    }

    fn record(&self, request: &OutboundRequest, outcome: &Attempt) {
        let counted_against_quota = outcome.status != StatusCode::NOT_MODIFIED.as_u16();
        self.with_metrics_mut(|metrics| {
            metrics.issued_requests += 1;
            if counted_against_quota {
                metrics.spent_points += request.kind.points();
            }
            if let Some(snapshot) = outcome.rate_limit {
                metrics.rate_limit = Some(snapshot);
                if metrics.health != Health::Throttled {
                    metrics.health = if snapshot.is_degraded() {
                        Health::Degraded
                    } else {
                        Health::Healthy
                    };
                }
            }
        });
        tracing::debug!(
            kind = request.kind.label(),
            priority = request.priority.as_str(),
            status = outcome.status,
            "outbound request completed"
        );
    }

    fn mark_throttled(&self) {
        self.with_metrics_mut(|metrics| {
            metrics.throttled_responses += 1;
            metrics.health = Health::Throttled;
        });
    }

    fn redact(&self, text: &str) -> String {
        match self.token.as_ref() {
            Some(token) => token.redact(text),
            None => text.to_owned(),
        }
    }

    fn with_metrics<T>(&self, read: impl FnOnce(&GovernorMetrics) -> T) -> T {
        match self.metrics.lock() {
            Ok(metrics) => read(&metrics),
            Err(poisoned) => read(&poisoned.into_inner()),
        }
    }

    fn with_metrics_mut(&self, write: impl FnOnce(&mut GovernorMetrics)) {
        match self.metrics.lock() {
            Ok(mut metrics) => write(&mut metrics),
            Err(poisoned) => write(&mut poisoned.into_inner()),
        }
    }
}

struct Attempt {
    status: u16,
    body: String,
    rate_limit: Option<RateLimitSnapshot>,
    retry_after: Option<Duration>,
}

fn is_throttled(status: u16) -> bool {
    status == 403 || status == 429
}

fn strip_credentials(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            if word.contains("://") && word.contains('@') {
                "[url with credentials]"
            } else {
                word
            }
        })
        .collect::<Vec<&str>>()
        .join(" ")
}

fn header_number(headers: &reqwest::header::HeaderMap, name: &str) -> Option<u64> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
}

fn read_rate_limit(headers: &reqwest::header::HeaderMap) -> Option<RateLimitSnapshot> {
    Some(RateLimitSnapshot {
        limit: header_number(headers, "x-ratelimit-limit")? as u32,
        remaining: header_number(headers, "x-ratelimit-remaining")? as u32,
        reset_at: header_number(headers, "x-ratelimit-reset")?,
    })
}

fn read_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    header_number(headers, "retry-after").map(Duration::from_secs)
}

fn process_jitter() -> f64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.subsec_nanos())
        .unwrap_or(0);
    0.5 + f64::from(nanos % 500_000) / 1_000_000.0
}
