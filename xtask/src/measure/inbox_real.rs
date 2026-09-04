use std::error::Error;
use std::hint::black_box;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use quay_store::Store;
use quay_store::inbox::{InboxFilter, InboxQuery};
use serde_json::{Value, json};

use crate::paths;
use crate::stats::Percentiles;

const DATABASE_VARIABLE: &str = "QUAY_DB";
const WARM_ITERATIONS: usize = 10_000;
const COLD_ITERATIONS: usize = 100;

pub struct QueryMeasurement {
    pub query: &'static str,
    pub cache: &'static str,
    pub rows: usize,
    pub percentiles: Percentiles,
    pub raw_log: String,
}

pub struct RealInbox {
    pub database: PathBuf,
    pub viewer: String,
    pub repositories: i64,
    pub open_pull_requests: i64,
    pub review_threads: i64,
    pub review_comments: i64,
    pub review_requests_for_viewer: i64,
    pub distinct_repositories_in_inbox: i64,
    pub measurements: Vec<QueryMeasurement>,
}

pub fn measure(started_at: &str) -> Result<RealInbox, Box<dyn Error>> {
    let database = PathBuf::from(std::env::var(DATABASE_VARIABLE).map_err(|_| {
        format!("{DATABASE_VARIABLE} must name the database synchronised by the quay binary")
    })?);
    if !database.exists() {
        return Err(format!("no database at {}", database.display()).into());
    }
    std::fs::create_dir_all(paths::raw())?;

    let store = Store::open(&database)?;
    let viewer: String = store.connection().query_row(
        "SELECT login FROM account ORDER BY id LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    let shape = read_shape(&store, &viewer)?;
    let filter = InboxFilter {
        viewer: viewer.clone(),
        ..InboxFilter::default()
    };

    let mut measurements = Vec::new();
    for query in [
        InboxQuery::ReviewRequestedExact,
        InboxQuery::OpenWithUnresolvedThreadCount,
    ] {
        let rows = store.inbox(query, &filter)?.len();

        let mut warm = Vec::with_capacity(WARM_ITERATIONS);
        for _ in 0..WARM_ITERATIONS {
            let started = Instant::now();
            let result = store.inbox(query, &filter)?;
            warm.push(started.elapsed().as_secs_f64() * 1_000.0);
            black_box(result);
        }
        measurements.push(record(started_at, query.id(), "warm", rows, warm)?);

        let mut cold = Vec::with_capacity(COLD_ITERATIONS);
        for _ in 0..COLD_ITERATIONS {
            let reopened = Store::open(&database)?;
            let started = Instant::now();
            let result = reopened.inbox(query, &filter)?;
            cold.push(started.elapsed().as_secs_f64() * 1_000.0);
            black_box(result);
        }
        measurements.push(record(started_at, query.id(), "cold", rows, cold)?);
    }

    Ok(RealInbox {
        database,
        viewer,
        measurements,
        ..shape
    })
}

fn read_shape(store: &Store, viewer: &str) -> Result<RealInbox, Box<dyn Error>> {
    let count = |sql: &str| -> Result<i64, Box<dyn Error>> {
        Ok(store
            .connection()
            .query_row(sql, [viewer], |row| row.get(0))?)
    };
    Ok(RealInbox {
        database: PathBuf::new(),
        viewer: viewer.to_owned(),
        repositories: count("SELECT COUNT(*) FROM repo WHERE ?1 IS NOT NULL")?,
        open_pull_requests: count(
            "SELECT COUNT(*) FROM pull_request WHERE state = 'open' AND ?1 IS NOT NULL",
        )?,
        review_threads: count("SELECT COUNT(*) FROM review_thread WHERE ?1 IS NOT NULL")?,
        review_comments: count("SELECT COUNT(*) FROM review_comment WHERE ?1 IS NOT NULL")?,
        review_requests_for_viewer: count(
            "SELECT COUNT(*) FROM review_request WHERE reviewer = ?1",
        )?,
        distinct_repositories_in_inbox: count(
            "SELECT COUNT(DISTINCT r.id) FROM pull_request pr
             JOIN repo r ON r.id = pr.repo_id
             JOIN review_request rr ON rr.pr_id = pr.id
             WHERE pr.state = 'open' AND r.is_tracked = 1
               AND pr.author <> ?1 AND rr.reviewer = ?1",
        )?,
        measurements: Vec::new(),
    })
}

fn record(
    started_at: &str,
    query: &'static str,
    cache: &'static str,
    rows: usize,
    samples: Vec<f64>,
) -> Result<QueryMeasurement, Box<dyn Error>> {
    let file_name = format!(
        "inbox-real-{}-{}-{}.csv",
        started_at.replace(':', ""),
        query,
        cache
    );
    let mut file = std::fs::File::create(paths::raw().join(&file_name))?;
    writeln!(file, "elapsed_ms")?;
    for sample in &samples {
        writeln!(file, "{sample:.6}")?;
    }
    let percentiles = Percentiles::of(&samples).ok_or("an empty sample cannot be summarised")?;
    println!(
        "inbox-real: {query} {cache} rows={rows} p50={:.3}ms p99={:.3}ms max={:.3}ms",
        percentiles.p50, percentiles.p99, percentiles.max
    );
    Ok(QueryMeasurement {
        query,
        cache,
        rows,
        percentiles,
        raw_log: format!("measurements/raw/{file_name}"),
    })
}

pub fn summary(inbox: &RealInbox, started_at: &str) -> Value {
    json!({
        "scenario": "inbox-real",
        "measured_at": started_at,
        "machine": crate::machine(),
        "dataset": {
            "database_file": inbox
                .database
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
            "viewer_resolved": !inbox.viewer.is_empty(),
            "repositories": inbox.repositories,
            "open_pull_requests": inbox.open_pull_requests,
            "review_threads": inbox.review_threads,
            "review_comments": inbox.review_comments,
            "review_requests_for_viewer": inbox.review_requests_for_viewer,
            "distinct_repositories_in_inbox": inbox.distinct_repositories_in_inbox,
        },
        "rows": inbox
            .measurements
            .iter()
            .map(|measurement| json!({
                "query": measurement.query,
                "cache": measurement.cache,
                "rows": measurement.rows,
                "percentiles": measurement.percentiles,
                "raw_log": measurement.raw_log,
            }))
            .collect::<Vec<Value>>(),
    })
}
