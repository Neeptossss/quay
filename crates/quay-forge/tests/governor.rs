use std::time::{Duration, Instant};

use quay_core::Priority;
use quay_forge::{
    CacheValidators, DevLocks, ForgeError, GovernorConfig, Health, OutboundRequest, RateGovernor,
    Token,
};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn governor(config: GovernorConfig) -> RateGovernor {
    match RateGovernor::new(
        config,
        Some(Token::new("ghp_testtoken")),
        DevLocks::from_settings(Some("1"), None),
    ) {
        Ok(governor) => governor,
        Err(error) => panic!("the governor must build: {error}"),
    }
}

fn read(server: &MockServer, suffix: &str) -> OutboundRequest {
    OutboundRequest::rest_read(format!("{}{suffix}", server.uri()), Priority::Warm)
}

fn quick() -> GovernorConfig {
    GovernorConfig {
        request_timeout: Duration::from_millis(500),
        ..GovernorConfig::default()
    }
}

#[tokio::test]
async fn a_bare_403_carrying_no_rate_limit_signal_is_treated_as_a_refusal() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rate-limited"))
        .respond_with(ResponseTemplate::new(403))
        .expect(1)
        .mount(&server)
        .await;

    let governor = governor(GovernorConfig {
        max_attempts: 2,
        ..quick()
    });
    match governor.send(read(&server, "/rate-limited")).await {
        Ok(response) => {
            assert_eq!(response.status, 403);
            assert_eq!(response.attempts, 1);
        }
        Err(error) => panic!("a 403 with no rate limit signal is an answer: {error}"),
    }
    assert_eq!(governor.health(), Health::Healthy);
}

#[tokio::test]
async fn a_403_with_a_retry_after_header_is_retried_and_reported_as_throttled() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rate-limited"))
        .respond_with(ResponseTemplate::new(403).insert_header("retry-after", "0"))
        .mount(&server)
        .await;

    let governor = governor(GovernorConfig {
        max_attempts: 2,
        ..quick()
    });
    match governor.send(read(&server, "/rate-limited")).await {
        Err(ForgeError::Throttled {
            attempts,
            last_status,
        }) => {
            assert_eq!(attempts, 2);
            assert_eq!(last_status, 403);
        }
        other => panic!("a throttling 403 must be reported as throttled, got {other:?}"),
    }
    assert_eq!(governor.health(), Health::Throttled);
}

#[tokio::test]
async fn a_429_with_retry_after_zero_is_retried_immediately_and_can_succeed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/retry"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/retry"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;

    let governor = governor(GovernorConfig {
        max_attempts: 3,
        ..quick()
    });
    match governor.send(read(&server, "/retry")).await {
        Ok(response) => {
            assert_eq!(response.status, 200);
            assert_eq!(response.attempts, 2);
        }
        Err(error) => panic!("the retry must succeed: {error}"),
    }
    assert_eq!(governor.throttled_responses(), 1);
}

#[tokio::test]
async fn a_304_costs_no_point_while_a_200_costs_one() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/not-modified"))
        .respond_with(ResponseTemplate::new(304))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/modified"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;

    let governor = governor(quick());
    match governor.send(read(&server, "/not-modified")).await {
        Ok(response) => assert_eq!(response.status, 304),
        Err(error) => panic!("a 304 is not an error: {error}"),
    }
    assert_eq!(governor.spent_points(), 0);

    match governor.send(read(&server, "/modified")).await {
        Ok(response) => assert_eq!(response.status, 200),
        Err(error) => panic!("a 200 must succeed: {error}"),
    }
    assert_eq!(governor.spent_points(), 1);
    assert_eq!(governor.issued_requests(), 2);
}

