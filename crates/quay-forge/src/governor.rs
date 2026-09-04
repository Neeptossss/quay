use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use quay_core::Priority;
use reqwest::{Client, Method, StatusCode};
use tokio::sync::Semaphore;

use crate::backoff::Backoff;
use crate::budget::SlidingBudget;
use crate::conditional::{CacheValidators, header_text};
use crate::error::ForgeError;
use crate::locks::DevLocks;
use crate::token::Token;

const DEGRADED_QUOTA_FRACTION: f64 = 0.20;
const SPECULATIVE_QUOTA_PERCENT: u32 = 15;

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
    pub validators: Option<CacheValidators>,
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
            validators: None,
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
            validators: None,
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
            validators: None,
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
            validators: None,
        }
    }

    pub fn revalidating(mut self, validators: Option<CacheValidators>) -> Self {
        self.validators = validators;
        self
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
    pub validators: Option<CacheValidators>,
    pub poll_interval: Option<Duration>,
    pub single_sign_on: Option<String>,
    pub granted_scopes: Option<String>,
}

impl ForgeResponse {
    pub fn is_not_modified(&self) -> bool {
        self.status == StatusCode::NOT_MODIFIED.as_u16()
    }
}

pub struct GovernorConfig {
    pub max_concurrent_requests: usize,
    pub hourly_request_budget: u32,
    pub request_timeout: Duration,
    pub max_attempts: u32,
    pub user_agent: String,
}

impl GovernorConfig {
    pub fn speculative_budget(&self) -> u32 {
        self.hourly_request_budget * SPECULATIVE_QUOTA_PERCENT / 100
    }
}

impl Default for GovernorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 8,
            hourly_request_budget: 1_500,
            request_timeout: Duration::from_secs(60),
            max_attempts: 5,
            user_agent: "quay/0.1".to_owned(),
        }
    }
}

struct GovernorMetrics {
    spent_points: u32,
    issued_requests: u32,
    free_revalidations: u32,
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
    speculative_budget: Mutex<SlidingBudget>,
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
            speculative_budget: Mutex::new(SlidingBudget::per_hour(config.speculative_budget())),
            backoff: Backoff::default(),
            metrics: Mutex::new(GovernorMetrics {
                spent_points: 0,
                issued_requests: 0,
                free_revalidations: 0,
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

    pub fn free_revalidations(&self) -> u32 {
        self.with_metrics(|metrics| metrics.free_revalidations)
    }

    pub fn throttled_responses(&self) -> u32 {
        self.with_metrics(|metrics| metrics.throttled_responses)
    }

    pub async fn send(&self, request: OutboundRequest) -> Result<ForgeResponse, ForgeError> {
        self.locks.authorize(&request)?;
        self.reserve_budget(request.priority)?;

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

            if !is_throttled(&outcome) {
                return Ok(ForgeResponse {
                    status: outcome.status,
                    bytes: outcome.body.len(),
                    body: outcome.body,
                    attempts: attempt,
                    elapsed: started.elapsed(),
                    rate_limit: outcome.rate_limit,
                    points: request.kind.points(),
                    validators: outcome.validators,
                    poll_interval: outcome.poll_interval,
                    single_sign_on: outcome.single_sign_on,
                    granted_scopes: outcome.granted_scopes,
                });
            }
            if attempt >= self.max_attempts {
                return Err(ForgeError::Throttled {
                    attempts: attempt,
                    last_status: outcome.status,
                });
            }

            self.mark_throttled();
            let delay = self
                .backoff
                .delay(attempt - 1, outcome.retry_after, process_jitter());
            tokio::time::sleep(delay).await;
            self.reserve_budget(request.priority)?;
        }
    }

    fn reserve_budget(&self, priority: Priority) -> Result<(), ForgeError> {
        if priority == Priority::Speculative {
            let mut speculative = lock(&self.speculative_budget);
            speculative.reserve(Instant::now()).map_err(|wait| {
                ForgeError::SpeculationBudgetExhausted {
                    budget: speculative.limit(),
                    retry_in_seconds: wait.as_secs(),
                }
            })?;
        }
        let mut budget = lock(&self.budget);
        budget
            .reserve(Instant::now())
            .map_err(|wait| ForgeError::BudgetExhausted {
                budget: budget.limit(),
                retry_in_seconds: wait.as_secs(),
            })
    }

    fn refund_budget(&self, priority: Priority) {
        let now = Instant::now();
        lock(&self.budget).refund(now);
        if priority == Priority::Speculative {
            lock(&self.speculative_budget).refund(now);
        }
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
        if let Some(validators) = request.validators.as_ref() {
            if let Some(etag) = validators.etag.as_ref() {
                builder = builder.header("If-None-Match", etag);
            }
            if let Some(last_modified) = validators.last_modified.as_ref() {
                builder = builder.header("If-Modified-Since", last_modified);
            }
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
        let headers = response.headers().clone();
        let body = response.text().await.map_err(|error| {
            ForgeError::Transport(self.redact(&strip_credentials(&error.to_string())))
        })?;

        Ok(Attempt {
            status,
            body,
            rate_limit: read_rate_limit(&headers),
            retry_after: read_retry_after(&headers),
            validators: CacheValidators::from_headers(&headers),
            poll_interval: header_text(&headers, "x-poll-interval")
                .and_then(|value| value.trim().parse::<u64>().ok())
                .map(Duration::from_secs),
            single_sign_on: header_text(&headers, "x-github-sso"),
            granted_scopes: header_text(&headers, "x-oauth-scopes"),
        })
    }

    fn record(&self, request: &OutboundRequest, outcome: &Attempt) {
        let revalidated_for_free = outcome.status == StatusCode::NOT_MODIFIED.as_u16();
        if revalidated_for_free {
            self.refund_budget(request.priority);
        }
        self.with_metrics_mut(|metrics| {
            metrics.issued_requests += 1;
            if revalidated_for_free {
                metrics.free_revalidations += 1;
            } else {
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
        read(&lock(&self.metrics))
    }

    fn with_metrics_mut(&self, write: impl FnOnce(&mut GovernorMetrics)) {
        write(&mut lock(&self.metrics));
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

struct Attempt {
    status: u16,
    body: String,
    rate_limit: Option<RateLimitSnapshot>,
    retry_after: Option<Duration>,
    validators: Option<CacheValidators>,
    poll_interval: Option<Duration>,
    single_sign_on: Option<String>,
    granted_scopes: Option<String>,
}

fn is_throttled(outcome: &Attempt) -> bool {
    if outcome.status == 429 {
        return true;
    }
    if outcome.status != 403 {
        return false;
    }
    if outcome.retry_after.is_some() {
        return true;
    }
    if outcome
        .rate_limit
        .is_some_and(|snapshot| snapshot.remaining == 0)
    {
        return true;
    }
    let lowered = outcome.body.to_lowercase();
    lowered.contains("rate limit") || lowered.contains("abuse detection")
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
    header_text(headers, name).and_then(|value| value.trim().parse::<u64>().ok())
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
