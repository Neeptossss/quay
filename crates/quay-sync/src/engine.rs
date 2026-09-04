use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use quay_core::{ChangeSignal, EventSource, Priority, Tier, merge_deduplicated};
use quay_forge::{CacheValidators, RateGovernor, fetch_pull_request};
use quay_store::{CacheEntry, Store};

use crate::error::SyncError;

const DUE_BATCH: i64 = 64;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct TickReport {
    pub signals: usize,
    pub already_current: usize,
    pub fetched: usize,
    pub stored: usize,
    pub failures: Vec<String>,
}

pub struct SyncEngine {
    store: Store,
    governor: Arc<RateGovernor>,
    api_base: String,
    graphql_endpoint: String,
    account_id: i64,
    sources: Vec<Box<dyn EventSource>>,
}

impl SyncEngine {
    pub fn new(store: Store, governor: Arc<RateGovernor>, api_base: &str, account_id: i64) -> Self {
        Self {
            store,
            governor,
            api_base: api_base.trim_end_matches('/').to_owned(),
            graphql_endpoint: format!("{}/graphql", api_base.trim_end_matches('/')),
            account_id,
            sources: Vec::new(),
        }
    }

    pub fn listening_to(mut self, source: Box<dyn EventSource>) -> Self {
        self.sources.push(source);
        self
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub async fn drain_mutations(
        &mut self,
        limit: usize,
    ) -> Result<crate::MutationReport, SyncError> {
        crate::worker::drain(&mut self.store, &self.governor, &self.api_base, limit).await
    }

    pub fn replay_interrupted_mutations(&mut self) -> Result<quay_store::ReplayReport, SyncError> {
        Ok(self.store.replay_interrupted_mutations()?)
    }

    pub async fn tick(&mut self) -> Result<TickReport, SyncError> {
        let signals = self.collect_signals().await;
        let mut report = TickReport {
            signals: signals.len(),
            ..TickReport::default()
        };

        for signal in signals {
            match self.reconcile(&signal).await {
                Ok(Outcome::AlreadyCurrent) => report.already_current += 1,
                Ok(Outcome::Stored) => {
                    report.fetched += 1;
                    report.stored += 1;
                }
                Err(failure) => {
                    report.fetched += 1;
                    report.failures.push(failure);
                }
            }
        }
        Ok(report)
    }

    pub fn due_for_refresh(&self) -> Result<Vec<CacheEntry>, SyncError> {
        Ok(self.store.due_for_refresh(now_seconds(), DUE_BATCH)?)
    }

    async fn collect_signals(&mut self) -> Vec<ChangeSignal> {
        let mut collected = Vec::new();
        for source in &mut self.sources {
            collected.extend(source.next().await);
        }
        merge_deduplicated(collected)
    }

    async fn reconcile(&mut self, signal: &ChangeSignal) -> Result<Outcome, String> {
        let Some((owner, name, number)) = signal.entity.as_pull_request() else {
            return Err(format!("unreadable entity {}", signal.entity.key));
        };
        let key = cache_key(&signal.entity.key);

        match self.store.freshness(&key) {
            Ok(Some(entry)) if entry.fetched_at >= signal.updated_at.0 => {
                return Ok(Outcome::AlreadyCurrent);
            }
            Ok(_) => {}
            Err(error) => return Err(error.to_string()),
        }

        let snapshot = fetch_pull_request(
            &self.governor,
            &self.graphql_endpoint,
            &owner,
            &name,
            number,
            Priority::Warm,
        )
        .await
        .map_err(|error| format!("{}: {error}", signal.entity.key))?;

        self.store
            .save_pull_request(self.account_id, &snapshot)
            .map_err(|error| format!("{}: {error}", signal.entity.key))?;

        let fetched_at = now_seconds();
        let entry = CacheEntry {
            key,
            etag: None,
            last_modified: None,
            fetched_at,
            stale_after: fetched_at + Tier::Warm.refresh_interval().as_secs() as i64,
            tier: Tier::Warm,
        };
        self.store
            .remember_freshness(&entry)
            .map_err(|error| format!("{}: {error}", signal.entity.key))?;
        Ok(Outcome::Stored)
    }
}

pub const NOTIFICATIONS_KEY: &str = "notifications";

pub fn notification_validators(store: &Store) -> Result<Option<CacheValidators>, SyncError> {
    Ok(store
        .freshness(NOTIFICATIONS_KEY)?
        .map(|entry| CacheValidators {
            etag: entry.etag,
            last_modified: entry.last_modified,
        }))
}

pub fn remember_notification_validators(
    store: &Store,
    validators: Option<&CacheValidators>,
) -> Result<(), SyncError> {
    let fetched_at = now_seconds();
    let entry = CacheEntry {
        key: NOTIFICATIONS_KEY.to_owned(),
        etag: validators.and_then(|validators| validators.etag.clone()),
        last_modified: validators.and_then(|validators| validators.last_modified.clone()),
        fetched_at,
        stale_after: fetched_at + Tier::Warm.refresh_interval().as_secs() as i64,
        tier: Tier::Warm,
    };
    Ok(store.remember_freshness(&entry)?)
}

enum Outcome {
    AlreadyCurrent,
    Stored,
}

fn cache_key(entity_key: &str) -> String {
    format!("pr:{entity_key}")
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}
