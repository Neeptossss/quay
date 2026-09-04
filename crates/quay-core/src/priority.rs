#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    Speculative,
    Warm,
    Hot,
    User,
}

impl Priority {
    pub fn cancels_in_flight(self, in_flight: Priority) -> bool {
        self == Priority::User && in_flight.is_cancellable()
    }

    pub fn is_cancellable(self) -> bool {
        self == Priority::Speculative
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Priority::Speculative => "speculative",
            Priority::Warm => "warm",
            Priority::Hot => "hot",
            Priority::User => "user",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Priority;

    #[test]
    fn user_outranks_every_other_priority() {
        assert!(Priority::User > Priority::Hot);
        assert!(Priority::Hot > Priority::Warm);
        assert!(Priority::Warm > Priority::Speculative);
    }

    #[test]
    fn scheduling_order_puts_user_first_and_speculative_last() {
        let mut queue = vec![
            Priority::Warm,
            Priority::Speculative,
            Priority::User,
            Priority::Hot,
        ];
        queue.sort_by(|a, b| b.cmp(a));
        assert_eq!(
            queue,
            vec![
                Priority::User,
                Priority::Hot,
                Priority::Warm,
                Priority::Speculative
            ]
        );
    }

    #[test]
    fn user_request_cancels_speculative_in_flight_request() {
        assert!(Priority::User.cancels_in_flight(Priority::Speculative));
    }

    #[test]
    fn user_request_never_cancels_non_speculative_in_flight_request() {
        assert!(!Priority::User.cancels_in_flight(Priority::Warm));
        assert!(!Priority::User.cancels_in_flight(Priority::Hot));
        assert!(!Priority::User.cancels_in_flight(Priority::User));
    }

    #[test]
    fn hot_request_never_cancels_a_speculative_in_flight_request() {
        assert!(!Priority::Hot.cancels_in_flight(Priority::Speculative));
        assert!(!Priority::Warm.cancels_in_flight(Priority::Speculative));
    }

    #[test]
    fn only_speculative_is_cancellable() {
        assert!(Priority::Speculative.is_cancellable());
        assert!(!Priority::Warm.is_cancellable());
        assert!(!Priority::Hot.is_cancellable());
        assert!(!Priority::User.is_cancellable());
    }
}
