use std::sync::Arc;
use std::time::Duration;

use quay_forge::{DevLocks, GovernorConfig, PollingSource, RateGovernor, Token};
use quay_store::Store;
use quay_store::inbox::{InboxFilter, InboxQuery};
use quay_sync::SyncEngine;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const VIEWER: &str = "octocat";

fn notification(kind: &str, url: &str) -> serde_json::Value {
    json!({
        "id": "1",
        "unread": true,
        "updated_at": "2026-09-04T10:00:00Z",
        "reason": "review_requested",
        "subject": { "title": "Fix the thing", "url": url, "type": kind },
        "repository": { "full_name": "acme/api", "node_id": "R_1" }
    })
}

fn thread(identifier: &str, resolved: bool) -> serde_json::Value {
    json!({
        "id": identifier,
        "isResolved": resolved,
        "isOutdated": false,
        "path": "src/main.rs",
        "line": 12,
        "originalLine": 10,
        "diffSide": "RIGHT",
        "comments": { "nodes": [{
            "id": format!("{identifier}-c1"),
            "body": "This needs a test",
            "createdAt": "2026-09-04T09:00:00Z",
            "author": { "login": "avery" }
        }]}
    })
}

fn detail(threads: Vec<serde_json::Value>, has_next_page: bool, cursor: &str) -> serde_json::Value {
    json!({
        "data": { "repository": {
            "id": "R_1",
            "name": "api",
            "owner": { "login": "acme" },
            "defaultBranchRef": { "name": "main" },
            "pullRequest": {
                "id": "PR_1",
                "number": 1234,
                "title": "Refactor the transport layer",
                "state": "OPEN",
                "isDraft": false,
                "mergeable": "MERGEABLE",
                "reviewDecision": "REVIEW_REQUIRED",
                "updatedAt": "2026-09-04T10:00:00Z",
                "baseRefName": "main",
                "headRefOid": "deadbeef",
                "author": { "login": "avery" },
                "reviewRequests": { "nodes": [
                    { "requestedReviewer": { "__typename": "User", "login": VIEWER } },
                    { "requestedReviewer": { "__typename": "Team", "slug": "platform" } }
                ]},
                "commits": { "nodes": [
                    { "commit": { "statusCheckRollup": { "state": "SUCCESS" } } }
                ]},
                "reviewThreads": {
                    "totalCount": 3,
                    "pageInfo": { "hasNextPage": has_next_page, "endCursor": cursor },
                    "nodes": threads
                }
            }
        }}
    })
}

fn governor() -> Arc<RateGovernor> {
    let built = RateGovernor::new(
        GovernorConfig {
            request_timeout: Duration::from_millis(800),
            ..GovernorConfig::default()
        },
        Some(Token::new("ghp_testtoken")),
        DevLocks::from_settings(Some("1"), None),
    );
    match built {
        Ok(governor) => Arc::new(governor),
        Err(error) => panic!("the governor must build: {error}"),
    }
}

fn engine(server: &MockServer) -> (tempfile::TempDir, SyncEngine) {
    let directory = match tempfile::tempdir() {
        Ok(directory) => directory,
        Err(error) => panic!("temporary directory: {error}"),
    };
    let store = match Store::open(&directory.path().join("quay.db")) {
        Ok(store) => store,
        Err(error) => panic!("the store must open: {error}"),
    };
    let account_id = match store.remember_account("api.github.com", VIEWER, "pat_classic", "ref") {
        Ok(identifier) => identifier,
        Err(error) => panic!("the account must be stored: {error}"),
    };
    let governor = governor();
    let source = PollingSource::new(governor.clone(), &server.uri());
    let engine =
        SyncEngine::new(store, governor, &server.uri(), account_id).listening_to(Box::new(source));
    (directory, engine)
}

async fn mount_notifications(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/notifications"))
        .respond_with(response)
        .mount(server)
        .await;
}

async fn mount_graphql(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(response)
        .mount(server)
        .await;
}

