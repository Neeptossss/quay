use quay_core::{
    Priority, PullRequest, PullRequestSnapshot, PullRequestState, Repository, ReviewComment,
    ReviewRequest, ReviewThread, TimelineEvent, TimelineKind,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::ForgeError;
use crate::governor::{OutboundRequest, RateGovernor};

const THREADS_PER_PAGE: i64 = 50;
const COMMENTS_PER_PAGE: i64 = 25;
const REVIEWERS_PER_PAGE: i64 = 50;
const TIMELINE_ITEMS: i64 = 60;
const ABBREVIATED_OID: usize = 7;
const MISSING_ACTOR: &str = "ghost";

const DETAIL_QUERY: &str = "\
query PrDetail($owner: String!, $name: String!, $number: Int!, $threads: Int!, $comments: Int!, $reviewers: Int!, $events: Int!, $head: Boolean!, $after: String) {
  repository(owner: $owner, name: $name) {
    id
    name
    owner { login }
    defaultBranchRef { name }
    pullRequest(number: $number) {
      id number title state isDraft mergeable reviewDecision updatedAt
      baseRefName headRefOid
      author { login }
      reviewRequests(first: $reviewers) {
        nodes {
          requestedReviewer {
            __typename
            ... on User { login }
            ... on Team { slug }
          }
        }
      }
      commits(last: 1) { nodes { commit { statusCheckRollup { state } } } }
      timelineItems(last: $events, itemTypes: [PULL_REQUEST_COMMIT, ISSUE_COMMENT, PULL_REQUEST_REVIEW, REVIEW_REQUESTED_EVENT, READY_FOR_REVIEW_EVENT, MERGED_EVENT, CLOSED_EVENT, REOPENED_EVENT, HEAD_REF_FORCE_PUSHED_EVENT]) @include(if: $head) {
        nodes {
          __typename
          ... on PullRequestCommit {
            id
            commit { oid messageHeadline committedDate author { name user { login } } }
          }
          ... on IssueComment { id body createdAt author { login } }
          ... on PullRequestReview { id body state createdAt author { login } }
          ... on ReviewRequestedEvent {
            id createdAt actor { login }
            requestedReviewer { __typename ... on User { login } ... on Team { slug } }
          }
          ... on ReadyForReviewEvent { id createdAt actor { login } }
          ... on MergedEvent { id createdAt actor { login } commit { oid } }
          ... on ClosedEvent { id createdAt actor { login } }
          ... on ReopenedEvent { id createdAt actor { login } }
          ... on HeadRefForcePushedEvent { id createdAt actor { login } afterCommit { oid } }
        }
      }
      reviewThreads(first: $threads, after: $after) {
        totalCount
        pageInfo { hasNextPage endCursor }
        nodes {
          id isResolved isOutdated path line originalLine diffSide
          comments(first: $comments) {
            nodes { id body createdAt author { login } }
          }
        }
      }
    }
  }
}";

#[derive(Debug, Deserialize)]
struct Envelope {
    data: Option<EnvelopeData>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Debug, Deserialize)]
