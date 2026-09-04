use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::stats::Percentiles;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub budget_id: String,
    pub scenario: String,
    pub statistic: String,
    pub value: f64,
    pub unit: String,
    pub percentiles: Option<Percentiles>,
    pub measured_at: String,
    pub machine: String,
    pub raw_log: String,
}

impl Summary {
    pub fn load(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let rendered = serde_json::to_string_pretty(self)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        std::fs::write(path, format!("{rendered}\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::Summary;
    use crate::stats::Percentiles;

    fn sample() -> Summary {
        Summary {
            budget_id: "inbox_query".to_owned(),
            scenario: "j1b".to_owned(),
            statistic: "p99".to_owned(),
            value: 0.42,
            unit: "ms".to_owned(),
            percentiles: Percentiles::of(&[0.1, 0.2, 0.42]),
            measured_at: "2026-09-04T10:00:00Z".to_owned(),
            machine: "aarch64-apple-darwin".to_owned(),
            raw_log: "measurements/raw/example.csv".to_owned(),
        }
    }

    #[test]
    fn a_summary_survives_a_round_trip_through_disk() {
        let directory = match tempfile::tempdir() {
            Ok(directory) => directory,
            Err(error) => panic!("temporary directory: {error}"),
        };
        let path = directory.path().join("inbox_query.json");
        match sample().save(&path) {
            Ok(()) => {}
            Err(error) => panic!("saving the summary: {error}"),
        }
        match Summary::load(&path) {
            None => panic!("the summary should load back"),
            Some(loaded) => {
                assert_eq!(loaded.budget_id, "inbox_query");
                assert_eq!(loaded.value, 0.42);
            }
        }
    }

    #[test]
    fn a_missing_summary_reads_as_absent_rather_than_failing() {
        assert!(Summary::load(std::path::Path::new("/nonexistent/summary.json")).is_none());
    }

    #[test]
    fn a_corrupt_summary_reads_as_absent_rather_than_as_a_measurement() {
        let directory = match tempfile::tempdir() {
            Ok(directory) => directory,
            Err(error) => panic!("temporary directory: {error}"),
        };
        let path = directory.path().join("broken.json");
        match std::fs::write(&path, "{ this is not json") {
            Ok(()) => {}
            Err(error) => panic!("writing the corrupt file: {error}"),
        }
        assert!(Summary::load(&path).is_none());
    }
}
