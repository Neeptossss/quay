use quay_core::{MutationKind, Priority};
use reqwest::Method;
use serde_json::{Value, json};

use crate::error::ForgeError;
use crate::governor::OutboundRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationTarget {
    pub owner: String,
    pub name: String,
    pub number: i64,
    pub node_id: String,
}

impl MutationTarget {
    pub fn repository(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

pub fn build(
    api_base: &str,
    kind: MutationKind,
    target: &MutationTarget,
    payload: &str,
) -> Result<OutboundRequest, ForgeError> {
    let payload: Value = serde_json::from_str(payload).unwrap_or_else(|_| json!({}));
    let body = payload
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let api_base = api_base.trim_end_matches('/');
    let repository = target.repository();

    let request = match kind {
        MutationKind::ApprovePullRequest => OutboundRequest::rest_write(
            format!(
                "{api_base}/repos/{repository}/pulls/{}/reviews",
                target.number
            ),
            json!({ "event": "APPROVE" }).to_string(),
            repository,
            Priority::User,
        ),
        MutationKind::RequestChanges => OutboundRequest::rest_write(
            format!(
                "{api_base}/repos/{repository}/pulls/{}/reviews",
                target.number
            ),
            json!({ "event": "REQUEST_CHANGES", "body": body }).to_string(),
            repository,
            Priority::User,
        ),
        MutationKind::PostComment => OutboundRequest::rest_write(
            format!(
                "{api_base}/repos/{repository}/issues/{}/comments",
                target.number
            ),
            json!({ "body": body }).to_string(),
            repository,
            Priority::User,
        ),
        MutationKind::MergePullRequest => OutboundRequest::rest_write(
            format!(
                "{api_base}/repos/{repository}/pulls/{}/merge",
                target.number
            ),
            json!({}).to_string(),
            repository,
            Priority::User,
        )
        .with_method(Method::PUT),
        MutationKind::ResolveThread => {
            graphql_thread_mutation(api_base, "resolveReviewThread", &target.node_id, repository)
        }
        MutationKind::UnresolveThread => graphql_thread_mutation(
            api_base,
            "unresolveReviewThread",
            &target.node_id,
            repository,
        ),
    };
    Ok(request)
}

fn graphql_thread_mutation(
    api_base: &str,
    field: &str,
    thread_node_id: &str,
    repository: String,
) -> OutboundRequest {
    let document = format!(
        "mutation ThreadState($threadId: ID!) {{ \
           {field}(input: {{ threadId: $threadId }}) {{ thread {{ id isResolved }} }} }}"
    );
    OutboundRequest::graphql_mutation(
        format!("{api_base}/graphql"),
        json!({ "query": document, "variables": { "threadId": thread_node_id } }).to_string(),
        repository,
        Priority::User,
    )
}

#[cfg(test)]
mod tests {
    use quay_core::MutationKind;

    use super::{MutationTarget, build};
    use crate::governor::RequestKind;

    fn target() -> MutationTarget {
        MutationTarget {
            owner: "acme".to_owned(),
            name: "api".to_owned(),
            number: 42,
            node_id: "RT_1".to_owned(),
        }
    }

    fn built(kind: MutationKind, payload: &str) -> crate::governor::OutboundRequest {
        match build("https://api.github.com", kind, &target(), payload) {
            Ok(request) => request,
            Err(error) => panic!("the request must build: {error}"),
        }
    }

    #[test]
    fn every_mutation_names_its_repository_so_the_write_allowlist_can_judge_it() {
        for kind in MutationKind::ALL {
            let request = built(kind, "{}");
            assert_eq!(request.repo.as_deref(), Some("acme/api"), "{}", kind.id());
        }
    }

    #[test]
    fn every_mutation_is_declared_as_a_write_so_read_only_mode_can_stop_it() {
        for kind in MutationKind::ALL {
            assert!(built(kind, "{}").kind.is_write(), "{}", kind.id());
        }
    }

    #[test]
    fn approving_posts_a_review_carrying_the_approve_event() {
        let request = built(MutationKind::ApprovePullRequest, "{}");
        assert!(request.url.ends_with("/repos/acme/api/pulls/42/reviews"));
        assert_eq!(request.kind, RequestKind::RestWrite);
        match request.body {
            Some(body) => assert!(body.contains("APPROVE")),
            None => panic!("the review event must be sent"),
        }
    }

    #[test]
    fn a_comment_carries_the_body_the_user_typed() {
        let request = built(MutationKind::PostComment, "{\"body\":\"needs a test\"}");
        assert!(request.url.ends_with("/repos/acme/api/issues/42/comments"));
        match request.body {
            Some(body) => assert!(body.contains("needs a test")),
            None => panic!("the comment body must be sent"),
        }
    }

    #[test]
    fn a_merge_uses_put_because_the_forge_refuses_a_post_there() {
        assert_eq!(built(MutationKind::MergePullRequest, "{}").method, "PUT");
    }

    #[test]
    fn resolving_a_thread_travels_as_a_graphql_mutation_naming_the_thread() {
        let request = built(MutationKind::ResolveThread, "{}");
        assert_eq!(request.kind, RequestKind::GraphQlMutation);
        assert!(request.url.ends_with("/graphql"));
        match request.body {
            Some(body) => {
                assert!(body.contains("resolveReviewThread"));
                assert!(body.contains("RT_1"));
            }
            None => panic!("the mutation document must be sent"),
        }
    }

    #[test]
    fn a_payload_that_is_not_json_still_produces_a_usable_request() {
        let request = built(MutationKind::ApprovePullRequest, "not json at all");
        assert!(request.body.is_some());
    }
}
