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

#[derive(Clone, Default)]
pub struct SharedValidators {
    cell: Arc<Mutex<Option<CacheValidators>>>,
}

impl SharedValidators {
    pub fn holding(validators: Option<CacheValidators>) -> Self {
        Self {
            cell: Arc::new(Mutex::new(validators)),
        }
    }

    pub fn current(&self) -> Option<CacheValidators> {
        match self.cell.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn replace(&self, validators: CacheValidators) {
        match self.cell.lock() {
            Ok(mut guard) => *guard = Some(validators),
            Err(poisoned) => *poisoned.into_inner() = Some(validators),
        }
    }
}

pub struct PollingSource {
    governor: Arc<RateGovernor>,
    endpoint: String,
    validators: SharedValidators,
    interval: Duration,
    last_status: Option<u16>,
    last_error: Option<String>,
}

impl PollingSource {
    pub fn new(governor: Arc<RateGovernor>, api_base: &str) -> Self {
        Self::sharing(governor, api_base, SharedValidators::default())
    }

    pub fn sharing(
        governor: Arc<RateGovernor>,
        api_base: &str,
        validators: SharedValidators,
    ) -> Self {
        Self {
            governor,
            endpoint: format!("{}/notifications", api_base.trim_end_matches('/')),
            validators,
            interval: Tier::Warm.refresh_interval(),
            last_status: None,
            last_error: None,
        }
    }

    pub fn validators(&self) -> SharedValidators {
        self.validators.clone()
    }

    pub fn interval(&self) -> Duration {
        self.interval
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
            .revalidating(self.validators.current());
        let response = match self.governor.send(request).await {
            Ok(response) => response,
            Err(error) => {
                self.last_error = Some(error.to_string());
                self.last_status = None;
                return Vec::new();
            }
        };

        self.last_status = Some(response.status);
        self.interval = Tier::Warm.interval_respecting(response.poll_interval);
        if let Some(validators) = response.validators.clone() {
            self.validators.replace(validators);
        }

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