#[tokio::test]
async fn a_not_modified_inbox_produces_no_signal_and_no_fetch() {
    let server = MockServer::start().await;
    mount_notifications(&server, ResponseTemplate::new(304)).await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let (_directory, mut engine) = engine(&server);
    match engine.tick().await {
        Ok(report) => {
            assert_eq!(report.signals, 0);
            assert_eq!(report.fetched, 0);
            assert!(report.failures.is_empty());
        }
        Err(error) => panic!("a 304 is not a tick failure: {error}"),
    }
}

#[tokio::test]
async fn a_refused_inbox_leaves_the_tick_successful_and_the_store_untouched() {
    let server = MockServer::start().await;
    mount_notifications(
        &server,
        ResponseTemplate::new(403)
            .insert_header("x-ratelimit-limit", "5000")
            .insert_header("x-ratelimit-remaining", "4900")
            .insert_header("x-ratelimit-reset", "1788000000")
            .set_body_string("{\"message\":\"Resource not accessible\"}"),
    )
    .await;

    let (_directory, mut engine) = engine(&server);
    match engine.tick().await {
        Ok(report) => assert_eq!(report.signals, 0),
        Err(error) => panic!("a refusal is not a tick failure: {error}"),
    }
}

#[tokio::test]
async fn a_malformed_inbox_payload_produces_no_signal_rather_than_a_panic() {
    let server = MockServer::start().await;
    mount_notifications(
        &server,
        ResponseTemplate::new(200).set_body_string("[{\"id\": \"1\", \"unre"),
    )
    .await;

    let (_directory, mut engine) = engine(&server);
    match engine.tick().await {
        Ok(report) => assert_eq!(report.signals, 0),
        Err(error) => panic!("a malformed payload is not a tick failure: {error}"),
    }
}

#[tokio::test]
async fn a_notification_about_an_issue_never_reaches_the_detail_query() {
    let server = MockServer::start().await;
    mount_notifications(
        &server,
        ResponseTemplate::new(200).set_body_json(json!([notification(
            "Issue",
            "https://api.github.com/repos/acme/api/issues/12"
        )])),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let (_directory, mut engine) = engine(&server);
    match engine.tick().await {
        Ok(report) => assert_eq!(report.signals, 0),
        Err(error) => panic!("an issue notification is not a tick failure: {error}"),
    }
}

#[tokio::test]
async fn a_detail_query_the_forge_refuses_is_reported_without_aborting_the_tick() {
    let server = MockServer::start().await;
    mount_notifications(
        &server,
        ResponseTemplate::new(200).set_body_json(json!([notification(
            "PullRequest",
            "https://api.github.com/repos/acme/api/pulls/1234"
        )])),
    )
    .await;
    mount_graphql(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({
            "errors": [{ "message": "Something went wrong while executing your query" }]
        })),
    )
    .await;

    let (_directory, mut engine) = engine(&server);
    match engine.tick().await {
        Ok(report) => {
            assert_eq!(report.signals, 1);
            assert_eq!(report.stored, 0);
            assert_eq!(report.failures.len(), 1);
            assert!(report.failures[0].contains("acme/api#1234"));
        }
        Err(error) => panic!("a refused query is not a tick failure: {error}"),
    }
}

