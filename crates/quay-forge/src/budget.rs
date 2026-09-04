use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub struct SlidingBudget {
    limit: u32,
    window: Duration,
    issued: VecDeque<Instant>,
}

impl SlidingBudget {
    pub fn new(limit: u32, window: Duration) -> Self {
        Self {
            limit,
            window,
            issued: VecDeque::new(),
        }
    }

    pub fn per_hour(limit: u32) -> Self {
        Self::new(limit, Duration::from_secs(3_600))
    }

    pub fn limit(&self) -> u32 {
        self.limit
    }

    pub fn reserve(&mut self, now: Instant) -> Result<(), Duration> {
        self.forget_expired(now);
        if (self.issued.len() as u32) < self.limit {
            self.issued.push_back(now);
            return Ok(());
        }
        let oldest = self.issued.front().copied().unwrap_or(now);
        Err(self
            .window
            .saturating_sub(now.saturating_duration_since(oldest)))
    }

    pub fn refund(&mut self, now: Instant) {
        self.forget_expired(now);
        self.issued.pop_back();
    }

    pub fn used(&mut self, now: Instant) -> u32 {
        self.forget_expired(now);
        self.issued.len() as u32
    }

    fn forget_expired(&mut self, now: Instant) {
        while let Some(oldest) = self.issued.front() {
            if now.saturating_duration_since(*oldest) >= self.window {
                self.issued.pop_front();
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SlidingBudget;
    use std::time::{Duration, Instant};

    #[test]
    fn a_budget_refuses_the_request_that_would_exceed_it() {
        let mut budget = SlidingBudget::new(2, Duration::from_secs(60));
        let now = Instant::now();
        assert!(budget.reserve(now).is_ok());
        assert!(budget.reserve(now).is_ok());
        assert!(budget.reserve(now).is_err());
    }

    #[test]
    fn a_refused_request_reports_how_long_to_wait() {
        let mut budget = SlidingBudget::new(1, Duration::from_secs(60));
        let start = Instant::now();
        assert!(budget.reserve(start).is_ok());
        match budget.reserve(start + Duration::from_secs(10)) {
            Ok(()) => panic!("the budget should have been exhausted"),
            Err(wait) => assert_eq!(wait, Duration::from_secs(50)),
        }
    }

    #[test]
    fn a_slot_frees_up_once_it_leaves_the_window() {
        let mut budget = SlidingBudget::new(1, Duration::from_secs(60));
        let start = Instant::now();
        assert!(budget.reserve(start).is_ok());
        assert!(budget.reserve(start + Duration::from_secs(59)).is_err());
        assert!(budget.reserve(start + Duration::from_secs(60)).is_ok());
    }

    #[test]
    fn a_budget_of_zero_refuses_every_request() {
        let mut budget = SlidingBudget::new(0, Duration::from_secs(60));
        assert!(budget.reserve(Instant::now()).is_err());
    }

    #[test]
    fn the_hourly_budget_of_the_specification_is_fifteen_hundred_requests() {
        assert_eq!(SlidingBudget::per_hour(1_500).limit(), 1_500);
    }

    #[test]
    fn a_refunded_slot_becomes_available_again() {
        let mut budget = SlidingBudget::new(1, Duration::from_secs(60));
        let now = Instant::now();
        assert!(budget.reserve(now).is_ok());
        assert!(budget.reserve(now).is_err());
        budget.refund(now);
        assert!(budget.reserve(now).is_ok());
    }

    #[test]
    fn refunding_an_unused_budget_does_not_go_negative() {
        let mut budget = SlidingBudget::new(2, Duration::from_secs(60));
        let now = Instant::now();
        budget.refund(now);
        budget.refund(now);
        assert_eq!(budget.used(now), 0);
        assert!(budget.reserve(now).is_ok());
        assert!(budget.reserve(now).is_ok());
        assert!(budget.reserve(now).is_err());
    }

    #[test]
    fn usage_drops_back_to_zero_once_the_window_has_passed() {
        let mut budget = SlidingBudget::new(4, Duration::from_secs(60));
        let start = Instant::now();
        for _ in 0..4 {
            assert!(budget.reserve(start).is_ok());
        }
        assert_eq!(budget.used(start), 4);
        assert_eq!(budget.used(start + Duration::from_secs(61)), 0);
    }
}
