#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestState {
    Open,
    Closed,
    Merged,
}

impl PullRequestState {
    pub fn id(self) -> &'static str {
        match self {
            PullRequestState::Open => "open",
            PullRequestState::Closed => "closed",
            PullRequestState::Merged => "merged",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "open" => Some(PullRequestState::Open),
            "closed" => Some(PullRequestState::Closed),
            "merged" => Some(PullRequestState::Merged),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    pub node_id: String,
    pub owner: String,
    pub name: String,
    pub default_branch: Option<String>,
}

impl Repository {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    pub node_id: String,
    pub number: i64,
    pub title: String,
    pub state: PullRequestState,
    pub is_draft: bool,
    pub author: String,
    pub base_ref: String,
    pub head_sha: String,
    pub review_state: Option<String>,
    pub checks_state: Option<String>,
    pub mergeable: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewComment {
    pub node_id: String,
    pub author: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewThread {
    pub node_id: String,
    pub path: String,
    pub line: Option<i64>,
    pub side: Option<String>,
    pub original_line: Option<i64>,
    pub diff_hunk: Option<String>,
    pub is_resolved: bool,
    pub is_outdated: bool,
    pub comments: Vec<ReviewComment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRequest {
    pub reviewer: String,
    pub is_team: bool,
    pub requested_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineKind {
    Commit,
    Comment,
    Review,
    ReviewRequested,
    ReadyForReview,
    ForcePush,
    Merged,
    Closed,
    Reopened,
}

impl TimelineKind {
    pub fn id(self) -> &'static str {
        match self {
            TimelineKind::Commit => "commit",
            TimelineKind::Comment => "comment",
            TimelineKind::Review => "review",
            TimelineKind::ReviewRequested => "review_requested",
            TimelineKind::ReadyForReview => "ready_for_review",
            TimelineKind::ForcePush => "force_push",
            TimelineKind::Merged => "merged",
            TimelineKind::Closed => "closed",
            TimelineKind::Reopened => "reopened",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.id() == value)
    }

    pub const ALL: [TimelineKind; 9] = [
        TimelineKind::Commit,
        TimelineKind::Comment,
        TimelineKind::Review,
        TimelineKind::ReviewRequested,
        TimelineKind::ReadyForReview,
        TimelineKind::ForcePush,
        TimelineKind::Merged,
        TimelineKind::Closed,
        TimelineKind::Reopened,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEvent {
    pub node_id: String,
    pub kind: TimelineKind,
    pub actor: String,
    pub body: Option<String>,
    pub reference: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestSnapshot {
    pub repository: Repository,
    pub pull_request: PullRequest,
    pub threads: Vec<ReviewThread>,
    pub review_requests: Vec<ReviewRequest>,
    pub events: Vec<TimelineEvent>,
    pub raw: Vec<u8>,
}

impl PullRequestSnapshot {
    pub fn unresolved_threads(&self) -> usize {
        self.threads
            .iter()
            .filter(|thread| !thread.is_resolved)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::{PullRequestState, Repository, TimelineKind};

    #[test]
    fn every_pull_request_state_survives_a_round_trip_through_its_stored_identifier() {
        for state in [
            PullRequestState::Open,
            PullRequestState::Closed,
            PullRequestState::Merged,
        ] {
            assert_eq!(PullRequestState::parse(state.id()), Some(state));
        }
    }

    #[test]
    fn a_state_the_forge_spells_in_capitals_is_still_understood() {
        assert_eq!(
            PullRequestState::parse("MERGED"),
            Some(PullRequestState::Merged)
        );
    }

    #[test]
    fn an_unknown_state_is_refused_rather_than_defaulted_to_open() {
        assert_eq!(PullRequestState::parse("draft"), None);
        assert_eq!(PullRequestState::parse(""), None);
    }

    #[test]
    fn every_timeline_kind_survives_a_round_trip_through_its_stored_identifier() {
        for kind in TimelineKind::ALL {
            assert_eq!(TimelineKind::parse(kind.id()), Some(kind));
        }
    }

    #[test]
    fn a_timeline_kind_the_forge_invents_later_is_refused_rather_than_folded_into_a_comment() {
        assert_eq!(TimelineKind::parse("labeled"), None);
        assert_eq!(TimelineKind::parse(""), None);
    }

    #[test]
    fn no_two_timeline_kinds_share_a_stored_identifier() {
        let mut identifiers: Vec<&str> = TimelineKind::ALL.iter().map(|kind| kind.id()).collect();
        identifiers.sort_unstable();
        let count = identifiers.len();
        identifiers.dedup();
        assert_eq!(identifiers.len(), count);
    }

    #[test]
    fn a_repository_renders_the_full_name_the_forge_uses() {
        let repository = Repository {
            node_id: "R_1".to_owned(),
            owner: "acme".to_owned(),
            name: "api".to_owned(),
            default_branch: None,
        };
        assert_eq!(repository.full_name(), "acme/api");
    }
}
