use quay_core::TimelineKind;
use rusqlite::Connection;

use crate::error::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentView {
    pub author: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadView {
    pub node_id: String,
    pub path: String,
    pub line: Option<i64>,
    pub is_resolved: bool,
    pub is_outdated: bool,
    pub opened_at: String,
    pub comments: Vec<CommentView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventView {
    pub node_id: String,
    pub kind: TimelineKind,
    pub actor: String,
    pub body: Option<String>,
    pub reference: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedItem {
    Event(EventView),
    Thread(ThreadView),
}

impl FeedItem {
    pub fn happened_at(&self) -> &str {
        match self {
            FeedItem::Event(event) => &event.created_at,
            FeedItem::Thread(thread) => &thread.opened_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestView {
    pub owner: String,
    pub name: String,
    pub number: i64,
    pub node_id: String,
    pub title: String,
    pub state: String,
    pub is_draft: bool,
    pub author: String,
    pub base_ref: String,
    pub head_sha: String,
    pub review_state: Option<String>,
    pub checks_state: Option<String>,
    pub updated_at: String,
    pub threads: Vec<ThreadView>,
    pub events: Vec<EventView>,
}

impl PullRequestView {
    pub fn unresolved_threads(&self) -> usize {
        self.threads
            .iter()
            .filter(|thread| !thread.is_resolved)
            .count()
    }

    pub fn feed(&self) -> Vec<FeedItem> {
        let mut items: Vec<FeedItem> = self
            .events
            .iter()
            .filter(|event| !is_the_empty_shell_of_an_inline_review(event))
            .cloned()
            .map(FeedItem::Event)
            .chain(self.threads.iter().cloned().map(FeedItem::Thread))
            .collect();
        items.sort_by(|left, right| left.happened_at().cmp(right.happened_at()));
        items
    }
}

fn is_the_empty_shell_of_an_inline_review(event: &EventView) -> bool {
    event.kind == TimelineKind::Review
        && event.body.is_none()
        && event.reference.as_deref() == Some("commented")
}

pub fn pull_request(
    connection: &Connection,
    owner: &str,
    name: &str,
    number: i64,
) -> Result<Option<PullRequestView>, StoreError> {
    let mut statement = connection.prepare_cached(
        "SELECT pr.id, pr.node_id, pr.title, pr.state, pr.is_draft, pr.author,
                pr.base_ref, pr.head_sha, pr.review_state, pr.checks_state, pr.updated_at
         FROM pull_request pr JOIN repo r ON r.id = pr.repo_id
         WHERE r.owner = ?1 AND r.name = ?2 AND pr.number = ?3",
    )?;
    let mut rows = statement.query_map((owner, name, number), |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, Option<String>>(8)?,
            row.get::<_, Option<String>>(9)?,
            row.get::<_, String>(10)?,
        ))
    })?;
    let Some(found) = rows.next() else {
        return Ok(None);
    };
    let (
        id,
        node_id,
        title,
        state,
        is_draft,
        author,
        base_ref,
        head_sha,
        review_state,
        checks_state,
        updated_at,
    ) = found?;
    drop(rows);

    Ok(Some(PullRequestView {
        owner: owner.to_owned(),
        name: name.to_owned(),
        number,
        node_id,
        title,
        state,
        is_draft: is_draft != 0,
        author,
        base_ref,
        head_sha,
        review_state,
        checks_state,
        updated_at,
        threads: threads_of(connection, id)?,
        events: events_of(connection, id)?,
    }))
}

fn threads_of(
    connection: &Connection,
    pull_request_id: i64,
) -> Result<Vec<ThreadView>, StoreError> {
    let mut statement = connection.prepare_cached(
        "SELECT id, node_id, path, line, is_resolved, is_outdated
         FROM review_thread WHERE pr_id = ?1 ORDER BY is_resolved, path, id",
    )?;
    let rows = statement
        .query_map((pull_request_id,), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?
        .collect::<Result<Vec<(i64, String, String, Option<i64>, i64, i64)>, rusqlite::Error>>()?;

    let mut threads = Vec::with_capacity(rows.len());
    for (id, node_id, path, line, is_resolved, is_outdated) in rows {
        let comments = comments_of(connection, id)?;
        threads.push(ThreadView {
            node_id,
            path,
            line,
            is_resolved: is_resolved != 0,
            is_outdated: is_outdated != 0,
            opened_at: comments
                .first()
                .map(|comment| comment.created_at.clone())
                .unwrap_or_default(),
            comments,
        });
    }
    Ok(threads)
}

fn comments_of(connection: &Connection, thread_id: i64) -> Result<Vec<CommentView>, StoreError> {
    let mut statement = connection.prepare_cached(
        "SELECT author, body, created_at FROM review_comment WHERE thread_id = ?1
         ORDER BY created_at, id",
    )?;
    Ok(statement
        .query_map((thread_id,), |row| {
            Ok(CommentView {
                author: row.get(0)?,
                body: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<CommentView>, rusqlite::Error>>()?)
}

fn events_of(connection: &Connection, pull_request_id: i64) -> Result<Vec<EventView>, StoreError> {
    let mut statement = connection.prepare_cached(
        "SELECT node_id, kind, actor, body, reference, created_at
         FROM timeline_event WHERE pr_id = ?1 ORDER BY created_at, id",
    )?;
    let rows = statement
        .query_map((pull_request_id,), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?
        .collect::<Result<
            Vec<(
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                String,
            )>,
            rusqlite::Error,
        >>()?;

    Ok(rows
        .into_iter()
        .filter_map(|(node_id, kind, actor, body, reference, created_at)| {
            Some(EventView {
                node_id,
                kind: TimelineKind::parse(&kind)?,
                actor,
                body,
                reference,
                created_at,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{CommentView, EventView, FeedItem, PullRequestView, ThreadView};
    use quay_core::TimelineKind;

    fn event(
        kind: TimelineKind,
        at: &str,
        reference: Option<&str>,
        body: Option<&str>,
    ) -> EventView {
        EventView {
            node_id: format!("{}-{at}", kind.id()),
            kind,
            actor: "octocat".to_owned(),
            body: body.map(str::to_owned),
            reference: reference.map(str::to_owned),
            created_at: at.to_owned(),
        }
    }

    fn thread(path: &str, comments: &[&str]) -> ThreadView {
        ThreadView {
            node_id: format!("thread-{path}"),
            path: path.to_owned(),
            line: Some(12),
            is_resolved: false,
            is_outdated: false,
            opened_at: comments
                .first()
                .map(|at| (*at).to_owned())
                .unwrap_or_default(),
            comments: comments
                .iter()
                .map(|at| CommentView {
                    author: "octocat".to_owned(),
                    body: "nit".to_owned(),
                    created_at: (*at).to_owned(),
                })
                .collect(),
        }
    }

    fn view(threads: Vec<ThreadView>, events: Vec<EventView>) -> PullRequestView {
        PullRequestView {
            owner: "acme".to_owned(),
            name: "api".to_owned(),
            number: 7,
            node_id: "PR_1".to_owned(),
            title: "Tighten the governor".to_owned(),
            state: "open".to_owned(),
            is_draft: false,
            author: "octocat".to_owned(),
            base_ref: "main".to_owned(),
            head_sha: "abcdef1234567890".to_owned(),
            review_state: None,
            checks_state: None,
            updated_at: "2024-05-04T00:00:00Z".to_owned(),
            threads,
            events,
        }
    }

    fn ordering(view: &PullRequestView) -> Vec<String> {
        view.feed()
            .into_iter()
            .map(|item| match item {
                FeedItem::Event(event) => format!("{}@{}", event.kind.id(), event.created_at),
                FeedItem::Thread(thread) => format!("thread@{}", thread.opened_at),
            })
            .collect()
    }

    #[test]
    fn a_review_thread_appears_where_its_first_comment_landed_not_at_the_end() {
        let subject = view(
            vec![thread("src/lib.rs", &["2024-05-02T00:00:00Z"])],
            vec![
                event(
                    TimelineKind::Commit,
                    "2024-05-01T00:00:00Z",
                    None,
                    Some("init"),
                ),
                event(TimelineKind::Merged, "2024-05-03T00:00:00Z", None, None),
            ],
        );
        assert_eq!(
            ordering(&subject),
            vec![
                "commit@2024-05-01T00:00:00Z",
                "thread@2024-05-02T00:00:00Z",
                "merged@2024-05-03T00:00:00Z",
            ]
        );
    }

    #[test]
    fn a_review_that_only_carried_inline_comments_is_not_repeated_as_an_empty_entry() {
        let subject = view(
            vec![thread("src/lib.rs", &["2024-05-02T00:00:00Z"])],
            vec![event(
                TimelineKind::Review,
                "2024-05-02T00:00:00Z",
                Some("commented"),
                None,
            )],
        );
        assert_eq!(ordering(&subject), vec!["thread@2024-05-02T00:00:00Z"]);
    }

    #[test]
    fn a_review_that_approves_without_a_word_stays_in_the_feed() {
        let subject = view(
            Vec::new(),
            vec![event(
                TimelineKind::Review,
                "2024-05-02T00:00:00Z",
                Some("approved"),
                None,
            )],
        );
        assert_eq!(ordering(&subject), vec!["review@2024-05-02T00:00:00Z"]);
    }

    #[test]
    fn a_review_that_says_something_while_commenting_stays_in_the_feed() {
        let subject = view(
            Vec::new(),
            vec![event(
                TimelineKind::Review,
                "2024-05-02T00:00:00Z",
                Some("commented"),
                Some("looks good overall"),
            )],
        );
        assert_eq!(ordering(&subject), vec!["review@2024-05-02T00:00:00Z"]);
    }

    #[test]
    fn a_thread_we_cannot_date_is_shown_rather_than_dropped() {
        let subject = view(
            vec![thread("src/lib.rs", &[])],
            vec![event(
                TimelineKind::Commit,
                "2024-05-01T00:00:00Z",
                None,
                None,
            )],
        );
        assert_eq!(
            ordering(&subject),
            vec!["thread@", "commit@2024-05-01T00:00:00Z"]
        );
    }

    #[test]
    fn an_empty_pull_request_has_an_empty_feed_rather_than_a_placeholder() {
        assert!(view(Vec::new(), Vec::new()).feed().is_empty());
    }

    #[test]
    fn the_feed_carries_every_thread_including_the_resolved_ones() {
        let mut resolved = thread("src/old.rs", &["2024-05-01T00:00:00Z"]);
        resolved.is_resolved = true;
        let subject = view(
            vec![resolved, thread("src/new.rs", &["2024-05-02T00:00:00Z"])],
            Vec::new(),
        );
        assert_eq!(subject.feed().len(), 2);
        assert_eq!(subject.unresolved_threads(), 1);
    }
}
