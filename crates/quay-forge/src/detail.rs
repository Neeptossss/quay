use quay_core::{
    Priority, PullRequest, PullRequestSnapshot, PullRequestState, Repository, ReviewComment,
    ReviewRequest, ReviewThread,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::ForgeError;
use crate::governor::{OutboundRequest, RateGovernor};

const THREADS_PER_PAGE: i64 = 50;
const COMMENTS_PER_PAGE: i64 = 25;
const REVIEWERS_PER_PAGE: i64 = 50;

const DETAIL_QUERY: &str = "\
query PrDetail($owner: String!, $name: String!, $number: Int!, $threads: Int!, $comments: Int!, $reviewers: Int!, $after: String) {
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
    review_threads: ReviewThreadConnection,
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

    loop {
        let body = json!({
            "query": DETAIL_QUERY,
            "variables": {
                "owner": owner,
                "name": name,
                "number": number,
                "threads": THREADS_PER_PAGE,
                "comments": COMMENTS_PER_PAGE,
                "reviewers": REVIEWERS_PER_PAGE,
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
    assemble(repository, pull_request, threads, raw)
}

fn assemble(
    repository: RepositoryNode,
    pull_request: PullRequestNode,
    threads: Vec<ReviewThread>,
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
