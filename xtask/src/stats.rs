use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Percentiles {
    pub samples: usize,
    pub min: f64,
    pub mean: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
}

impl Percentiles {
    pub fn of(samples: &[f64]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }
        let mut sorted = samples.to_vec();
        sorted.sort_by(|left, right| left.total_cmp(right));
        let sum: f64 = sorted.iter().sum();
        Some(Self {
            samples: sorted.len(),
            min: quantile(&sorted, 0.0),
            mean: sum / sorted.len() as f64,
            p50: quantile(&sorted, 0.50),
            p95: quantile(&sorted, 0.95),
            p99: quantile(&sorted, 0.99),
            max: quantile(&sorted, 1.0),
        })
    }
}

fn quantile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let last = sorted.len() - 1;
    let rank = (fraction * last as f64).round() as usize;
    sorted[rank.min(last)]
}

#[cfg(test)]
mod tests {
    use super::Percentiles;

    #[test]
    fn an_empty_sample_yields_no_percentiles() {
        assert!(Percentiles::of(&[]).is_none());
    }

    #[test]
    fn a_single_sample_is_every_percentile() {
        match Percentiles::of(&[7.0]) {
            None => panic!("a single sample must produce percentiles"),
            Some(percentiles) => {
                assert_eq!(percentiles.min, 7.0);
                assert_eq!(percentiles.p50, 7.0);
                assert_eq!(percentiles.p99, 7.0);
                assert_eq!(percentiles.max, 7.0);
            }
        }
    }

    #[test]
    fn percentiles_are_read_from_the_sorted_sample() {
        let samples: Vec<f64> = (1..=100).map(f64::from).collect();
        match Percentiles::of(&samples) {
            None => panic!("percentiles are expected"),
            Some(percentiles) => {
                assert_eq!(percentiles.samples, 100);
                assert_eq!(percentiles.min, 1.0);
                assert_eq!(percentiles.max, 100.0);
                assert_eq!(percentiles.p50, 51.0);
                assert_eq!(percentiles.p99, 99.0);
                assert!((percentiles.mean - 50.5).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn an_unsorted_sample_is_ordered_before_reading() {
        match Percentiles::of(&[9.0, 1.0, 5.0]) {
            None => panic!("percentiles are expected"),
            Some(percentiles) => {
                assert_eq!(percentiles.min, 1.0);
                assert_eq!(percentiles.max, 9.0);
                assert_eq!(percentiles.p50, 5.0);
            }
        }
    }

    #[test]
    fn every_percentile_stays_within_the_observed_range() {
        let samples: Vec<f64> = (1..=1_000).map(f64::from).collect();
        match Percentiles::of(&samples) {
            None => panic!("percentiles are expected"),
            Some(percentiles) => {
                assert!(percentiles.p50 <= percentiles.p95);
                assert!(percentiles.p95 <= percentiles.p99);
                assert!(percentiles.p99 <= percentiles.max);
                assert!(percentiles.min <= percentiles.p50);
            }
        }
    }
}
