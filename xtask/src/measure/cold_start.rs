use std::error::Error;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

use crate::paths;
use crate::stats::Percentiles;

const LAUNCHES: usize = 5;
const PHASES: [&str; 5] = [
    "store opened",
    "builder about to run",
    "setup finished",
    "frontend bundle running",
    "first paint",
];

pub struct PhaseMeasurement {
    pub phase: &'static str,
    pub percentiles: Percentiles,
}

pub struct ColdStart {
    pub binary: String,
    pub launches: usize,
    pub phases: Vec<PhaseMeasurement>,
    pub raw_log: String,
}

pub fn measure(started_at: &str) -> Result<ColdStart, Box<dyn Error>> {
    let binary = paths::workspace_root().join("target/release/quay");
    if !binary.exists() {
        return Err("build the release binary first: cargo build --release -p quay-app".into());
    }
    std::fs::create_dir_all(paths::raw())?;

    let file_name = format!("cold-start-{}.jsonl", started_at.replace(':', ""));
    let mut log = std::fs::File::create(paths::raw().join(&file_name))?;
    let mut samples: Vec<Vec<f64>> = vec![Vec::new(); PHASES.len()];

    for launch in 0..LAUNCHES {
        let mut child = Command::new(&binary)
            .arg("ui")
            .env("RUST_LOG", "quay_app=info")
            .env("QUAY_READONLY", "1")
            .env("NO_COLOR", "1")
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        std::thread::sleep(Duration::from_secs(9));
        let _ = child.kill();
        let output = child.wait_with_output()?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        for (rank, phase) in PHASES.iter().enumerate() {
            if let Some(milliseconds) = read_phase(&text, phase) {
                samples[rank].push(milliseconds);
                writeln!(
                    log,
                    "{}",
                    json!({
                        "scenario": "cold-start",
                        "run_started_at": started_at,
                        "machine": crate::machine(),
                        "launch": launch,
                        "phase": phase,
                        "elapsed_ms": milliseconds,
                    })
                )?;
            }
        }
    }

    let mut phases = Vec::new();
    for (rank, phase) in PHASES.iter().enumerate() {
        let Some(percentiles) = Percentiles::of(&samples[rank]) else {
            continue;
        };
        println!(
            "cold-start: {phase} p50={:.0}ms max={:.0}ms sur {} lancement(s)",
            percentiles.p50, percentiles.max, percentiles.samples
        );
        phases.push(PhaseMeasurement { phase, percentiles });
    }
    if phases.is_empty() {
        return Err("no launch reported a phase; is the binary logging at info level?".into());
    }

    Ok(ColdStart {
        binary: binary.display().to_string(),
        launches: LAUNCHES,
        phases,
        raw_log: format!("measurements/raw/{file_name}"),
    })
}

fn without_escapes(line: &str) -> String {
    let mut plain = String::with_capacity(line.len());
    let mut characters = line.chars();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            plain.push(character);
            continue;
        }
        for escaped in characters.by_ref() {
            if escaped.is_ascii_alphabetic() {
                break;
            }
        }
    }
    plain
}

fn read_phase(text: &str, phase: &str) -> Option<f64> {
    let line = text
        .lines()
        .map(without_escapes)
        .find(|line| line.contains(phase) && line.contains("elapsed_ms"))?;
    let tail = &line[line.find("elapsed_ms")? + "elapsed_ms".len()..];
    let number: String = tail
        .chars()
        .skip_while(|character| !character.is_ascii_digit())
        .take_while(|character| character.is_ascii_digit() || *character == '.')
        .collect();
    number.parse().ok()
}

pub fn summary(cold: &ColdStart, started_at: &str) -> Value {
    json!({
        "scenario": "cold-start",
        "measured_at": started_at,
        "machine": crate::machine(),
        "binary": cold.binary,
        "launches": cold.launches,
        "raw_log": cold.raw_log,
        "rows": cold
            .phases
            .iter()
            .map(|phase| json!({
                "phase": phase.phase,
                "percentiles": phase.percentiles,
            }))
            .collect::<Vec<Value>>(),
    })
}

pub fn budget_summary(cold: &ColdStart, started_at: &str) -> Option<crate::summary::Summary> {
    let painted = cold
        .phases
        .iter()
        .find(|phase| phase.phase == "first paint")?;
    Some(crate::summary::Summary {
        budget_id: "cold_start_to_first_paint".to_owned(),
        scenario: "cold-start, process launch to the first painted list".to_owned(),
        statistic: "median".to_owned(),
        value: painted.percentiles.p50,
        unit: "ms".to_owned(),
        percentiles: Some(painted.percentiles),
        measured_at: started_at.to_owned(),
        machine: crate::machine(),
        raw_log: cold.raw_log.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::read_phase;

    #[test]
    fn a_plain_line_yields_its_elapsed_milliseconds() {
        let line = "2026-09-04T17:29:30Z  INFO quay_app::ui: first paint elapsed_ms=4489.16";
        assert_eq!(read_phase(line, "first paint"), Some(4489.16));
    }

    #[test]
    fn a_line_dressed_in_terminal_escapes_is_still_read() {
        let line = "\u{1b}[2mquay_app::ui\u{1b}[0m: store opened \u{1b}[3melapsed_ms\u{1b}[0m\u{1b}[2m=\u{1b}[0m4.15";
        assert_eq!(read_phase(line, "store opened"), Some(4.15));
    }

    #[test]
    fn an_escape_that_carries_a_digit_is_never_read_as_the_value() {
        let line = "\u{1b}[0m store opened \u{1b}[3melapsed_ms\u{1b}[0m\u{1b}[2m=\u{1b}[0m12.5";
        assert_eq!(read_phase(line, "store opened"), Some(12.5));
    }

    #[test]
    fn an_absent_phase_yields_nothing_rather_than_zero() {
        assert_eq!(read_phase("nothing here", "first paint"), None);
    }
}