#[tokio::test]
async fn a_pull_request_travels_from_the_inbox_notification_all_the_way_into_sqlite() {
    let server = MockServer::start().await;
    mount_notifications(
        &server,
        ResponseTemplate::new(200)
            .insert_header("etag", "W/\"inbox-1\"")
            .insert_header("x-poll-interval", "90")
            .set_body_json(json!([notification(
                "PullRequest",
                "https://api.github.com/repos/acme/api/pulls/1234"
            )])),
    )
    .await;
    mount_graphql(
        &server,
        ResponseTemplate::new(200).set_body_json(detail(
            vec![thread("RT_1", false), thread("RT_2", true)],
            false,
            "",
        )),
    )
    .await;

    let (_directory, mut engine) = engine(&server);
    match engine.tick().await {
        Ok(report) => {
            assert_eq!(report.signals, 1);
            assert_eq!(report.stored, 1);
            assert!(report.failures.is_empty(), "{:?}", report.failures);
        }
        Err(error) => panic!("the tick must succeed: {error}"),
    }

    let filter = InboxFilter {
        viewer: VIEWER.to_owned(),
        ..InboxFilter::default()
    };
    match engine
        .store()
        .inbox(InboxQuery::ReviewRequestedExact, &filter)
    {
        Ok(rows) => {
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].owner, "acme");
            assert_eq!(rows[0].name, "api");
            assert_eq!(rows[0].number, 1234);
            assert_eq!(rows[0].author, "avery");
            assert_eq!(rows[0].checks_state.as_deref(), Some("success"));
            assert_eq!(rows[0].review_state.as_deref(), Some("review_required"));
            assert_eq!(
                rows[0].unresolved_threads, 1,
                "only the unresolved thread counts"
            );
        }
        Err(error) => panic!("the inbox must be readable: {error}"),
    }
}

#[tokio::test]
async fn a_signal_the_store_is_already_current_on_never_reaches_the_network_twice() {
    let server = MockServer::start().await;
    mount_notifications(
        &server,
        ResponseTemplate::new(200).set_body_json(json!([notification(
            "PullRequest",
            "https://api.github.com/repos/acme/api/pulls/1234"
        )])),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(detail(vec![], false, "")))
        .expect(1)
        .mount(&server)
        .await;

    let (_directory, mut engine) = engine(&server);
    match engine.tick().await {
        Ok(report) => assert_eq!(report.stored, 1),
        Err(error) => panic!("the first tick must succeed: {error}"),
    }
    match engine.tick().await {
        Ok(report) => {
            assert_eq!(report.signals, 1);
            assert_eq!(report.already_current, 1);
            assert_eq!(report.fetched, 0);
        }
        Err(error) => panic!("the second tick must succeed: {error}"),
    }
}

#[tokio::test]
async fn every_page_of_review_threads_is_stored_so_the_unresolved_count_never_lies() {
    let server = MockServer::start().await;
    mount_notifications(
        &server,
        ResponseTemplate::new(200).set_body_json(json!([notification(
            "PullRequest",
            "https://api.github.com/repos/acme/api/pulls/1234"
        )])),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(detail(
            vec![thread("RT_1", false), thread("RT_2", false)],
            true,
            "cursor-1",
        )))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    mount_graphql(
        &server,
        ResponseTemplate::new(200).set_body_json(detail(vec![thread("RT_3", false)], false, "")),
    )
    .await;

    let (_directory, mut engine) = engine(&server);
    match engine.tick().await {
        Ok(report) => assert_eq!(report.stored, 1),
        Err(error) => panic!("the tick must succeed: {error}"),
    }

    let filter = InboxFilter {
        viewer: VIEWER.to_owned(),
        ..InboxFilter::default()
    };
    match engine
        .store()
        .inbox(InboxQuery::OpenWithUnresolvedThreadCount, &filter)
    {
        Ok(rows) => {
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].unresolved_threads, 3);
        }
        Err(error) => panic!("the inbox must be readable: {error}"),
    }
}

#[tokio::test]
async fn the_inbox_validators_survive_a_round_trip_through_the_store() {
    let server = MockServer::start().await;
    mount_notifications(&server, ResponseTemplate::new(304)).await;

    let (_directory, engine) = engine(&server);
    let validators = quay_forge::CacheValidators {
        etag: Some("W/\"inbox-1\"".to_owned()),
        last_modified: None,
    };
    if let Err(error) = engine.remember_notification_validators(Some(&validators)) {
        panic!("the validators must be stored: {error}");
    }
    match engine.notification_validators() {
        Ok(Some(loaded)) => assert_eq!(loaded.etag.as_deref(), Some("W/\"inbox-1\"")),
        other => panic!("the validators must come back, got {other:?}"),
    }
}
