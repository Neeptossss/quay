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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestSnapshot {
    pub repository: Repository,
    pub pull_request: PullRequest,
    pub threads: Vec<ReviewThread>,
    pub review_requests: Vec<ReviewRequest>,
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
    use super::{PullRequestState, Repository};

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