#[tokio::test]
async fn a_truncated_body_is_handed_back_untouched_rather_than_parsed_or_panicked() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/truncated"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{\"data\": {\"repos"))
        .mount(&server)
        .await;

    let governor = governor(quick());
    match governor.send(read(&server, "/truncated")).await {
        Ok(response) => {
            assert_eq!(response.status, 200);
            assert_eq!(response.body, "{\"data\": {\"repos");
            assert_eq!(response.bytes, 16);
        }
        Err(error) => panic!("a truncated body is the caller's problem: {error}"),
    }
}

#[tokio::test]
async fn a_timeout_is_reported_as_a_transport_failure_and_never_leaks_the_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/slow"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
        .mount(&server)
        .await;

    let governor = governor(GovernorConfig {
        request_timeout: Duration::from_millis(200),
        ..GovernorConfig::default()
    });
    match governor.send(read(&server, "/slow")).await {
        Err(ForgeError::Transport(message)) => {
            assert!(!message.contains("ghp_testtoken"));
        }
        other => panic!("a timeout must be a transport failure, got {other:?}"),
    }
}

#[tokio::test]
async fn a_quota_below_twenty_percent_switches_the_governor_to_degraded() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/quota"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-ratelimit-limit", "5000")
                .insert_header("x-ratelimit-remaining", "900")
                .insert_header("x-ratelimit-reset", "1788000000")
                .set_body_string("{}"),
        )
        .mount(&server)
        .await;

    let governor = governor(quick());
    match governor.send(read(&server, "/quota")).await {
        Ok(_) => {}
        Err(error) => panic!("the request must succeed: {error}"),
    }
    assert_eq!(governor.health(), Health::Degraded);
    match governor.rate_limit() {
        Some(snapshot) => {
            assert_eq!(snapshot.remaining, 900);
            assert_eq!(snapshot.limit, 5_000);
        }
        None => panic!("the rate limit headers must be read"),
    }
}

#[tokio::test]
async fn a_healthy_quota_keeps_the_governor_healthy() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/quota"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-ratelimit-limit", "5000")
                .insert_header("x-ratelimit-remaining", "4000")
                .insert_header("x-ratelimit-reset", "1788000000")
                .set_body_string("{}"),
        )
        .mount(&server)
        .await;

    let governor = governor(quick());
    match governor.send(read(&server, "/quota")).await {
        Ok(_) => {}
        Err(error) => panic!("the request must succeed: {error}"),
    }
    assert_eq!(governor.health(), Health::Healthy);
}

#[tokio::test]
async fn the_hourly_budget_refuses_the_request_that_would_exceed_it() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/budget"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;

    let governor = governor(GovernorConfig {
        hourly_request_budget: 2,
        ..quick()
    });
    for _ in 0..2 {
        match governor.send(read(&server, "/budget")).await {
            Ok(_) => {}
            Err(error) => panic!("the budget must allow this request: {error}"),
        }
    }
    match governor.send(read(&server, "/budget")).await {
        Err(ForgeError::BudgetExhausted { budget, .. }) => assert_eq!(budget, 2),
        other => panic!("the budget must refuse the third request, got {other:?}"),
    }
}

#[tokio::test]
async fn read_only_mode_stops_a_write_before_it_reaches_the_network() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/writes"))
        .respond_with(ResponseTemplate::new(201))
        .expect(0)
        .mount(&server)
        .await;

    let governor = governor(quick());
    let request = OutboundRequest::rest_write(
        format!("{}/writes", server.uri()),
        "{}",
        "acme/api",
        Priority::User,
    );
    match governor.send(request).await {
        Err(ForgeError::ReadOnlyModeRejected { .. }) => {}
        other => panic!("read-only mode must reject the write, got {other:?}"),
    }
    assert_eq!(governor.issued_requests(), 0);
}

