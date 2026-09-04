use std::error::Error;
use std::io::Write;
use std::time::Instant;

use quay_store::{Durability, Store};
use serde_json::{Value, json};

use crate::paths;
use crate::stats::Percentiles;

const ITERATIONS: usize = 2_000;
const FIXTURE: &str = "
    INSERT INTO account (id, host, login, auth_kind, keychain_ref)
        VALUES (1, 'api.github.com', 'octocat', 'pat_classic', 'quay/octocat');
    INSERT INTO repo (id, account_id, node_id, owner, name, is_tracked)
        VALUES (1, 1, 'R_1', 'acme', 'api', 1);
    INSERT INTO pull_request (
        id, repo_id, number, node_id, title, state, is_draft, author,
        base_ref, head_sha, review_state, updated_at)
        VALUES (1, 1, 7, 'PR_1', 'Fix', 'open', 0, 'avery',
                'main', 'deadbeef', NULL, '2026-09-04T10:00:00Z');
";

pub struct DurabilityMeasurement {
    pub mode: &'static str,
    pub synchronous: &'static str,
    pub flushes_the_drive_cache: bool,
    pub percentiles: Percentiles,
    pub raw_log: String,
}

pub fn measure(started_at: &str) -> Result<Vec<DurabilityMeasurement>, Box<dyn Error>> {
    std::fs::create_dir_all(paths::raw())?;
    let mut measurements = Vec::new();
    for durability in Durability::ALL {
        measurements.push(measure_mode(started_at, durability)?);
    }
    Ok(measurements)
}

fn measure_mode(
    started_at: &str,
    durability: Durability,
) -> Result<DurabilityMeasurement, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open_with(&directory.path().join("quay.db"), durability)?;
    store.connection().execute_batch(FIXTURE)?;

    let mut samples = Vec::with_capacity(ITERATIONS);
    for iteration in 0..ITERATIONS {
        let key = format!("durability-{iteration}");
        let started = Instant::now();
        store.approve_pull_request("PR_1", &key, 1_788_516_000)?;
        samples.push(started.elapsed().as_secs_f64() * 1_000.0);
    }

    let mode = durability.id();
    let file_name = format!("durability-{}-{mode}.csv", started_at.replace(':', ""));
    let mut file = std::fs::File::create(paths::raw().join(&file_name))?;
    writeln!(file, "elapsed_ms")?;
    for sample in &samples {
        writeln!(file, "{sample:.6}")?;
    }

    let percentiles = Percentiles::of(&samples).ok_or("an empty sample cannot be summarised")?;
    println!(
        "durability: {mode} (synchronous={}, fullfsync={}) p50={:.3}ms p95={:.3}ms p99={:.3}ms max={:.3}ms",
        durability.synchronous(),
        durability.flushes_the_drive_cache(),
        percentiles.p50,
        percentiles.p95,
        percentiles.p99,
        percentiles.max
    );
    Ok(DurabilityMeasurement {
        mode,
        synchronous: durability.synchronous(),
        flushes_the_drive_cache: durability.flushes_the_drive_cache(),
        percentiles,
        raw_log: format!("measurements/raw/{file_name}"),
    })
}

pub fn summary(measurements: &[DurabilityMeasurement], started_at: &str) -> Value {
    json!({
        "scenario": "durability",
        "measured_at": started_at,
        "machine": crate::machine(),
        "operation": "optimistic approve: one transaction, one update and one queue insert",
        "iterations": ITERATIONS,
        "rows": measurements
            .iter()
            .map(|measurement| json!({
                "guarantee": measurement.mode,
                "synchronous": measurement.synchronous,
                "fullfsync": measurement.flushes_the_drive_cache,
                "percentiles": measurement.percentiles,
                "raw_log": measurement.raw_log,
            }))
            .collect::<Vec<Value>>(),
    })
}