struct EnvelopeData {
    repository: Option<RepositoryNode>,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepositoryNode {
    id: String,
    name: String,
    owner: ActorNode,
    default_branch_ref: Option<RefNode>,
    pull_request: Option<PullRequestNode>,
}

#[derive(Debug, Deserialize)]
struct RefNode {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ActorNode {
    login: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestNode {
    id: String,
    number: i64,
    title: String,
    state: String,
    is_draft: bool,
    mergeable: Option<String>,
    review_decision: Option<String>,
    updated_at: String,
    base_ref_name: String,
    head_ref_oid: String,
    author: Option<ActorNode>,
    review_requests: ReviewRequestConnection,
    commits: CommitConnection,
    timeline_items: Option<TimelineItemConnection>,
    review_threads: ReviewThreadConnection,
}

#[derive(Debug, Deserialize)]
struct TimelineItemConnection {
    nodes: Vec<TimelineItemNode>,
}

#[derive(Debug, Deserialize)]
struct OidNode {
    oid: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitSummary {
    oid: String,
    message_headline: String,
    committed_date: String,
    author: Option<CommitAuthorNode>,
}

#[derive(Debug, Deserialize)]
struct CommitAuthorNode {
    name: Option<String>,
    user: Option<ActorNode>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__typename")]
enum TimelineItemNode {
    PullRequestCommit {
        id: String,
        commit: CommitSummary,
    },
    #[serde(rename_all = "camelCase")]
    IssueComment {
        id: String,
        body: String,
        created_at: String,
        author: Option<ActorNode>,
    },
    #[serde(rename_all = "camelCase")]
    PullRequestReview {
        id: String,
        body: String,
        state: String,
        created_at: String,
        author: Option<ActorNode>,
    },
    #[serde(rename_all = "camelCase")]
    ReviewRequestedEvent {
        id: String,
        created_at: String,
        actor: Option<ActorNode>,
        requested_reviewer: Option<RequestedReviewer>,
    },
    #[serde(rename_all = "camelCase")]
    ReadyForReviewEvent {
        id: String,
        created_at: String,
        actor: Option<ActorNode>,
    },
    #[serde(rename_all = "camelCase")]
    MergedEvent {
        id: String,
        created_at: String,
        actor: Option<ActorNode>,
        commit: Option<OidNode>,
    },
    #[serde(rename_all = "camelCase")]
    ClosedEvent {
        id: String,
        created_at: String,
        actor: Option<ActorNode>,
    },
    #[serde(rename_all = "camelCase")]
    ReopenedEvent {
        id: String,
        created_at: String,
        actor: Option<ActorNode>,
    },
    #[serde(rename_all = "camelCase")]
    HeadRefForcePushedEvent {
        id: String,
        created_at: String,
        actor: Option<ActorNode>,
        after_commit: Option<OidNode>,
    },
    #[serde(other)]
    Unrecognised,
}

#[derive(Debug, Deserialize)]
struct ReviewRequestConnection {
    nodes: Vec<ReviewRequestNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewRequestNode {
    requested_reviewer: Option<RequestedReviewer>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__typename")]
enum RequestedReviewer {
    User {
        login: String,
    },
    Team {
        slug: String,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct CommitConnection {
    nodes: Vec<CommitNode>,
}

#[derive(Debug, Deserialize)]
struct CommitNode {
    commit: CommitDetail,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitDetail {
    status_check_rollup: Option<RollupNode>,
}

#[derive(Debug, Deserialize)]
struct RollupNode {
    state: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadConnection {
    page_info: PageInfo,
    nodes: Vec<ReviewThreadNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadNode {
    id: String,
    is_resolved: bool,
    is_outdated: bool,
    path: String,
    line: Option<i64>,
    original_line: Option<i64>,
    diff_side: Option<String>,
    comments: ReviewCommentConnection,
}

#[derive(Debug, Deserialize)]
struct ReviewCommentConnection {
    nodes: Vec<ReviewCommentNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewCommentNode {
    id: String,
    body: String,
    created_at: String,
    author: Option<ActorNode>,
}

pub async fn fetch(
    governor: &RateGovernor,
    endpoint: &str,
    owner: &str,
    name: &str,
    number: i64,
    priority: Priority,
) -> Result<PullRequestSnapshot, ForgeError> {
    let mut pages: Vec<Value> = Vec::new();
    let mut cursor: Option<String> = None;
    let mut threads: Vec<ReviewThread> = Vec::new();
    let mut head: Option<(RepositoryNode, PullRequestNode)> = None;
    let mut events: Vec<TimelineEvent> = Vec::new();

    loop {
        let first_pass = cursor.is_none();
        let body = json!({
            "query": DETAIL_QUERY,
            "variables": {
                "owner": owner,
                "name": name,
                "number": number,
                "threads": THREADS_PER_PAGE,
                "comments": COMMENTS_PER_PAGE,
                "reviewers": REVIEWERS_PER_PAGE,
                "events": TIMELINE_ITEMS,
                "head": first_pass,
                "after": cursor,
            }
        });
        let response = governor
            .send(OutboundRequest::graphql_query(
                endpoint,
                body.to_string(),
                priority,
            ))
            .await?;
        if response.status != 200 {
            return Err(ForgeError::UnexpectedStatus {
                status: response.status,
                message: crate::auth::forge_message(&response.body),
            });
        }
        pages.push(
            serde_json::from_str(&response.body).unwrap_or_else(|_| json!({ "unparsed": true })),
        );

        let envelope: Envelope =
            serde_json::from_str(&response.body).map_err(|error| ForgeError::MalformedPayload {
                message: error.to_string(),
            })?;
        if let Some(errors) = envelope.errors.filter(|errors| !errors.is_empty()) {
            return Err(ForgeError::ForgeRefusedQuery {
                messages: errors.into_iter().map(|error| error.message).collect(),
            });
        }
        let repository = envelope
            .data
            .and_then(|data| data.repository)
            .ok_or_else(|| ForgeError::MissingResource {
                what: format!("{owner}/{name}"),
            })?;
        let mut repository = repository;
        let pull_request =
            repository
                .pull_request
                .take()
                .ok_or_else(|| ForgeError::MissingResource {
                    what: format!("{owner}/{name}#{number}"),
                })?;

        if first_pass && let Some(timeline) = &pull_request.timeline_items {
            events.extend(timeline.nodes.iter().filter_map(read_event));
        }
        threads.extend(pull_request.review_threads.nodes.iter().map(read_thread));
        let has_next_page = pull_request.review_threads.page_info.has_next_page;
        cursor = pull_request.review_threads.page_info.end_cursor.clone();
        if head.is_none() {
            head = Some((repository, pull_request));
        }
        if !has_next_page || cursor.is_none() {
            break;
        }
    }

    let (repository, pull_request) = head.ok_or_else(|| ForgeError::MissingResource {
        what: format!("{owner}/{name}#{number}"),
    })?;
    let raw = serde_json::to_vec(&pages).unwrap_or_default();
    assemble(repository, pull_request, threads, events, raw)
}

fn assemble(
    repository: RepositoryNode,
    pull_request: PullRequestNode,
    threads: Vec<ReviewThread>,
    events: Vec<TimelineEvent>,
    raw: Vec<u8>,
) -> Result<PullRequestSnapshot, ForgeError> {
    let state = PullRequestState::parse(&pull_request.state).ok_or_else(|| {
        ForgeError::MalformedPayload {
            message: format!("unknown pull request state {}", pull_request.state),
        }
    })?;

    let review_requests = pull_request
        .review_requests
        .nodes
        .into_iter()
        .filter_map(|node| match node.requested_reviewer? {
            RequestedReviewer::User { login } => Some(ReviewRequest {
                reviewer: login,
                is_team: false,
                requested_at: pull_request.updated_at.clone(),
            }),
            RequestedReviewer::Team { slug } => Some(ReviewRequest {
                reviewer: slug,
                is_team: true,
                requested_at: pull_request.updated_at.clone(),
            }),
            RequestedReviewer::Unknown => None,
        })
        .collect();

    let checks_state = pull_request
        .commits
        .nodes
        .first()
        .and_then(|node| node.commit.status_check_rollup.as_ref())
        .map(|rollup| rollup.state.to_lowercase());

    Ok(PullRequestSnapshot {
        repository: Repository {
            node_id: repository.id,
            owner: repository.owner.login,
            name: repository.name,
            default_branch: repository
                .default_branch_ref
                .map(|reference| reference.name),
        },
        pull_request: PullRequest {
            node_id: pull_request.id,
            number: pull_request.number,
            title: pull_request.title,
            state,
            is_draft: pull_request.is_draft,
            author: pull_request
                .author
                .map(|actor| actor.login)
                .unwrap_or_else(|| "ghost".to_owned()),
            base_ref: pull_request.base_ref_name,
            head_sha: pull_request.head_ref_oid,
            review_state: pull_request
                .review_decision
                .map(|decision| decision.to_lowercase()),
            checks_state,
            mergeable: pull_request
                .mergeable
                .map(|mergeable| mergeable.to_lowercase()),
            updated_at: pull_request.updated_at,
        },
        threads,
        review_requests,
        events,
        raw,
    })
}

fn read_thread(node: &ReviewThreadNode) -> ReviewThread {
    ReviewThread {
        node_id: node.id.clone(),
        path: node.path.clone(),
        line: node.line,
        side: node.diff_side.clone(),
        original_line: node.original_line,
        diff_hunk: None,
        is_resolved: node.is_resolved,
        is_outdated: node.is_outdated,
        comments: node
            .comments
            .nodes
            .iter()
            .map(|comment| ReviewComment {
                node_id: comment.id.clone(),
                author: comment
                    .author
                    .as_ref()
                    .map(|actor| actor.login.clone())
                    .unwrap_or_else(|| "ghost".to_owned()),
                body: comment.body.clone(),
                created_at: comment.created_at.clone(),
            })
            .collect(),
    }
}

fn login_of(actor: &Option<ActorNode>) -> String {
    actor
        .as_ref()
        .map(|actor| actor.login.clone())
        .unwrap_or_else(|| MISSING_ACTOR.to_owned())
}

fn spoken_body(body: &str) -> Option<String> {
    let trimmed = body.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn abbreviated(oid: &str) -> String {
    oid.chars().take(ABBREVIATED_OID).collect()
}

fn read_event(node: &TimelineItemNode) -> Option<TimelineEvent> {
    match node {
        TimelineItemNode::PullRequestCommit { id, commit } => Some(TimelineEvent {
            node_id: id.clone(),
            kind: TimelineKind::Commit,
            actor: commit
                .author
                .as_ref()
                .and_then(|author| {
                    author
                        .user
                        .as_ref()
                        .map(|user| user.login.clone())
                        .or_else(|| author.name.clone())
                })
                .unwrap_or_else(|| MISSING_ACTOR.to_owned()),
            body: spoken_body(&commit.message_headline),
            reference: Some(abbreviated(&commit.oid)),
            created_at: commit.committed_date.clone(),
        }),
        TimelineItemNode::IssueComment {
            id,
            body,
            created_at,
            author,
        } => Some(TimelineEvent {
            node_id: id.clone(),
            kind: TimelineKind::Comment,
            actor: login_of(author),
            body: spoken_body(body),
            reference: None,
            created_at: created_at.clone(),
        }),
        TimelineItemNode::PullRequestReview {
            id,
            body,
            state,
            created_at,
            author,
        } => {
            let state = state.to_lowercase();
            if state == "pending" {
                return None;
            }
            Some(TimelineEvent {
                node_id: id.clone(),
                kind: TimelineKind::Review,
                actor: login_of(author),
                body: spoken_body(body),
                reference: Some(state),
                created_at: created_at.clone(),
            })
        }
        TimelineItemNode::ReviewRequestedEvent {
            id,
            created_at,
            actor,
            requested_reviewer,
        } => Some(TimelineEvent {
            node_id: id.clone(),
            kind: TimelineKind::ReviewRequested,
            actor: login_of(actor),
            body: None,
            reference: match requested_reviewer {
                Some(RequestedReviewer::User { login }) => Some(login.clone()),
                Some(RequestedReviewer::Team { slug }) => Some(slug.clone()),
                _ => None,
            },
            created_at: created_at.clone(),
        }),
        TimelineItemNode::ReadyForReviewEvent {
            id,
            created_at,
            actor,
        } => Some(TimelineEvent {
            node_id: id.clone(),
            kind: TimelineKind::ReadyForReview,
            actor: login_of(actor),
            body: None,
            reference: None,
            created_at: created_at.clone(),
        }),
        TimelineItemNode::MergedEvent {
            id,
            created_at,
            actor,
            commit,
        } => Some(TimelineEvent {
            node_id: id.clone(),
            kind: TimelineKind::Merged,
            actor: login_of(actor),
            body: None,
            reference: commit.as_ref().map(|commit| abbreviated(&commit.oid)),
            created_at: created_at.clone(),
        }),
        TimelineItemNode::ClosedEvent {
            id,
            created_at,
            actor,
        } => Some(TimelineEvent {
            node_id: id.clone(),
            kind: TimelineKind::Closed,
            actor: login_of(actor),
            body: None,
            reference: None,
            created_at: created_at.clone(),
        }),
        TimelineItemNode::ReopenedEvent {
            id,
            created_at,
            actor,
        } => Some(TimelineEvent {
            node_id: id.clone(),
            kind: TimelineKind::Reopened,
            actor: login_of(actor),
            body: None,
            reference: None,
            created_at: created_at.clone(),
        }),
        TimelineItemNode::HeadRefForcePushedEvent {
            id,
            created_at,
            actor,
            after_commit,
        } => Some(TimelineEvent {
            node_id: id.clone(),
            kind: TimelineKind::ForcePush,
            actor: login_of(actor),
            body: None,
            reference: after_commit.as_ref().map(|commit| abbreviated(&commit.oid)),
            created_at: created_at.clone(),
        }),
        TimelineItemNode::Unrecognised => None,
    }
}
