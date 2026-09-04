use std::path::PathBuf;

use crate::paths;
use crate::summary::Summary;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bound {
    AtMost,
    AtLeast,
}

pub struct Budget {
    pub id: &'static str,
    pub metric: &'static str,
    pub bound: Bound,
    pub limit: f64,
    pub unit: &'static str,
    pub statistic: &'static str,
    pub instrument: &'static str,
    pub specification: &'static str,
}

pub const BUDGETS: [Budget; 9] = [
    Budget {
        id: "cold_start_to_first_paint",
        metric: "cold start to first painted list",
        bound: Bound::AtMost,
        limit: 400.0,
        unit: "ms",
        statistic: "median",
        instrument: "tauri trace, process start to paint",
        specification: "§4",
    },
    Budget {
        id: "keystroke_to_pixel",
        metric: "keystroke to updated pixel",
        bound: Bound::AtMost,
        limit: 16.0,
        unit: "ms",
        statistic: "p99",
        instrument: "frontend instrumentation, performance.now()",
        specification: "§4",
    },
    Budget {
        id: "cached_pull_request_navigation",
        metric: "navigation to the next pull request from cache",
        bound: Bound::AtMost,
        limit: 50.0,
        unit: "ms",
        statistic: "median",
        instrument: "frontend instrumentation, performance.now()",
        specification: "§4",
    },
    Budget {
        id: "inbox_query",
        metric: "filtered inbox query on SQLite",
        bound: Bound::AtMost,
        limit: 5.0,
        unit: "ms",
        statistic: "p99",
        instrument: "xtask measure j1b, reference dataset",
        specification: "§4",
    },
    Budget {
        id: "diff_open_two_thousand_lines",
        metric: "opening a 2000 line diff",
        bound: Bound::AtMost,
        limit: 120.0,
        unit: "ms",
        statistic: "median",
        instrument: "bench, virtualised rendering",
        specification: "§4",
    },
    Budget {
        id: "resident_memory_idle",
        metric: "resident memory at rest, 20 repositories synchronised",
        bound: Bound::AtMost,
        limit: 250.0,
        unit: "MB",
        statistic: "max",
        instrument: "process measurement",
        specification: "§4",
    },
    Budget {
        id: "background_cpu_idle",
        metric: "cpu at rest, window in the background",
        bound: Bound::AtMost,
        limit: 0.5,
        unit: "%",
        statistic: "mean",
        instrument: "process measurement",
        specification: "§4",
    },
    Budget {
        id: "github_quota_per_hour",
        metric: "GitHub quota consumed in normal operation",
        bound: Bound::AtMost,
        limit: 1_500.0,
        unit: "req/h",
        statistic: "max",
        instrument: "rate governor counter",
        specification: "§4",
    },
    Budget {
        id: "cache_served_navigations",
        metric: "navigations served from cache in under 50 ms",
        bound: Bound::AtLeast,
        limit: 85.0,
        unit: "%",
        statistic: "sliding window",
        instrument: "preload_outcome",
        specification: "§4",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Missing,
    Over,
    Met,
}

impl Verdict {
    pub fn is_red(self) -> bool {
        self != Verdict::Met
    }

    pub fn label(self) -> &'static str {
        match self {
            Verdict::Missing => "MISSING",
            Verdict::Over => "OVER",
            Verdict::Met => "MET",
        }
    }
}

pub struct BudgetStatus {
    pub budget: &'static Budget,
    pub summary: Option<Summary>,
    pub verdict: Verdict,
}

impl Budget {
    pub fn summary_path(&self) -> PathBuf {
        paths::summaries().join(format!("{}.json", self.id))
    }

    pub fn satisfied_by(&self, value: f64) -> bool {
        match self.bound {
            Bound::AtMost => value <= self.limit,
            Bound::AtLeast => value >= self.limit,
        }
    }
}

pub fn status() -> Vec<BudgetStatus> {
    BUDGETS
        .iter()
        .map(|budget| {
            let summary = Summary::load(&budget.summary_path());
            let verdict = match summary.as_ref() {
                None => Verdict::Missing,
                Some(summary) if budget.satisfied_by(summary.value) => Verdict::Met,
                Some(_) => Verdict::Over,
            };
            BudgetStatus {
                budget,
                summary,
                verdict,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{BUDGETS, Bound, Verdict};

    #[test]
    fn every_budget_of_the_specification_is_declared_once() {
        let mut identifiers: Vec<&str> = BUDGETS.iter().map(|budget| budget.id).collect();
        identifiers.sort_unstable();
        let count = identifiers.len();
        identifiers.dedup();
        assert_eq!(identifiers.len(), count);
        assert_eq!(count, 9);
    }

    #[test]
    fn an_upper_bound_budget_rejects_a_value_above_its_limit() {
        match BUDGETS.iter().find(|budget| budget.id == "inbox_query") {
            None => panic!("the inbox budget is missing"),
            Some(budget) => {
                assert_eq!(budget.bound, Bound::AtMost);
                assert!(budget.satisfied_by(4.9));
                assert!(budget.satisfied_by(5.0));
                assert!(!budget.satisfied_by(5.1));
            }
        }
    }

    #[test]
    fn a_lower_bound_budget_rejects_a_value_below_its_limit() {
        match BUDGETS
            .iter()
            .find(|budget| budget.id == "cache_served_navigations")
        {
            None => panic!("the cache hit budget is missing"),
            Some(budget) => {
                assert_eq!(budget.bound, Bound::AtLeast);
                assert!(budget.satisfied_by(85.0));
                assert!(!budget.satisfied_by(84.9));
            }
        }
    }

    #[test]
    fn a_budget_without_a_measurement_is_red() {
        assert!(Verdict::Missing.is_red());
        assert!(Verdict::Over.is_red());
        assert!(!Verdict::Met.is_red());
    }
}
