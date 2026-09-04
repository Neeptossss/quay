use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    Repository,
    PullRequest,
    ReviewThread,
    Notification,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId {
    pub kind: EntityKind,
    pub key: String,
}

impl EntityId {
    pub fn new(kind: EntityKind, key: impl Into<String>) -> Self {
        Self {
            kind,
            key: key.into(),
        }
    }

    pub fn pull_request(owner: &str, name: &str, number: i64) -> Self {
        Self::new(EntityKind::PullRequest, format!("{owner}/{name}#{number}"))
    }

    pub fn as_pull_request(&self) -> Option<(String, String, i64)> {
        if self.kind != EntityKind::PullRequest {
            return None;
        }
        let (repository, number) = self.key.rsplit_once('#')?;
        let (owner, name) = repository.split_once('/')?;
        Some((owner.to_owned(), name.to_owned(), number.parse().ok()?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChangeSignal {
    pub entity: EntityId,
    pub updated_at: Timestamp,
}

impl ChangeSignal {
    pub fn new(entity: EntityId, updated_at: Timestamp) -> Self {
        Self { entity, updated_at }
    }
}

pub fn merge_deduplicated(signals: impl IntoIterator<Item = ChangeSignal>) -> Vec<ChangeSignal> {
    let mut seen: HashSet<ChangeSignal> = HashSet::new();
    let mut merged = Vec::new();
    for signal in signals {
        if seen.insert(signal.clone()) {
            merged.push(signal);
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::{ChangeSignal, EntityId, EntityKind, Timestamp, merge_deduplicated};

    fn signal(key: &str, updated_at: i64) -> ChangeSignal {
        ChangeSignal::new(
            EntityId::new(EntityKind::PullRequest, key),
            Timestamp(updated_at),
        )
    }

    #[test]
    fn a_pull_request_key_survives_a_round_trip_through_its_locator() {
        let entity = EntityId::pull_request("acme", "api", 123);
        assert_eq!(entity.key, "acme/api#123");
        assert_eq!(
            entity.as_pull_request(),
            Some(("acme".to_owned(), "api".to_owned(), 123))
        );
    }

    #[test]
    fn a_repository_name_carrying_a_hash_is_still_parsed_from_the_right() {
        let entity = EntityId::new(EntityKind::PullRequest, "acme/we#ird#7");
        assert_eq!(
            entity.as_pull_request(),
            Some(("acme".to_owned(), "we#ird".to_owned(), 7))
        );
    }

    #[test]
    fn an_entity_of_another_kind_is_not_read_as_a_pull_request() {
        let entity = EntityId::new(EntityKind::Notification, "acme/api#1");
        assert!(entity.as_pull_request().is_none());
    }

    #[test]
    fn a_malformed_locator_is_refused_rather_than_guessed() {
        for key in ["acme/api", "acme#1", "acme/api#not-a-number", ""] {
            let entity = EntityId::new(EntityKind::PullRequest, key);
            assert!(entity.as_pull_request().is_none(), "{key}");
        }
    }

    #[test]
    fn merging_an_empty_stream_yields_nothing() {
        assert!(merge_deduplicated(Vec::new()).is_empty());
    }

    #[test]
    fn the_same_entity_reported_twice_at_the_same_instant_is_kept_once() {
        let merged = merge_deduplicated(vec![signal("PR_1", 100), signal("PR_1", 100)]);
        assert_eq!(merged, vec![signal("PR_1", 100)]);
    }

    #[test]
    fn the_same_entity_reported_at_two_instants_is_kept_twice() {
        let merged = merge_deduplicated(vec![signal("PR_1", 100), signal("PR_1", 200)]);
        assert_eq!(merged, vec![signal("PR_1", 100), signal("PR_1", 200)]);
    }

    #[test]
    fn the_same_key_under_two_entity_kinds_is_not_deduplicated() {
        let pull_request = signal("SHARED", 100);
        let thread = ChangeSignal::new(
            EntityId::new(EntityKind::ReviewThread, "SHARED"),
            Timestamp(100),
        );
        let merged = merge_deduplicated(vec![pull_request.clone(), thread.clone()]);
        assert_eq!(merged, vec![pull_request, thread]);
    }

    #[test]
    fn merging_two_sources_preserves_first_seen_order() {
        let polled = vec![signal("PR_1", 100), signal("PR_2", 100)];
        let pushed = vec![signal("PR_2", 100), signal("PR_3", 100)];
        let merged = merge_deduplicated(polled.into_iter().chain(pushed));
        assert_eq!(
            merged,
            vec![
                signal("PR_1", 100),
                signal("PR_2", 100),
                signal("PR_3", 100)
            ]
        );
    }
}
