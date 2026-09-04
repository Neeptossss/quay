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
    pub node_id: String,
}

impl EntityId {
    pub fn new(kind: EntityKind, node_id: impl Into<String>) -> Self {
        Self {
            kind,
            node_id: node_id.into(),
        }
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

    fn signal(node_id: &str, updated_at: i64) -> ChangeSignal {
        ChangeSignal::new(
            EntityId::new(EntityKind::PullRequest, node_id),
            Timestamp(updated_at),
        )
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
    fn the_same_node_id_under_two_entity_kinds_is_not_deduplicated() {
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
