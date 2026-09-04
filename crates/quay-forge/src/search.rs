use std::sync::Arc;

use async_trait::async_trait;
use quay_core::{ChangeSignal, EntityId, EventSource, Priority, Timestamp, merge_deduplicated};
use serde::Deserialize;

use crate::governor::{OutboundRequest, RateGovernor};
use crate::polling::epoch_seconds;

const PER_PAGE: u32 = 100;

#[derive(Debug, Deserialize)]
struct SearchPayload {
    items: Vec<SearchItem>,
}

#[derive(Debug, Deserialize)]
struct SearchItem {
    number: i64,
    updated_at: String,
    repository_url: String,
}

pub fn repository_from_url(repository_url: &str) -> Option<(String, String)> {
    let tail = repository_url.split("/repos/").nth(1)?;
    let (owner, name) = tail.split_once('/')?;
    let name = name.split('/').next()?;
    if owner.is_empty() || name.is_empty() {
        return None;
    }
    Some((owner.to_owned(), name.to_owned()))
}

pub struct SearchSource {
    governor: Arc<RateGovernor>,
    endpoint: String,
    queries: Vec<String>,
    last_error: Option<String>,
}

impl SearchSource {
    pub fn new(governor: Arc<RateGovernor>, api_base: &str, queries: Vec<String>) -> Self {
        Self {
            governor,
            endpoint: format!("{}/search/issues", api_base.trim_end_matches('/')),
            queries,
            last_error: None,
        }
    }

    pub fn review_queue(governor: Arc<RateGovernor>, api_base: &str) -> Self {
        Self::new(
            governor,
            api_base,
            vec![
                "is:pr is:open review-requested:@me".to_owned(),
                "is:pr is:open author:@me".to_owned(),
            ],
        )
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    async fn signals_for(&self, query: &str) -> Result<Vec<ChangeSignal>, String> {
        let url = format!(
            "{}?q={}&per_page={PER_PAGE}&advanced_search=true",
            self.endpoint,
            urlencode(query)
        );
        let response = self
            .governor
            .send(OutboundRequest::rest_read(url, Priority::Warm))
            .await
            .map_err(|error| error.to_string())?;
        if response.status != 200 {
            return Err(crate::auth::forge_message(&response.body));
        }
        let payload: SearchPayload =
            serde_json::from_str(&response.body).map_err(|error| error.to_string())?;
        Ok(payload
            .items
            .iter()
            .filter_map(|item| {
                let (owner, name) = repository_from_url(&item.repository_url)?;
                Some(ChangeSignal::new(
                    EntityId::pull_request(&owner, &name, item.number),
                    Timestamp(epoch_seconds(&item.updated_at)?),
                ))
            })
            .collect())
    }
}

fn urlencode(query: &str) -> String {
    query
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => character.to_string(),
            ' ' => "+".to_owned(),
            other => other
                .to_string()
                .as_bytes()
                .iter()
                .map(|byte| format!("%{byte:02X}"))
                .collect(),
        })
        .collect()
}

#[async_trait]
impl EventSource for SearchSource {
    async fn next(&mut self) -> Vec<ChangeSignal> {
        let mut collected = Vec::new();
        let mut failures = Vec::new();
        for query in &self.queries {
            match self.signals_for(query).await {
                Ok(signals) => collected.extend(signals),
                Err(error) => failures.push(format!("{query}: {error}")),
            }
        }
        self.last_error = if failures.is_empty() {
            None
        } else {
            Some(failures.join(" | "))
        };
        merge_deduplicated(collected)
    }
}

#[cfg(test)]
mod tests {
    use super::{repository_from_url, urlencode};

    #[test]
    fn a_repository_url_yields_its_owner_and_name() {
        assert_eq!(
            repository_from_url("https://api.github.com/repos/acme/api"),
            Some(("acme".to_owned(), "api".to_owned()))
        );
    }

    #[test]
    fn a_repository_url_with_a_trailing_path_still_yields_the_name_alone() {
        assert_eq!(
            repository_from_url("https://api.github.com/repos/acme/api/issues/1"),
            Some(("acme".to_owned(), "api".to_owned()))
        );
    }

    #[test]
    fn a_malformed_repository_url_yields_nothing_rather_than_a_wrong_name() {
        for url in [
            "https://api.github.com/repos/acme",
            "",
            "https://example.com",
        ] {
            assert!(repository_from_url(url).is_none(), "{url}");
        }
    }

    #[test]
    fn a_search_query_is_encoded_without_breaking_its_qualifiers() {
        assert_eq!(
            urlencode("is:pr is:open review-requested:@me"),
            "is%3Apr+is%3Aopen+review-requested%3A%40me"
        );
    }

    #[test]
    fn an_unreserved_character_is_left_untouched_by_the_encoder() {
        assert_eq!(urlencode("abc-DEF_123.~"), "abc-DEF_123.~");
    }
}
