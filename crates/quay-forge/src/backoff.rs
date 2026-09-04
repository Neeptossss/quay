use std::time::Duration;

pub struct Backoff {
    base: Duration,
    ceiling: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            base: Duration::from_secs(1),
            ceiling: Duration::from_secs(900),
        }
    }
}

impl Backoff {
    pub fn new(base: Duration, ceiling: Duration) -> Self {
        Self { base, ceiling }
    }

    pub fn delay(&self, attempt: u32, retry_after: Option<Duration>, jitter: f64) -> Duration {
        if let Some(advised) = retry_after {
            return advised.min(self.ceiling);
        }
        let exponent = attempt.min(20);
        let scaled = self.base.saturating_mul(1u32 << exponent);
        let capped = scaled.min(self.ceiling);
        capped.mul_f64(jitter.clamp(0.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::Backoff;
    use std::time::Duration;

    #[test]
    fn a_retry_after_header_wins_over_the_computed_delay() {
        let backoff = Backoff::default();
        assert_eq!(
            backoff.delay(5, Some(Duration::from_secs(42)), 1.0),
            Duration::from_secs(42)
        );
    }

    #[test]
    fn a_retry_after_header_is_still_capped_by_the_ceiling() {
        let backoff = Backoff::default();
        assert_eq!(
            backoff.delay(0, Some(Duration::from_secs(7_200)), 1.0),
            Duration::from_secs(900)
        );
    }

    #[test]
    fn the_delay_doubles_with_each_attempt_from_one_second() {
        let backoff = Backoff::default();
        assert_eq!(backoff.delay(0, None, 1.0), Duration::from_secs(1));
        assert_eq!(backoff.delay(1, None, 1.0), Duration::from_secs(2));
        assert_eq!(backoff.delay(2, None, 1.0), Duration::from_secs(4));
        assert_eq!(backoff.delay(3, None, 1.0), Duration::from_secs(8));
    }

    #[test]
    fn the_delay_never_exceeds_fifteen_minutes() {
        let backoff = Backoff::default();
        assert_eq!(backoff.delay(30, None, 1.0), Duration::from_secs(900));
    }

    #[test]
    fn jitter_shortens_the_delay_without_ever_lengthening_it() {
        let backoff = Backoff::default();
        assert_eq!(backoff.delay(3, None, 0.5), Duration::from_secs(4));
        assert_eq!(backoff.delay(3, None, 0.0), Duration::ZERO);
        assert_eq!(backoff.delay(3, None, 2.0), Duration::from_secs(8));
    }

    #[test]
    fn a_custom_ceiling_is_respected() {
        let backoff = Backoff::new(Duration::from_secs(1), Duration::from_secs(10));
        assert_eq!(backoff.delay(9, None, 1.0), Duration::from_secs(10));
    }
}
