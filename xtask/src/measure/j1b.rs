use std::error::Error;
use std::hint::black_box;
use std::io::Write;
use std::time::Instant;

use quay_store::dataset::{DatasetShape, seed};
use quay_store::inbox::{InboxFilter, InboxQuery, run};
use quay_store::schema::{self, SchemaVariant};

use crate::paths;
use crate::stats::Percentiles;
use crate::summary::Summary;

const SCALE_FACTORS: [usize; 3] = [1, 10, 100];
const WARM_ITERATIONS_REFERENCE: usize = 10_000;
const WARM_ITERATIONS_SCALED: usize = 2_000;
const COLD_ITERATIONS: usize = 100;

pub struct QueryMeasurement {
    pub schema: &'static str,
    pub dataset: String,
    pub scale: usize,
    pub query: &'static str,
    pub cache: &'static str,
    pub rows: usize,
    pub plan: String,
    pub percentiles: Percentiles,
    pub raw_log: String,
}

struct Run<'a> {
    started_at: &'a str,
    variant: SchemaVariant,
    shape: &'a DatasetShape,
    scale: usize,
    query: InboxQuery,
    cache: &'static str,
    rows: usize,
    plan: &'a str,
}

pub fn measure(started_at: &str) -> Result<Vec<QueryMeasurement>, Box<dyn Error>> {
    std::fs::create_dir_all(paths::raw())?;
    let mut measurements = Vec::new();

    for variant in SchemaVariant::ALL {
        for scale in SCALE_FACTORS {
            measurements.extend(measure_dataset(started_at, variant, scale)?);
        }
    }

    write_budget_summary(started_at, &measurements)?;
    Ok(measurements)
}

fn measure_dataset(
    started_at: &str,
    variant: SchemaVariant,
    scale: usize,
) -> Result<Vec<QueryMeasurement>, Box<dyn Error>> {
    let shape = if scale == 1 {
        DatasetShape::reference()
    } else {
        DatasetShape::scaled(scale)
    };
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("quay.db");
    println!("j1b: seeding {} on schema {}", shape.label(), variant.id());
    let mut connection = schema::create(&database, variant)?;
    seed(&mut connection, &shape, variant)?;
    drop(connection);

    let warm_iterations = if scale == 1 {
        WARM_ITERATIONS_REFERENCE
    } else {
        WARM_ITERATIONS_SCALED
    };
    let filter = InboxFilter::default();
    let mut measurements = Vec::new();

    for query in InboxQuery::for_schema(variant) {
        let connection = schema::open(&database)?;
        let plan = explain(&connection, query)?;
        let rows = run(&connection, query, &filter)?.len();

        let mut warm = Vec::with_capacity(warm_iterations);
        for _ in 0..warm_iterations {
            let started = Instant::now();
            let result = run(&connection, query, &filter)?;
            warm.push(started.elapsed().as_secs_f64() * 1_000.0);
            black_box(result);
        }
        drop(connection);
        measurements.push(record(
            &Run {
                started_at,
                variant,
                shape: &shape,
                scale,
                query,
                cache: "warm",
                rows,
                plan: &plan,
            },
            warm,
        )?);

        let mut cold = Vec::with_capacity(COLD_ITERATIONS);
        for _ in 0..COLD_ITERATIONS {
            let connection = schema::open(&database)?;
            let started = Instant::now();
            let result = run(&connection, query, &filter)?;
            cold.push(started.elapsed().as_secs_f64() * 1_000.0);
            black_box(result);
            drop(connection);
        }
        measurements.push(record(
            &Run {
                started_at,
                variant,
                shape: &shape,
                scale,
                query,
                cache: "cold",
                rows,
                plan: &plan,
            },
            cold,
        )?);
    }
    Ok(measurements)
}

fn record(run: &Run<'_>, samples: Vec<f64>) -> Result<QueryMeasurement, Box<dyn Error>> {
    let Run {
        started_at,
        variant,
        shape,
        scale,
        query,
        cache,
        rows,
        plan,
    } = run;
    let file_name = format!(
        "j1b-{}-{}-{}-{}-{}.csv",
        started_at.replace(':', ""),
        variant.id(),
        shape.label(),
        query.id(),
        cache
    );
    let path = paths::raw().join(&file_name);
    let mut file = std::fs::File::create(&path)?;
    writeln!(file, "elapsed_ms")?;
    for sample in &samples {
        writeln!(file, "{sample:.6}")?;
    }

    let percentiles = Percentiles::of(&samples).ok_or("an empty sample cannot be summarised")?;
    println!(
        "j1b: {} {} {} {} rows={} p50={:.3}ms p99={:.3}ms max={:.3}ms",
        variant.id(),
        shape.label(),
        query.id(),
        cache,
        rows,
        percentiles.p50,
        percentiles.p99,
        percentiles.max
    );

    Ok(QueryMeasurement {
        schema: variant.id(),
        dataset: shape.label(),
        scale: *scale,
        query: query.id(),
        cache,
        rows: *rows,
        plan: (*plan).to_owned(),
        percentiles,
        raw_log: format!("measurements/raw/{file_name}"),
    })
}

fn write_budget_summary(
    started_at: &str,
    measurements: &[QueryMeasurement],
) -> Result<(), Box<dyn Error>> {
    let worst = measurements
        .iter()
        .filter(|measurement| {
            measurement.schema == SchemaVariant::Corrected.id()
                && measurement.scale == 1
                && measurement.cache == "warm"
        })
        .max_by(|left, right| left.percentiles.p99.total_cmp(&right.percentiles.p99))
        .ok_or("no reference measurement was produced")?;

    Summary {
        budget_id: "inbox_query".to_owned(),
        scenario: format!("j1b {} {} {}", worst.schema, worst.dataset, worst.query),
        statistic: "p99".to_owned(),
        value: worst.percentiles.p99,
        unit: "ms".to_owned(),
        percentiles: Some(worst.percentiles),
        measured_at: started_at.to_owned(),
        machine: crate::machine(),
        raw_log: worst.raw_log.clone(),
    }
    .save(&paths::summaries().join("inbox_query.json"))?;
    Ok(())
}

fn explain(connection: &rusqlite::Connection, query: InboxQuery) -> rusqlite::Result<String> {
    let sql = format!("EXPLAIN QUERY PLAN {}", query.sql());
    let mut statement = connection.prepare(&sql)?;
    let uses_second_parameter = query.sql().contains("?2");
    let steps: Vec<String> = if uses_second_parameter {
        statement
            .query_map((50i64, "viewer"), |row| row.get::<_, String>(3))?
            .collect::<rusqlite::Result<Vec<String>>>()?
    } else {
        statement
            .query_map((50i64,), |row| row.get::<_, String>(3))?
            .collect::<rusqlite::Result<Vec<String>>>()?
    };
    Ok(steps.join(" ; "))
}
