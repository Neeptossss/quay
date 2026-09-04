use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    Cold,
    Warm,
    Hot,
}

impl Tier {
    pub const ALL: [Tier; 3] = [Tier::Hot, Tier::Warm, Tier::Cold];

    pub fn id(self) -> &'static str {
        match self {
            Tier::Hot => "hot",
            Tier::Warm => "warm",
            Tier::Cold => "cold",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        match id {
            "hot" => Some(Tier::Hot),
            "warm" => Some(Tier::Warm),
            "cold" => Some(Tier::Cold),
            _ => None,
        }
    }

    pub fn refresh_interval(self) -> Duration {
        match self {
            Tier::Hot => Duration::from_secs(10),
            Tier::Warm => Duration::from_secs(60),
            Tier::Cold => Duration::from_secs(6 * 3_600),
        }
    }

    pub fn interval_respecting(self, advised: Option<Duration>) -> Duration {
        match advised {
            Some(advised) => advised.max(self.refresh_interval()),
            None => self.refresh_interval(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::Tier;

    #[test]
    fn a_hotter_tier_sorts_above_a_colder_one() {
        assert!(Tier::Hot > Tier::Warm);
        assert!(Tier::Warm > Tier::Cold);
    }

    #[test]
    fn every_tier_survives_a_round_trip_through_its_stored_identifier() {
        for tier in Tier::ALL {
            assert_eq!(Tier::parse(tier.id()), Some(tier));
        }
    }

    #[test]
    fn an_unknown_identifier_is_refused_rather_than_defaulted() {
        assert_eq!(Tier::parse("lukewarm"), None);
        assert_eq!(Tier::parse(""), None);
    }

    #[test]
    fn the_intervals_are_those_of_the_freshness_table() {
        assert_eq!(Tier::Hot.refresh_interval(), Duration::from_secs(10));
        assert_eq!(Tier::Warm.refresh_interval(), Duration::from_secs(60));
        assert_eq!(Tier::Cold.refresh_interval(), Duration::from_secs(21_600));
    }

    #[test]
    fn an_advised_interval_longer_than_ours_wins_because_the_forge_asked_for_it() {
        assert_eq!(
            Tier::Warm.interval_respecting(Some(Duration::from_secs(300))),
            Duration::from_secs(300)
        );
    }

    #[test]
    fn an_advised_interval_shorter_than_ours_never_makes_us_poll_faster() {
        assert_eq!(
            Tier::Warm.interval_respecting(Some(Duration::from_secs(5))),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn no_advice_leaves_the_tier_interval_untouched() {
        assert_eq!(Tier::Hot.interval_respecting(None), Duration::from_secs(10));
    }
}