#[tokio::test]
async fn the_concurrency_ceiling_serialises_requests_beyond_its_permits() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/concurrent"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(150))
                .set_body_string("{}"),
        )
        .mount(&server)
        .await;

    let governor = std::sync::Arc::new(governor(GovernorConfig {
        max_concurrent_requests: 2,
        request_timeout: Duration::from_secs(10),
        ..GovernorConfig::default()
    }));

    let started = Instant::now();
    let mut handles = Vec::new();
    for _ in 0..6 {
        let governor = governor.clone();
        let url = format!("{}/concurrent", server.uri());
        handles.push(tokio::spawn(async move {
            governor
                .send(OutboundRequest::rest_read(url, Priority::Warm))
                .await
                .map(|response| response.status)
        }));
    }
    for handle in handles {
        match handle.await {
            Ok(Ok(status)) => assert_eq!(status, 200),
            other => panic!("every request must complete: {other:?}"),
        }
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(400),
        "six requests through two permits cannot finish in {elapsed:?}"
    );
}

#[tokio::test]
async fn the_default_concurrency_ceiling_is_eight_permits_not_one_hundred() {
    assert_eq!(GovernorConfig::default().max_concurrent_requests, 8);
    assert_eq!(GovernorConfig::default().hourly_request_budget, 1_500);
}

#[tokio::test]
async fn a_request_to_an_unreachable_host_is_a_transport_failure() {
    let governor = match RateGovernor::new(
        GovernorConfig {
            request_timeout: Duration::from_millis(300),
            ..GovernorConfig::default()
        },
        None,
        DevLocks::from_settings(Some("1"), None),
    ) {
        Ok(governor) => governor,
        Err(error) => panic!("the governor must build: {error}"),
    };
    let request = OutboundRequest::rest_read("http://127.0.0.1:1/unreachable", Priority::User);
    match governor.send(request).await {
        Err(ForgeError::Transport(_)) => {}
        other => panic!("an unreachable host must be a transport failure, got {other:?}"),
    }
}

#[tokio::test]
async fn a_403_from_a_permission_check_is_returned_at_once_and_never_retried() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/forbidden"))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("x-ratelimit-limit", "5000")
                .insert_header("x-ratelimit-remaining", "4873")
                .insert_header("x-ratelimit-reset", "1788000000")
                .set_body_string("{\"message\":\"Resource not accessible by integration\"}"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let governor = governor(quick());
    match governor.send(read(&server, "/forbidden")).await {
        Ok(response) => {
            assert_eq!(response.status, 403);
            assert_eq!(response.attempts, 1);
        }
        Err(error) => panic!("a permission refusal is an answer, not a throttle: {error}"),
    }
    assert_eq!(governor.health(), Health::Healthy);
    assert_eq!(governor.throttled_responses(), 0);
}

#[tokio::test]
async fn a_403_naming_a_secondary_rate_limit_is_retried() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/secondary"))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("x-ratelimit-limit", "5000")
                .insert_header("x-ratelimit-remaining", "4873")
                .insert_header("x-ratelimit-reset", "1788000000")
                .set_body_string("{\"message\":\"You have exceeded a secondary rate limit\"}"),
        )
        .mount(&server)
        .await;

    let governor = governor(GovernorConfig {
        max_attempts: 2,
        ..quick()
    });
    match governor.send(read(&server, "/secondary")).await {
        Err(ForgeError::Throttled { attempts, .. }) => assert_eq!(attempts, 2),
        other => panic!("a secondary rate limit must be retried, got {other:?}"),
    }
}

#[tokio::test]
async fn a_403_with_the_primary_quota_at_zero_is_retried() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/exhausted"))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("x-ratelimit-limit", "5000")
                .insert_header("x-ratelimit-remaining", "0")
                .insert_header("x-ratelimit-reset", "1788000000")
                .set_body_string("{\"message\":\"Forbidden\"}"),
        )
        .mount(&server)
        .await;

    let governor = governor(GovernorConfig {
        max_attempts: 2,
        ..quick()
    });
    assert!(matches!(
        governor.send(read(&server, "/exhausted")).await,
        Err(ForgeError::Throttled { .. })
    ));
}

