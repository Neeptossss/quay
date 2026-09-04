use std::sync::Arc;
use std::time::Duration;

use quay_core::MutationState;
use quay_forge::{DevLocks, GovernorConfig, RateGovernor, Token};
use quay_store::Store;
use quay_sync::drain_mutations;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const NOW: i64 = 1_788_516_000;
const FIXTURE: &str = "
    INSERT INTO account (id, host, login, auth_kind, keychain_ref)
        VALUES (1, 'api.github.com', 'octocat', 'pat_classic', 'quay/octocat');
    INSERT INTO repo (id, account_id, node_id, owner, name, is_tracked)
        VALUES (1, 1, 'R_1', 'acme', 'api', 1);
    INSERT INTO pull_request (
        id, repo_id, number, node_id, title, state, is_draft, author,
        base_ref, head_sha, review_state, updated_at)
        VALUES (1, 1, 42, 'PR_1', 'Fix', 'open', 0, 'avery',
                'main', 'deadbeef', 'commented', '2026-09-04T10:00:00Z');
    INSERT INTO review_thread (id, pr_id, node_id, path, is_resolved, is_outdated)
        VALUES (1, 1, 'RT_1', 'src/main.rs', 0, 0);
";

fn store() -> (tempfile::TempDir, Store) {
    let directory = match tempfile::tempdir() {
        Ok(directory) => directory,
        Err(error) => panic!("temporary directory: {error}"),
    };
    let store = match Store::open(&directory.path().join("quay.db")) {
        Ok(store) => store,
        Err(error) => panic!("the store must open: {error}"),
    };
    if let Err(error) = store.connection().execute_batch(FIXTURE) {
        panic!("the fixture must load: {error}");
    }
    (directory, store)
}

fn governor(readonly: &str, allowlist: Option<&str>) -> Arc<RateGovernor> {
    let built = RateGovernor::new(
        GovernorConfig {
            request_timeout: Duration::from_millis(800),
            max_attempts: 1,
            ..GovernorConfig::default()
        },
        Some(Token::new("ghp_testtoken")),
        DevLocks::from_settings(Some(readonly), allowlist),
    );
    match built {
        Ok(governor) => Arc::new(governor),
        Err(error) => panic!("the governor must build: {error}"),
    }
}

fn review_state(store: &Store) -> Option<String> {
    match store.connection().query_row(
        "SELECT review_state FROM pull_request WHERE node_id = 'PR_1'",
        [],
        |row| row.get::<_, Option<String>>(0),
    ) {
        Ok(value) => value,
        Err(error) => panic!("the pull request must be readable: {error}"),
    }
}

async fn mount(server: &MockServer, verb: &str, suffix: &str, response: ResponseTemplate) {
    Mock::given(method(verb))
        .and(path(suffix))
        .respond_with(response)
        .mount(server)
        .await;
}

#[tokio::test]
async fn read_only_mode_keeps_the_user_intent_queued_rather_than_rolling_it_back() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/api/pulls/42/reviews"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let (_directory, mut store) = store();
    if let Err(error) = store.approve_pull_request("PR_1", "idem-1", NOW) {
        panic!("the mutation must enqueue: {error}");
    }
    let governor = governor("1", None);
    match drain_mutations(&mut store, &governor, &server.uri(), 4).await {
        Ok(report) => {
            assert_eq!(report.sent, 0);
            assert_eq!(report.rolled_back, 0);
            assert_eq!(report.deferred, 1);
        }
        Err(error) => panic!("the drain must succeed: {error}"),
    }
    assert_eq!(
        review_state(&store).as_deref(),
        Some("approved"),
        "a development lock must not undo what the user asked for"
    );
}

#[tokio::test]
async fn a_repository_outside_the_write_allowlist_keeps_the_mutation_queued() {
    let server = MockServer::start().await;
    let (_directory, mut store) = store();
    if let Err(error) = store.approve_pull_request("PR_1", "idem-1", NOW) {
        panic!("the mutation must enqueue: {error}");
    }
    let governor = governor("0", Some("someone/scratch"));
    match drain_mutations(&mut store, &governor, &server.uri(), 4).await {
        Ok(report) => assert_eq!(report.deferred, 1),
        Err(error) => panic!("the drain must succeed: {error}"),
    }
}

#[tokio::test]
async fn an_accepted_review_settles_the_mutation_and_keeps_the_optimistic_state() {
    let server = MockServer::start().await;
    mount(
        &server,
        "POST",
        "/repos/acme/api/pulls/42/reviews",
        ResponseTemplate::new(200).set_body_string("{\"id\": 1}"),
    )
    .await;

    let (_directory, mut store) = store();
    let identifier = match store.approve_pull_request("PR_1", "idem-1", NOW) {
        Ok(identifier) => identifier,
        Err(error) => panic!("the mutation must enqueue: {error}"),
    };
    let governor = governor("0", None);
    match drain_mutations(&mut store, &governor, &server.uri(), 4).await {
        Ok(report) => {
            assert_eq!(report.sent, 1);
            assert!(report.failures.is_empty());
        }
        Err(error) => panic!("the drain must succeed: {error}"),
    }
    assert_eq!(review_state(&store).as_deref(), Some("approved"));
    match store.mutation(identifier) {
        Ok(Some(mutation)) => assert_eq!(mutation.state, MutationState::Done),
        other => panic!("the mutation must be done, got {other:?}"),
    }
}

