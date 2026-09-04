pub mod cold_start;
pub mod durability;
pub mod inbox_real;
pub mod j1a;
pub mod j1b;
pub mod m0_1;

use std::error::Error;

use serde_json::{Value, json};

use crate::paths;

pub fn now_utc() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| String::from("unknown"))
}

pub fn write_scenario_summary(name: &str, content: Value) -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all(paths::summaries())?;
    let path = paths::summaries().join(format!("{name}.json"));
    std::fs::write(
        &path,
        format!("{}\n", serde_json::to_string_pretty(&content)?),
    )?;
    Ok(())
}

pub fn j1b_summary(measurements: &[j1b::QueryMeasurement], started_at: &str) -> Value {
    json!({
        "scenario": "j1b",
        "measured_at": started_at,
        "machine": crate::machine(),
        "budget": "inbox_query",
        "rows": measurements
            .iter()
            .map(|measurement| json!({
                "schema": measurement.schema,
                "dataset": measurement.dataset,
                "scale": measurement.scale,
                "query": measurement.query,
                "cache": measurement.cache,
                "rows": measurement.rows,
                "plan": measurement.plan,
                "percentiles": measurement.percentiles,
                "raw_log": measurement.raw_log,
            }))
            .collect::<Vec<Value>>(),
    })
}

pub fn j1a_summary(samples: &[j1a::Sample], started_at: &str, raw_log: &str) -> Value {
    let mut groups: Vec<(String, String, String, Vec<&j1a::Sample>)> = Vec::new();
    for sample in samples {
        let repository = format!("{}/{}", sample.target.owner, sample.target.name);
        let key = (
            format!("{repository}#{}", sample.target.number),
            sample.mode.clone(),
            sample.configuration.clone(),
        );
        match groups
            .iter_mut()
            .find(|group| group.0 == key.0 && group.1 == key.1 && group.2 == key.2)
        {
            Some(group) => group.3.push(sample),
            None => groups.push((key.0, key.1, key.2, vec![sample])),
        }
    }

    let rows: Vec<Value> = groups
        .iter()
        .map(|(target, mode, configuration, samples)| {
            let durations: Vec<f64> = samples.iter().map(|sample| sample.elapsed_ms).collect();
            let representative = samples.first();
            json!({
                "target": target,
                "mode": mode,
                "configuration": configuration,
                "repetitions": samples.len(),
                "elapsed_ms": crate::stats::Percentiles::of(&durations),
                "selection": representative.map(|sample| sample.target.selection.clone()),
                "changed_files": representative.map(|sample| sample.target.changed_files),
                "review_threads": representative.map(|sample| sample.target.review_threads),
                "requests": representative.map(|sample| sample.requests),
                "bytes": representative.map(|sample| sample.bytes),
                "graphql_cost": representative.map(|sample| sample.cost),
                "graphql_node_count": representative.map(|sample| sample.node_count),
                "files_returned": representative.map(|sample| sample.files_returned),
                "files_total": representative.map(|sample| sample.files_total),
                "files_truncated": representative.map(|sample| sample.files_truncated),
                "threads_returned": representative.map(|sample| sample.threads_returned),
                "threads_total": representative.map(|sample| sample.threads_total),
                "threads_truncated": representative.map(|sample| sample.threads_truncated),
                "statuses": samples.iter().map(|sample| sample.status).collect::<Vec<u16>>(),
                "graphql_errors": samples
                    .iter()
                    .flat_map(|sample| sample.graphql_errors.clone())
                    .collect::<Vec<String>>(),
            })
        })
        .collect();

    json!({
        "scenario": "j1a",
        "measured_at": started_at,
        "machine": crate::machine(),
        "raw_log": raw_log,
        "rows": rows,
    })
}
