use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use quay_core::{ChangeSignal, EventSource, Priority, Tier, Timestamp, merge_deduplicated};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::conditional::CacheValidators;
use crate::governor::{OutboundRequest, RateGovernor};
use crate::notifications::signal_from;
use crate::transport::NotificationPayload;

pub fn epoch_seconds(rfc3339: &str) -> Option<i64> {
    OffsetDateTime::parse(rfc3339, &Rfc3339)
        .ok()
        .map(|moment| moment.unix_timestamp())
}

struct Checkpoint {
    validators: Option<CacheValidators>,
    interval: Duration,
}

#[derive(Clone)]
pub struct PollingCheckpoint {
    cell: Arc<Mutex<Checkpoint>>,
}

impl Default for PollingCheckpoint {
    fn default() -> Self {
        Self::holding(None)
    }
}

impl PollingCheckpoint {
    pub fn holding(validators: Option<CacheValidators>) -> Self {
        Self {
            cell: Arc::new(Mutex::new(Checkpoint {
                validators,
                interval: Tier::Warm.refresh_interval(),
            })),
        }
    }

    pub fn validators(&self) -> Option<CacheValidators> {
        self.read(|checkpoint| checkpoint.validators.clone())
    }

    pub fn interval(&self) -> Duration {
        self.read(|checkpoint| checkpoint.interval)
    }

    fn read<T>(&self, take: impl FnOnce(&Checkpoint) -> T) -> T {
        match self.cell.lock() {
            Ok(guard) => take(&guard),
            Err(poisoned) => take(&poisoned.into_inner()),
        }
    }

    fn write(&self, change: impl FnOnce(&mut Checkpoint)) {
        match self.cell.lock() {
            Ok(mut guard) => change(&mut guard),
            Err(poisoned) => change(&mut poisoned.into_inner()),
        }
    }
}

pub struct PollingSource {
    governor: Arc<RateGovernor>,
    endpoint: String,
    checkpoint: PollingCheckpoint,
    last_status: Option<u16>,
    last_error: Option<String>,
}

impl PollingSource {
    pub fn new(governor: Arc<RateGovernor>, api_base: &str) -> Self {
        Self::sharing(governor, api_base, PollingCheckpoint::default())
    }

    pub fn sharing(
        governor: Arc<RateGovernor>,
        api_base: &str,
        checkpoint: PollingCheckpoint,
    ) -> Self {
        Self {
            governor,
            endpoint: format!("{}/notifications", api_base.trim_end_matches('/')),
            checkpoint,
            last_status: None,
            last_error: None,
        }
    }

    pub fn checkpoint(&self) -> PollingCheckpoint {
        self.checkpoint.clone()
    }

    pub fn last_status(&self) -> Option<u16> {
        self.last_status
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
}

#[async_trait]
impl EventSource for PollingSource {
    async fn next(&mut self) -> Vec<ChangeSignal> {
        let request = OutboundRequest::rest_read(&self.endpoint, Priority::Warm)
            .revalidating(self.checkpoint.validators());
        let response = match self.governor.send(request).await {
            Ok(response) => response,
            Err(error) => {
                self.last_error = Some(error.to_string());
                self.last_status = None;
                return Vec::new();
            }
        };

        self.last_status = Some(response.status);
        let advised = Tier::Warm.interval_respecting(response.poll_interval);
        let validators = response.validators.clone();
        self.checkpoint.write(|checkpoint| {
            checkpoint.interval = advised;
            if let Some(validators) = validators {
                checkpoint.validators = Some(validators);
            }
        });

        if response.is_not_modified() {
            self.last_error = None;
            return Vec::new();
        }
        if response.status != 200 {
            self.last_error = Some(crate::auth::forge_message(&response.body));
            return Vec::new();
        }

        match serde_json::from_str::<Vec<NotificationPayload>>(&response.body) {
            Ok(payloads) => {
                self.last_error = None;
                merge_deduplicated(payloads.iter().filter_map(|payload| {
                    let updated_at = Timestamp(epoch_seconds(&payload.updated_at)?);
                    signal_from(payload, updated_at)
                }))
            }
            Err(error) => {
                self.last_error = Some(error.to_string());
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::epoch_seconds;

    #[test]
    fn an_rfc3339_instant_is_read_as_seconds_since_the_epoch() {
        assert_eq!(epoch_seconds("2026-09-04T10:00:00Z"), Some(1_788_516_000));
    }

    #[test]
    fn an_offset_instant_is_normalised_to_the_same_epoch_second() {
        assert_eq!(
            epoch_seconds("2026-09-04T12:00:00+02:00"),
            epoch_seconds("2026-09-04T10:00:00Z")
        );
    }

    #[test]
    fn an_unparseable_instant_yields_nothing_rather_than_the_epoch() {
        for value in ["", "yesterday", "2026-09-04", "2026-13-45T99:00:00Z"] {
            assert!(epoch_seconds(value).is_none(), "{value}");
        }
    }
}