#[tokio::test]
async fn a_revalidation_sends_the_cached_validators_and_costs_no_budget_slot() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/conditional"))
        .and(header("if-none-match", "W/\"cached\""))
        .respond_with(ResponseTemplate::new(304))
        .mount(&server)
        .await;

    let governor = governor(GovernorConfig {
        hourly_request_budget: 1,
        ..quick()
    });
    let request = read(&server, "/conditional").revalidating(Some(CacheValidators {
        etag: Some("W/\"cached\"".to_owned()),
        last_modified: None,
    }));
    match governor.send(request).await {
        Ok(response) => {
            assert!(response.is_not_modified());
            assert_eq!(response.points, 1);
        }
        Err(error) => panic!("a revalidation must succeed: {error}"),
    }
    assert_eq!(governor.spent_points(), 0);
    assert_eq!(governor.free_revalidations(), 1);

    let second = read(&server, "/conditional").revalidating(Some(CacheValidators {
        etag: Some("W/\"cached\"".to_owned()),
        last_modified: None,
    }));
    assert!(
        governor.send(second).await.is_ok(),
        "a budget of one must still allow a second free revalidation"
    );
}

#[tokio::test]
async fn a_changed_resource_returns_the_new_validators_to_cache() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/changed"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("etag", "W/\"fresh\"")
                .insert_header("last-modified", "Wed, 02 Sep 2026 10:00:00 GMT")
                .insert_header("x-poll-interval", "90")
                .set_body_string("[]"),
        )
        .mount(&server)
        .await;

    let governor = governor(quick());
    match governor.send(read(&server, "/changed")).await {
        Ok(response) => {
            assert!(!response.is_not_modified());
            match response.validators {
                Some(validators) => {
                    assert_eq!(validators.etag.as_deref(), Some("W/\"fresh\""));
                    assert!(validators.last_modified.is_some());
                }
                None => panic!("the validators must be captured"),
            }
            assert_eq!(response.poll_interval, Some(Duration::from_secs(90)));
        }
        Err(error) => panic!("the request must succeed: {error}"),
    }
    assert_eq!(governor.spent_points(), 1);
}

#[tokio::test]
async fn speculation_stops_at_fifteen_percent_of_the_budget_while_user_requests_continue() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/speculative"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;

    let governor = governor(GovernorConfig {
        hourly_request_budget: 20,
        ..quick()
    });
    let speculative = || {
        OutboundRequest::rest_read(
            format!("{}/speculative", server.uri()),
            Priority::Speculative,
        )
    };

    for _ in 0..3 {
        match governor.send(speculative()).await {
            Ok(response) => assert_eq!(response.status, 200),
            Err(error) => panic!("the speculation budget must allow this request: {error}"),
        }
    }
    match governor.send(speculative()).await {
        Err(ForgeError::SpeculationBudgetExhausted { budget, .. }) => assert_eq!(budget, 3),
        other => panic!("the fourth speculative request must be refused, got {other:?}"),
    }

    match governor
        .send(OutboundRequest::rest_read(
            format!("{}/speculative", server.uri()),
            Priority::User,
        ))
        .await
    {
        Ok(response) => assert_eq!(response.status, 200),
        Err(error) => panic!("a user request must not be blocked by speculation: {error}"),
    }
}

#[tokio::test]
async fn the_governor_never_polls_the_rate_limit_endpoint_by_itself() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rate_limit"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/anything"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-ratelimit-limit", "5000")
                .insert_header("x-ratelimit-remaining", "4321")
                .insert_header("x-ratelimit-reset", "1788000000")
                .set_body_string("{}"),
        )
        .mount(&server)
        .await;

    let governor = governor(quick());
    match governor.send(read(&server, "/anything")).await {
        Ok(_) => {}
        Err(error) => panic!("the request must succeed: {error}"),
    }
    match governor.rate_limit() {
        Some(snapshot) => assert_eq!(snapshot.remaining, 4321),
        None => panic!("the quota must come from the response headers"),
    }
}
