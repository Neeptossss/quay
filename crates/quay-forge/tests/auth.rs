use std::time::Duration;

use quay_core::Priority;
use quay_forge::{
    Capabilities, Capability, DevLocks, ForgeError, ForgeResponse, GovernorConfig, OutboundRequest,
    RateGovernor, SingleSignOn, Support, Token, TokenKind, read_identity,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const USER_BODY: &str = "{\"login\":\"octocat\",\"id\":1,\"node_id\":\"U_kgDOA\"}";
const ORGANIZATIONS_BODY: &str =
    "[{\"login\":\"acme\",\"id\":21},{\"login\":\"contoso\",\"id\":7}]";

fn governor() -> RateGovernor {
    match RateGovernor::new(
        GovernorConfig {
            request_timeout: Duration::from_millis(500),
            ..GovernorConfig::default()
        },
        Some(Token::new("ghp_testtoken")),
        DevLocks::from_settings(Some("1"), None),
    ) {
        Ok(governor) => governor,
        Err(error) => panic!("the governor must build: {error}"),
    }
}

async fn fetch(governor: &RateGovernor, server: &MockServer, suffix: &str) -> ForgeResponse {
    let request = OutboundRequest::rest_read(format!("{}{suffix}", server.uri()), Priority::User);
    match governor.send(request).await {
        Ok(response) => response,
        Err(error) => panic!("the request must reach the server: {error}"),
    }
}

async fn mount(server: &MockServer, suffix: &str, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path(suffix))
        .respond_with(response)
        .mount(server)
        .await;
}

#[tokio::test]
async fn a_rejected_token_is_reported_as_rejected_with_the_message_github_sent() {
    let server = MockServer::start().await;
    mount(
        &server,
        "/user",
        ResponseTemplate::new(401).set_body_string("{\"message\":\"Bad credentials\"}"),
    )
    .await;
    mount(
        &server,
        "/user/orgs",
        ResponseTemplate::new(200).set_body_string("[]"),
    )
    .await;

    let governor = governor();
    let user = fetch(&governor, &server, "/user").await;
    let organizations = fetch(&governor, &server, "/user/orgs").await;

    match read_identity(TokenKind::PatClassic, &user, &organizations) {
        Err(ForgeError::TokenRejected { message }) => assert_eq!(message, "Bad credentials"),
        other => panic!("a 401 must reject the token, got {other:?}"),
    }
}

#[tokio::test]
async fn an_organization_policy_refusal_is_told_apart_from_a_bad_token() {
    let server = MockServer::start().await;
    mount(
        &server,
        "/user",
        ResponseTemplate::new(200)
            .insert_header("x-oauth-scopes", "repo")
            .set_body_string(USER_BODY),
    )
    .await;
    mount(
        &server,
        "/user/orgs",
        ResponseTemplate::new(403)
            .insert_header("x-ratelimit-limit", "5000")
            .insert_header("x-ratelimit-remaining", "4900")
            .insert_header("x-ratelimit-reset", "1788000000")
            .set_body_string(
                "{\"message\":\"Personal access tokens are blocked by an enterprise policy\"}",
            ),
    )
    .await;

    let governor = governor();
    let user = fetch(&governor, &server, "/user").await;
    let organizations = fetch(&governor, &server, "/user/orgs").await;

    match read_identity(TokenKind::PatClassic, &user, &organizations) {
        Err(ForgeError::AccessForbidden { message }) => {
            assert!(message.contains("enterprise policy"));
        }
        other => panic!("a policy refusal must be forbidden access, got {other:?}"),
    }
}

#[tokio::test]
async fn a_truncated_payload_is_refused_rather_than_read_as_a_blank_identity() {
    let server = MockServer::start().await;
    mount(
        &server,
        "/user",
        ResponseTemplate::new(200)
            .insert_header("x-oauth-scopes", "repo")
            .set_body_string("{\"login\":\"octo"),
    )
    .await;
    mount(
        &server,
        "/user/orgs",
        ResponseTemplate::new(200).set_body_string("[]"),
    )
    .await;

    let governor = governor();
    let user = fetch(&governor, &server, "/user").await;
    let organizations = fetch(&governor, &server, "/user/orgs").await;

    assert!(matches!(
        read_identity(TokenKind::PatClassic, &user, &organizations),
        Err(ForgeError::MalformedPayload { .. })
    ));
}

