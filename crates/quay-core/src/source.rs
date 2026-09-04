use async_trait::async_trait;

use crate::change::ChangeSignal;

#[async_trait]
pub trait EventSource: Send {
    async fn next(&mut self) -> Vec<ChangeSignal>;
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::EventSource;
    use crate::change::{ChangeSignal, EntityId, EntityKind, Timestamp, merge_deduplicated};

    struct ScriptedSource {
        batches: Vec<Vec<ChangeSignal>>,
    }

    impl ScriptedSource {
        fn new(batches: Vec<Vec<ChangeSignal>>) -> Self {
            Self { batches }
        }
    }

    #[async_trait]
    impl EventSource for ScriptedSource {
        async fn next(&mut self) -> Vec<ChangeSignal> {
            if self.batches.is_empty() {
                Vec::new()
            } else {
                self.batches.remove(0)
            }
        }
    }

    fn signal(node_id: &str, updated_at: i64) -> ChangeSignal {
        ChangeSignal::new(
            EntityId::new(EntityKind::PullRequest, node_id),
            Timestamp(updated_at),
        )
    }

    async fn drain(sources: Vec<&mut dyn EventSource>) -> Vec<ChangeSignal> {
        let mut collected = Vec::new();
        for source in sources {
            collected.extend(source.next().await);
        }
        merge_deduplicated(collected)
    }

    #[tokio::test]
    async fn an_exhausted_source_reports_no_change() {
        let mut source = ScriptedSource::new(Vec::new());
        assert!(source.next().await.is_empty());
    }

    #[tokio::test]
    async fn a_source_that_yields_an_empty_batch_does_not_break_the_merge() {
        let mut idle = ScriptedSource::new(vec![Vec::new()]);
        let mut active = ScriptedSource::new(vec![vec![signal("PR_1", 100)]]);
        assert_eq!(
            drain(vec![&mut idle, &mut active]).await,
            vec![signal("PR_1", 100)]
        );
    }

    #[tokio::test]
    async fn a_scheduler_merges_two_sources_without_knowing_their_channel() {
        let mut polled = ScriptedSource::new(vec![vec![signal("PR_1", 100), signal("PR_2", 100)]]);
        let mut pushed = ScriptedSource::new(vec![vec![signal("PR_2", 100), signal("PR_3", 200)]]);
        assert_eq!(
            drain(vec![&mut polled, &mut pushed]).await,
            vec![
                signal("PR_1", 100),
                signal("PR_2", 100),
                signal("PR_3", 200)
            ]
        );
    }

    #[tokio::test]
    async fn a_boxed_source_is_usable_behind_a_trait_object() {
        let mut source: Box<dyn EventSource> =
            Box::new(ScriptedSource::new(vec![vec![signal("PR_1", 100)]]));
        assert_eq!(source.next().await, vec![signal("PR_1", 100)]);
    }
}
