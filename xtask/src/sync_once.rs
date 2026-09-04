use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

use quay_forge::{DevLocks, GovernorConfig, PollingSource, RateGovernor, Token, read_identity};
use quay_store::Store;
use quay_store::inbox::{InboxFilter, InboxQuery};
use quay_sync::SyncEngine;

const API: &str = "https://api.github.com";

pub async fn run() -> Result<(), Box<dyn Error>> {
    let token = Token::from_env("QUAY_TEST_TOKEN")
        .ok_or("QUAY_TEST_TOKEN is not set; the fetch path cannot run")?;
    let token_kind = token.kind();
    let governor = Arc::new(RateGovernor::new(
        GovernorConfig::default(),
        Some(token),
        DevLocks::from_env(),
    )?);

    let user = governor
        .send(quay_forge::OutboundRequest::rest_read(
            format!("{API}/user"),
            quay_core::Priority::User,
        ))
        .await?;
    let organizations = governor
        .send(quay_forge::OutboundRequest::rest_read(
            format!("{API}/user/orgs"),
            quay_core::Priority::User,
        ))
        .await?;
    let identity = read_identity(token_kind, &user, &organizations)?;

    let directory = tempfile::tempdir()?;
    let database = directory.path().join("quay.db");
    let store = Store::open(&database)?;
    let account_id =
        store.remember_account("api.github.com", &identity.login, "pat_classic", "keychain")?;

    let source = PollingSource::new(governor.clone(), API);
    let mut engine =
        SyncEngine::new(store, governor.clone(), API, account_id).listening_to(Box::new(source));

    let started = Instant::now();
    let report = engine.tick().await?;
    let sync_duration = started.elapsed();

    println!(
        "sync-once: {} signal(s), {} fetched, {} stored, {} already current, {} failure(s), in {:.0} ms",
        report.signals,
        report.fetched,
        report.stored,
        report.already_current,
        report.failures.len(),
        sync_duration.as_secs_f64() * 1_000.0
    );
    for failure in &report.failures {
        println!("sync-once: failure {failure}");
    }

    print_stored_breakdown(engine.store())?;

    let filter = InboxFilter {
        viewer: identity.login.clone(),
        ..InboxFilter::default()
    };
    for query in [
        InboxQuery::OpenWithUnresolvedThreadCount,
        InboxQuery::ReviewRequestedExact,
    ] {
        let started = Instant::now();
        let rows = engine.store().inbox(query, &filter)?;
        println!(
            "sync-once: {} returned {} row(s) in {:.3} ms",
            query.id(),
            rows.len(),
            started.elapsed().as_secs_f64() * 1_000.0
        );
        for row in rows.iter().take(5) {
            println!(
                "    {}/{}#{} [{}] {} unresolved, checks {}",
                row.owner,
                row.name,
                row.number,
                row.author,
                row.unresolved_threads,
                row.checks_state.as_deref().unwrap_or("none")
            );
        }
    }

    println!(
        "sync-once: {} request(s) issued, {} point(s) spent, {} free revalidation(s), health {:?}",
        governor.issued_requests(),
        governor.spent_points(),
        governor.free_revalidations(),
        governor.health()
    );
    Ok(())
}

fn print_stored_breakdown(store: &Store) -> Result<(), Box<dyn Error>> {
    let mut statement = store
        .connection()
        .prepare("SELECT state, COUNT(*) FROM pull_request GROUP BY state ORDER BY state")?;
    let counts = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<(String, i64)>>>()?;
    let rendered: Vec<String> = counts
        .iter()
        .map(|(state, count)| format!("{state}={count}"))
        .collect();
    println!(
        "sync-once: stored pull requests by state {}",
        rendered.join(" ")
    );

    let threads: i64 =
        store
            .connection()
            .query_row("SELECT COUNT(*) FROM review_thread", [], |row| row.get(0))?;
    let comments: i64 =
        store
            .connection()
            .query_row("SELECT COUNT(*) FROM review_comment", [], |row| row.get(0))?;
    let requests: i64 =
        store
            .connection()
            .query_row("SELECT COUNT(*) FROM review_request", [], |row| row.get(0))?;
    println!("sync-once: {threads} thread(s), {comments} comment(s), {requests} review request(s)");
    Ok(())
}