#[tokio::test]
async fn a_valid_token_yields_the_login_the_organizations_and_the_granted_scopes() {
    let server = MockServer::start().await;
    mount(
        &server,
        "/user",
        ResponseTemplate::new(200)
            .insert_header("x-oauth-scopes", "repo, notifications, read:org")
            .set_body_string(USER_BODY),
    )
    .await;
    mount(
        &server,
        "/user/orgs",
        ResponseTemplate::new(200).set_body_string(ORGANIZATIONS_BODY),
    )
    .await;

    let governor = governor();
    let user = fetch(&governor, &server, "/user").await;
    let organizations = fetch(&governor, &server, "/user/orgs").await;

    match read_identity(TokenKind::PatClassic, &user, &organizations) {
        Ok(identity) => {
            assert_eq!(identity.login, "octocat");
            assert_eq!(identity.organizations, vec!["acme", "contoso"]);
            assert!(identity.scopes.contains("notifications"));
            assert_eq!(identity.single_sign_on, SingleSignOn::NoRestriction);
        }
        Err(error) => panic!("the identity must be readable: {error}"),
    }
}

#[tokio::test]
async fn an_organization_hidden_by_single_sign_on_is_detected_and_never_silently_dropped() {
    let server = MockServer::start().await;
    mount(
        &server,
        "/user",
        ResponseTemplate::new(200)
            .insert_header("x-oauth-scopes", "repo, notifications, read:org")
            .set_body_string(USER_BODY),
    )
    .await;
    mount(
        &server,
        "/user/orgs",
        ResponseTemplate::new(200)
            .insert_header("x-github-sso", "partial-results; organizations=42")
            .set_body_string("[{\"login\":\"acme\",\"id\":21}]"),
    )
    .await;

    let governor = governor();
    let user = fetch(&governor, &server, "/user").await;
    let organizations = fetch(&governor, &server, "/user/orgs").await;

    match read_identity(TokenKind::PatClassic, &user, &organizations) {
        Ok(identity) => {
            assert_eq!(identity.organizations, vec!["acme"]);
            assert!(identity.single_sign_on.hides_organizations());
            match identity.single_sign_on {
                SingleSignOn::PartialResults { organization_ids } => {
                    assert_eq!(organization_ids, vec![42]);
                }
                other => panic!("expected partial results, got {other:?}"),
            }
        }
        Err(error) => panic!("the identity must be readable: {error}"),
    }
}

#[tokio::test]
async fn a_token_without_a_scope_header_leaves_the_capabilities_unknown_rather_than_disabled() {
    let server = MockServer::start().await;
    mount(
        &server,
        "/user",
        ResponseTemplate::new(200).set_body_string(USER_BODY),
    )
    .await;
    mount(
        &server,
        "/user/orgs",
        ResponseTemplate::new(200).set_body_string("[]"),
    )
    .await;

    let governor = governor();
    let user = fetch(&governor, &server, "/user").await;
    let organizations = fetch(&governor, &server, "/user/orgs").await;

    let identity = match read_identity(TokenKind::PatFineGrained, &user, &organizations) {
        Ok(identity) => identity,
        Err(error) => panic!("the identity must be readable: {error}"),
    };
    let capabilities = Capabilities::from_identity(&identity);
    assert!(identity.scopes.is_empty());
    for capability in Capability::ALL {
        assert_eq!(capabilities.get(capability), &Support::Unknown);
        assert!(capabilities.get(capability).is_enabled());
    }
}

#[tokio::test]
async fn a_probe_on_the_notifications_endpoint_settles_a_capability_left_unknown() {
    let server = MockServer::start().await;
    mount(
        &server,
        "/user",
        ResponseTemplate::new(200).set_body_string(USER_BODY),
    )
    .await;
    mount(
        &server,
        "/user/orgs",
        ResponseTemplate::new(200).set_body_string("[]"),
    )
    .await;
    mount(
        &server,
        "/notifications",
        ResponseTemplate::new(403).set_body_string(
            "{\"message\":\"Notifications require a classic personal access token\"}",
        ),
    )
    .await;

    let governor = governor();
    let user = fetch(&governor, &server, "/user").await;
    let organizations = fetch(&governor, &server, "/user/orgs").await;
    let probe = fetch(&governor, &server, "/notifications").await;

    let identity = match read_identity(TokenKind::PatFineGrained, &user, &organizations) {
        Ok(identity) => identity,
        Err(error) => panic!("the identity must be readable: {error}"),
    };
    let mut capabilities = Capabilities::from_identity(&identity);
    capabilities.observe_probe(
        Capability::Notifications,
        probe.status,
        &quay_forge::forge_message(&probe.body),
    );

    assert!(!capabilities.get(Capability::Notifications).is_enabled());
    assert!(
        !capabilities
            .discoverable()
            .contains(&Capability::Notifications),
        "a structurally unavailable capability must not appear in the command registry"
    );
}
