pub const INBOX_HEAD: usize = 5;
pub const UTILISATION_WINDOW: usize = 500;
pub const UTILISATION_FLOOR: f64 = 0.40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreloadReason {
    InboxHead,
    NeighbourOfOpenEntry,
    Navigation,
}

impl PreloadReason {
    pub fn id(self) -> &'static str {
        match self {
            PreloadReason::InboxHead => "inbox_head",
            PreloadReason::NeighbourOfOpenEntry => "neighbour_of_open_entry",
            PreloadReason::Navigation => "navigation",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        match id {
            "inbox_head" => Some(PreloadReason::InboxHead),
            "neighbour_of_open_entry" => Some(PreloadReason::NeighbourOfOpenEntry),
            "navigation" => Some(PreloadReason::Navigation),
            _ => None,
        }
    }

    pub fn is_speculative(self) -> bool {
        self != PreloadReason::Navigation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreloadRequest {
    pub key: String,
    pub reason: PreloadReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationState {
    pub list: Vec<String>,
    pub open_index: Option<usize>,
}

impl NavigationState {
    pub fn inbox(list: Vec<String>) -> Self {
        Self {
            list,
            open_index: None,
        }
    }

    pub fn opened(list: Vec<String>, index: usize) -> Self {
        Self {
            list,
            open_index: Some(index),
        }
    }
}

pub fn plan(state: &NavigationState) -> Vec<PreloadRequest> {
    let mut planned: Vec<PreloadRequest> = Vec::new();
    let mut push = |key: &String, reason: PreloadReason| {
        if !planned.iter().any(|request| &request.key == key) {
            planned.push(PreloadRequest {
                key: key.clone(),
                reason,
            });
        }
    };

    match state.open_index {
        None => {
            for key in state.list.iter().take(INBOX_HEAD) {
                push(key, PreloadReason::InboxHead);
            }
        }
        Some(index) => {
            if let Some(previous) = index.checked_sub(1).and_then(|rank| state.list.get(rank)) {
                push(previous, PreloadReason::NeighbourOfOpenEntry);
            }
            if let Some(next) = state.list.get(index + 1) {
                push(next, PreloadReason::NeighbourOfOpenEntry);
            }
        }
    }
    planned
}

pub fn speculation_stays_enabled(used: usize, observed: usize) -> bool {
    if observed < UTILISATION_WINDOW {
        return true;
    }
    used as f64 / observed as f64 >= UTILISATION_FLOOR
}

#[cfg(test)]
mod tests {
    use super::{
        NavigationState, PreloadReason, UTILISATION_WINDOW, plan, speculation_stays_enabled,
    };

    fn keys(count: usize) -> Vec<String> {
        (0..count).map(|rank| format!("acme/api#{rank}")).collect()
    }

    #[test]
    fn an_empty_inbox_asks_for_no_preload() {
        assert!(plan(&NavigationState::inbox(Vec::new())).is_empty());
    }

    #[test]
    fn a_loaded_inbox_preloads_its_first_five_entries() {
        let planned = plan(&NavigationState::inbox(keys(20)));
        assert_eq!(planned.len(), 5);
        assert_eq!(planned[0].key, "acme/api#0");
        assert_eq!(planned[4].key, "acme/api#4");
        assert!(
            planned
                .iter()
                .all(|request| request.reason == PreloadReason::InboxHead)
        );
    }

    #[test]
    fn an_inbox_shorter_than_five_entries_preloads_what_it_has() {
        assert_eq!(plan(&NavigationState::inbox(keys(2))).len(), 2);
    }

    #[test]
    fn an_open_entry_preloads_the_one_before_and_the_one_after() {
        let planned = plan(&NavigationState::opened(keys(10), 4));
        let requested: Vec<&str> = planned.iter().map(|request| request.key.as_str()).collect();
        assert_eq!(requested, vec!["acme/api#3", "acme/api#5"]);
    }

    #[test]
    fn the_first_open_entry_has_no_previous_neighbour_to_preload() {
        let planned = plan(&NavigationState::opened(keys(10), 0));
        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].key, "acme/api#1");
    }

    #[test]
    fn the_last_open_entry_has_no_next_neighbour_to_preload() {
        let planned = plan(&NavigationState::opened(keys(3), 2));
        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].key, "acme/api#1");
    }

    #[test]
    fn a_single_entry_list_asks_for_nothing_when_that_entry_is_open() {
        assert!(plan(&NavigationState::opened(keys(1), 0)).is_empty());
    }

    #[test]
    fn an_open_index_beyond_the_list_asks_for_nothing_rather_than_panicking() {
        assert!(plan(&NavigationState::opened(keys(3), 99)).is_empty());
    }

    #[test]
    fn the_same_entry_is_never_preloaded_twice_in_one_plan() {
        let planned = plan(&NavigationState::inbox(vec![
            "acme/api#1".to_owned(),
            "acme/api#1".to_owned(),
        ]));
        assert_eq!(planned.len(), 1);
    }

    #[test]
    fn speculation_stays_on_until_the_window_is_full() {
        assert!(speculation_stays_enabled(0, UTILISATION_WINDOW - 1));
    }

    #[test]
    fn speculation_switches_itself_off_below_forty_percent_of_use() {
        assert!(!speculation_stays_enabled(199, UTILISATION_WINDOW));
        assert!(speculation_stays_enabled(200, UTILISATION_WINDOW));
    }

    #[test]
    fn a_navigation_is_not_a_speculative_preload() {
        assert!(!PreloadReason::Navigation.is_speculative());
        assert!(PreloadReason::InboxHead.is_speculative());
        assert!(PreloadReason::NeighbourOfOpenEntry.is_speculative());
    }

    #[test]
    fn every_reason_survives_a_round_trip_through_its_stored_identifier() {
        for reason in [
            PreloadReason::InboxHead,
            PreloadReason::NeighbourOfOpenEntry,
            PreloadReason::Navigation,
        ] {
            assert_eq!(PreloadReason::parse(reason.id()), Some(reason));
        }
        assert_eq!(PreloadReason::parse("guesswork"), None);
    }
}