#[tokio::test]
async fn a_definitive_refusal_rolls_the_optimistic_state_back_and_names_the_reason() {
    let server = MockServer::start().await;
    mount(
        &server,
        "POST",
        "/repos/acme/api/pulls/42/reviews",
        ResponseTemplate::new(422)
            .insert_header("x-ratelimit-limit", "5000")
            .insert_header("x-ratelimit-remaining", "4900")
            .insert_header("x-ratelimit-reset", "1788000000")
            .set_body_string("{\"message\":\"Can not approve your own pull request\"}"),
    )
    .await;

    let (_directory, mut store) = store();
    if let Err(error) = store.approve_pull_request("PR_1", "idem-1", NOW) {
        panic!("the mutation must enqueue: {error}");
    }
    let governor = governor("0", None);
    match drain_mutations(&mut store, &governor, &server.uri(), 4).await {
        Ok(report) => {
            assert_eq!(report.rolled_back, 1);
            assert_eq!(report.failures.len(), 1);
            assert!(report.failures[0].contains("Can not approve"));
        }
        Err(error) => panic!("the drain must succeed: {error}"),
    }
    assert_eq!(
        review_state(&store).as_deref(),
        Some("commented"),
        "the state must return to what it was before the user acted"
    );
}

#[tokio::test]
async fn a_throttled_mutation_is_deferred_with_the_optimistic_state_left_in_place() {
    let server = MockServer::start().await;
    mount(
        &server,
        "POST",
        "/repos/acme/api/pulls/42/reviews",
        ResponseTemplate::new(429).insert_header("retry-after", "0"),
    )
    .await;

    let (_directory, mut store) = store();
    if let Err(error) = store.approve_pull_request("PR_1", "idem-1", NOW) {
        panic!("the mutation must enqueue: {error}");
    }
    let governor = governor("0", None);
    match drain_mutations(&mut store, &governor, &server.uri(), 4).await {
        Ok(report) => {
            assert_eq!(report.deferred, 1);
            assert_eq!(report.rolled_back, 0);
        }
        Err(error) => panic!("the drain must succeed: {error}"),
    }
    assert_eq!(review_state(&store).as_deref(), Some("approved"));
    match store.mutations_in_state(MutationState::Pending) {
        Ok(rows) => assert_eq!(rows.len(), 1),
        Err(error) => panic!("the queue must be readable: {error}"),
    }
}

#[tokio::test]
async fn a_server_error_is_deferred_rather_than_treated_as_a_refusal() {
    let server = MockServer::start().await;
    mount(
        &server,
        "POST",
        "/repos/acme/api/pulls/42/reviews",
        ResponseTemplate::new(500).set_body_string("{\"message\":\"Server Error\"}"),
    )
    .await;

    let (_directory, mut store) = store();
    if let Err(error) = store.approve_pull_request("PR_1", "idem-1", NOW) {
        panic!("the mutation must enqueue: {error}");
    }
    let governor = governor("0", None);
    match drain_mutations(&mut store, &governor, &server.uri(), 4).await {
        Ok(report) => {
            assert_eq!(report.deferred, 1);
            assert_eq!(report.rolled_back, 0);
        }
        Err(error) => panic!("the drain must succeed: {error}"),
    }
    assert_eq!(review_state(&store).as_deref(), Some("approved"));
}

#[tokio::test]
async fn resolving_a_thread_travels_as_a_graphql_mutation_and_settles() {
    let server = MockServer::start().await;
    mount(
        &server,
        "POST",
        "/graphql",
        ResponseTemplate::new(200)
            .set_body_string("{\"data\":{\"resolveReviewThread\":{\"thread\":{\"id\":\"RT_1\"}}}}"),
    )
    .await;

    let (_directory, mut store) = store();
    if let Err(error) = store.resolve_thread("RT_1", "idem-1", NOW) {
        panic!("the mutation must enqueue: {error}");
    }
    let governor = governor("0", None);
    match drain_mutations(&mut store, &governor, &server.uri(), 4).await {
        Ok(report) => assert_eq!(report.sent, 1),
        Err(error) => panic!("the drain must succeed: {error}"),
    }
}

#[tokio::test]
async fn draining_an_empty_queue_costs_no_request_and_reports_nothing() {
    let server = MockServer::start().await;
    let (_directory, mut store) = store();
    let governor = governor("0", None);
    match drain_mutations(&mut store, &governor, &server.uri(), 4).await {
        Ok(report) => assert_eq!(report, quay_sync::MutationReport::default()),
        Err(error) => panic!("the drain must succeed: {error}"),
    }
    assert_eq!(governor.issued_requests(), 0);
}
