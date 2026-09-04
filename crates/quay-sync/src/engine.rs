use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use quay_core::{
    ChangeSignal, EventSource, NavigationState, Priority, Tier, merge_deduplicated, plan_preloads,
    speculation_stays_enabled,
};
use quay_forge::{CacheValidators, RateGovernor, fetch_pull_request};
use quay_store::inbox::{InboxFilter, InboxQuery};
use quay_store::{CacheEntry, Store};

use crate::error::SyncError;

const DUE_BATCH: i64 = 64;
const UTILISATION_WINDOW: usize = 500;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct PreloadReport {
    pub planned: usize,
    pub already_fresh: usize,
    pub fetched: usize,
    pub refused_by_budget: usize,
    pub speculation_disabled: bool,
    pub failures: Vec<String>,
}

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

    pub async fn preload(&mut self, viewer: &str) -> Result<PreloadReport, SyncError> {
        let utilisation = self.store.speculation_utilisation(UTILISATION_WINDOW)?;
        if !speculation_stays_enabled(utilisation.used, utilisation.observed) {
            return Ok(PreloadReport {
                speculation_disabled: true,
                ..PreloadReport::default()
            });
        }

        let filter = InboxFilter {
            viewer: viewer.to_owned(),
            ..InboxFilter::default()
        };
        let list: Vec<String> = self
            .store
            .inbox(InboxQuery::ReviewRequestedExact, &filter)?
            .iter()
            .map(|row| format!("{}/{}#{}", row.owner, row.name, row.number))
            .collect();

        let planned = plan_preloads(&NavigationState::inbox(list));
        let mut report = PreloadReport {
            planned: planned.len(),
            ..PreloadReport::default()
        };
        let now = now_seconds();

        for request in planned {
            let key = cache_key(&request.key);
            match self.store.freshness(&key)? {
                Some(entry) if !entry.is_stale_at(now) => {
                    report.already_fresh += 1;
                    continue;
                }
                _ => {}
            }
            let Some((owner, name, number)) = entity_locator(&request.key) else {
                report
                    .failures
                    .push(format!("unreadable key {}", request.key));
                continue;
            };
            match fetch_pull_request(
                &self.governor,
                &self.graphql_endpoint,
                &owner,
                &name,
                number,
                Priority::Speculative,
            )
            .await
            {
                Ok(snapshot) => {
                    self.store.save_pull_request(self.account_id, &snapshot)?;
                    self.remember_freshness(&key, now)?;
                    self.store
                        .note_speculation(&request.key, request.reason, now)?;
                    report.fetched += 1;
                }
                Err(quay_forge::ForgeError::SpeculationBudgetExhausted { .. }) => {
                    report.refused_by_budget += 1;
                }
                Err(error) => report.failures.push(format!("{}: {error}", request.key)),
            }
        }
        Ok(report)
    }

    fn remember_freshness(&self, key: &str, now: i64) -> Result<(), SyncError> {
        Ok(self.store.remember_freshness(&CacheEntry {
            key: key.to_owned(),
            etag: None,
            last_modified: None,
            fetched_at: now,
            stale_after: now + Tier::Warm.refresh_interval().as_secs() as i64,
            tier: Tier::Warm,
        })?)
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
                self.remember_freshness(&key, now_seconds())
                    .map_err(|error| format!("{}: {error}", signal.entity.key))?;
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

        self.remember_freshness(&key, now_seconds())
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

fn entity_locator(entity_key: &str) -> Option<(String, String, i64)> {
    let (repository, number) = entity_key.rsplit_once('#')?;
    let (owner, name) = repository.split_once('/')?;
    Some((owner.to_owned(), name.to_owned(), number.parse().ok()?))
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}
