use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct UserPayload {
    pub login: String,
    pub id: u64,
    pub node_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrganizationPayload {
    pub login: String,
    pub id: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationPayload {
    pub id: String,
    pub unread: bool,
    pub updated_at: String,
    pub reason: String,
    pub subject: NotificationSubjectPayload,
    pub repository: NotificationRepositoryPayload,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationSubjectPayload {
    pub title: String,
    pub url: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationRepositoryPayload {
    pub full_name: String,
    pub node_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorPayload {
    pub message: String,
    pub documentation_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{ErrorPayload, NotificationPayload, OrganizationPayload, UserPayload};

    #[test]
    fn a_truncated_user_payload_fails_to_deserialise_rather_than_producing_a_blank_identity() {
        assert!(serde_json::from_str::<UserPayload>("{\"login\": \"octo").is_err());
        assert!(serde_json::from_str::<UserPayload>("{\"login\": \"octocat\"}").is_err());
    }

    #[test]
    fn a_user_payload_ignores_the_fields_the_domain_does_not_need() {
        let payload: UserPayload = match serde_json::from_str(
            "{\"login\":\"octocat\",\"id\":1,\"node_id\":\"U_1\",\"avatar_url\":\"https://x\"}",
        ) {
            Ok(payload) => payload,
            Err(error) => panic!("the payload must deserialise: {error}"),
        };
        assert_eq!(payload.login, "octocat");
        assert_eq!(payload.node_id, "U_1");
    }

    #[test]
    fn an_empty_organization_list_deserialises_to_no_organization() {
        let payload: Vec<OrganizationPayload> = match serde_json::from_str("[]") {
            Ok(payload) => payload,
            Err(error) => panic!("an empty list is valid: {error}"),
        };
        assert!(payload.is_empty());
    }

    #[test]
    fn a_notification_payload_keeps_the_subject_type_under_a_rust_name() {
        let payload: NotificationPayload = match serde_json::from_str(
            "{\"id\":\"1\",\"unread\":true,\"updated_at\":\"2026-09-04T10:00:00Z\",\
              \"reason\":\"review_requested\",\
              \"subject\":{\"title\":\"Fix\",\"url\":null,\"type\":\"PullRequest\"},\
              \"repository\":{\"full_name\":\"acme/api\",\"node_id\":\"R_1\"}}",
        ) {
            Ok(payload) => payload,
            Err(error) => panic!("the payload must deserialise: {error}"),
        };
        assert_eq!(payload.subject.kind, "PullRequest");
        assert_eq!(payload.repository.full_name, "acme/api");
        assert_eq!(payload.reason, "review_requested");
    }

    #[test]
    fn an_error_payload_carries_the_message_github_wants_shown() {
        let payload: ErrorPayload = match serde_json::from_str(
            "{\"message\":\"Bad credentials\",\"documentation_url\":null}",
        ) {
            Ok(payload) => payload,
            Err(error) => panic!("the payload must deserialise: {error}"),
        };
        assert_eq!(payload.message, "Bad credentials");
    }
}
