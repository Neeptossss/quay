#![forbid(unsafe_code)]

mod budgets;
mod measure;
mod paths;
mod report;
mod stats;
mod summary;
mod sync_once;

use std::process::ExitCode;

pub fn machine() -> String {
    format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let command = arguments.first().map(String::as_str);
    let argument = arguments.get(1).map(String::as_str);

    match (command, argument) {
        (Some("budgets"), _) => budgets_command(),
        (Some("measure"), Some("m0-1")) => measure_command(Scenario::M01),
        (Some("measure"), Some("inbox-real")) => measure_command(Scenario::InboxReal),
        (Some("measure"), Some("j1a")) => measure_command(Scenario::J1a),
        (Some("measure"), Some("j1b")) => measure_command(Scenario::J1b),
        (Some("measure"), Some("all")) => measure_command(Scenario::All),
        (Some("report"), _) => report_command(),
        (Some("sync-once"), _) => sync_once_command(),
        _ => usage(),
    }
}

enum Scenario {
    InboxReal,
    M01,
    J1a,
    J1b,
    All,
}

fn usage() -> ExitCode {
    eprintln!("usage: cargo run -p xtask -- <command>");
    eprintln!("  budgets        check every performance budget of the specification");
    eprintln!("  measure m0-1   probe which authentication the notifications endpoint accepts");
    eprintln!("  measure inbox-real  time the inbox against the database the quay binary filled");
    eprintln!("  measure j1a    measure the GraphQL pull request detail breaking point");
    eprintln!("  measure j1b    measure the inbox query against the reference dataset");
    eprintln!("  measure all    run both measurements");
    eprintln!("  report         regenerate measurements/REPORT.md from the raw logs");
    eprintln!("  sync-once      run one full fetch tick against the forge and read the inbox back");
    ExitCode::from(2)
}

fn budgets_command() -> ExitCode {
    let statuses = budgets::status();
    let width = budgets::BUDGETS
        .iter()
        .map(|budget| budget.id.len())
        .max()
        .unwrap_or(0);

    for status in &statuses {
        let measured = match status.summary.as_ref() {
            Some(summary) => format!("{:.3} {}", summary.value, summary.unit),
            None => "not measured".to_owned(),
        };
        println!(
            "{:<width$}  {:<7}  budget {} {} {}  measured {}",
            status.budget.id,
            status.verdict.label(),
            match status.budget.bound {
                budgets::Bound::AtMost => "<=",
                budgets::Bound::AtLeast => ">=",
            },
            status.budget.limit,
            status.budget.unit,
            measured,
            width = width
        );
    }

    let red = statuses
        .iter()
        .filter(|status| status.verdict.is_red())
        .count();
    println!("\n{} budget(s) red out of {}", red, statuses.len());
    if red == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn measure_command(scenario: Scenario) -> ExitCode {
    let started_at = measure::now_utc();
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("the async runtime could not start: {error}");
            return ExitCode::FAILURE;
        }
    };

    if matches!(scenario, Scenario::J1b | Scenario::All) {
        match measure::j1b::measure(&started_at) {
            Ok(measurements) => {
                let summary = measure::j1b_summary(&measurements, &started_at);
                if let Err(error) = measure::write_scenario_summary("j1b", summary) {
                    eprintln!("the J1-b summary could not be written: {error}");
                    return ExitCode::FAILURE;
                }
            }
            Err(error) => {
                eprintln!("J1-b failed: {error}");
                return ExitCode::FAILURE;
            }
        }
    }

    if matches!(scenario, Scenario::InboxReal) {
        match measure::inbox_real::measure(&started_at) {
            Ok(inbox) => {
                let summary = measure::inbox_real::summary(&inbox, &started_at);
                if let Err(error) = measure::write_scenario_summary("inbox-real", summary) {
                    eprintln!("the real inbox summary could not be written: {error}");
                    return ExitCode::FAILURE;
                }
            }
            Err(error) => {
                eprintln!("the real inbox measurement failed: {error}");
                return ExitCode::FAILURE;
            }
        }
    }

    if matches!(scenario, Scenario::M01 | Scenario::All) {
        let raw_log = format!(
            "measurements/raw/m0-1-{}.jsonl",
            started_at.replace(':', "")
        );
        match runtime.block_on(measure::m0_1::measure(&started_at)) {
            Ok(observations) => {
                let summary = measure::m0_1::summary(&observations, &started_at, &raw_log);
                if let Err(error) = measure::write_scenario_summary("m0-1", summary) {
                    eprintln!("the M0-1 summary could not be written: {error}");
                    return ExitCode::FAILURE;
                }
            }
            Err(error) => {
                eprintln!("M0-1 failed: {error}");
                return ExitCode::FAILURE;
            }
        }
    }

    if matches!(scenario, Scenario::J1a | Scenario::All) {
        let raw_log = format!("measurements/raw/j1a-{}.jsonl", started_at.replace(':', ""));
        match runtime.block_on(measure::j1a::measure(&started_at)) {
            Ok(samples) => {
                let summary = measure::j1a_summary(&samples, &started_at, &raw_log);
                if let Err(error) = measure::write_scenario_summary("j1a", summary) {
                    eprintln!("the J1-a summary could not be written: {error}");
                    return ExitCode::FAILURE;
                }
            }
            Err(error) => {
                eprintln!("J1-a failed: {error}");
                return ExitCode::FAILURE;
            }
        }
    }

    report_command()
}

fn sync_once_command() -> ExitCode {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("the async runtime could not start: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(sync_once::run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("the fetch path failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn report_command() -> ExitCode {
    match report::generate() {
        Ok(path) => {
            println!("report written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("the report could not be generated: {error}");
            ExitCode::FAILURE
        }
    }
}
