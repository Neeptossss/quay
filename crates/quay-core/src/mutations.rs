#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationKind {
    ApprovePullRequest,
    RequestChanges,
    PostComment,
    ResolveThread,
    UnresolveThread,
    MergePullRequest,
}

impl MutationKind {
    pub const ALL: [MutationKind; 6] = [
        MutationKind::ApprovePullRequest,
        MutationKind::RequestChanges,
        MutationKind::PostComment,
        MutationKind::ResolveThread,
        MutationKind::UnresolveThread,
        MutationKind::MergePullRequest,
    ];

    pub fn id(self) -> &'static str {
        match self {
            MutationKind::ApprovePullRequest => "approve_pr",
            MutationKind::RequestChanges => "request_changes",
            MutationKind::PostComment => "post_comment",
            MutationKind::ResolveThread => "resolve_thread",
            MutationKind::UnresolveThread => "unresolve_thread",
            MutationKind::MergePullRequest => "merge_pr",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        MutationKind::ALL.into_iter().find(|kind| kind.id() == id)
    }

    pub fn is_public(self) -> bool {
        self != MutationKind::ResolveThread && self != MutationKind::UnresolveThread
    }

    pub fn is_safe_to_resend(self) -> bool {
        match self {
            MutationKind::ResolveThread
            | MutationKind::UnresolveThread
            | MutationKind::MergePullRequest => true,
            MutationKind::ApprovePullRequest
            | MutationKind::RequestChanges
            | MutationKind::PostComment => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationState {
    Pending,
    InFlight,
    Failed,
    Done,
}

impl MutationState {
    pub fn id(self) -> &'static str {
        match self {
            MutationState::Pending => "pending",
            MutationState::InFlight => "inflight",
            MutationState::Failed => "failed",
            MutationState::Done => "done",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        match id {
            "pending" => Some(MutationState::Pending),
            "inflight" => Some(MutationState::InFlight),
            "failed" => Some(MutationState::Failed),
            "done" => Some(MutationState::Done),
            _ => None,
        }
    }

    pub fn is_settled(self) -> bool {
        matches!(self, MutationState::Done | MutationState::Failed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mutation {
    pub id: i64,
    pub kind: MutationKind,
    pub target: String,
    pub payload: String,
    pub idempotency: String,
    pub state: MutationState,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub created_at: i64,
}

#[cfg(test)]
mod tests {
    use super::{MutationKind, MutationState};

    #[test]
    fn every_mutation_kind_survives_a_round_trip_through_its_stored_identifier() {
        for kind in MutationKind::ALL {
            assert_eq!(MutationKind::parse(kind.id()), Some(kind));
        }
    }

    #[test]
    fn every_mutation_state_survives_a_round_trip_through_its_stored_identifier() {
        for state in [
            MutationState::Pending,
            MutationState::InFlight,
            MutationState::Failed,
            MutationState::Done,
        ] {
            assert_eq!(MutationState::parse(state.id()), Some(state));
        }
    }

    #[test]
    fn an_unknown_identifier_is_refused_rather_than_defaulted() {
        assert_eq!(MutationKind::parse("delete_repository"), None);
        assert_eq!(MutationState::parse("maybe"), None);
    }

    #[test]
    fn resolving_a_thread_is_the_only_kind_that_is_not_publicly_visible() {
        assert!(!MutationKind::ResolveThread.is_public());
        assert!(!MutationKind::UnresolveThread.is_public());
        assert!(MutationKind::ApprovePullRequest.is_public());
        assert!(MutationKind::PostComment.is_public());
        assert!(MutationKind::MergePullRequest.is_public());
    }

    #[test]
    fn a_mutation_that_creates_content_is_never_resent_after_an_interrupted_flight() {
        assert!(!MutationKind::PostComment.is_safe_to_resend());
        assert!(!MutationKind::ApprovePullRequest.is_safe_to_resend());
        assert!(!MutationKind::RequestChanges.is_safe_to_resend());
    }

    #[test]
    fn a_mutation_that_only_sets_a_state_may_be_resent() {
        assert!(MutationKind::ResolveThread.is_safe_to_resend());
        assert!(MutationKind::UnresolveThread.is_safe_to_resend());
        assert!(MutationKind::MergePullRequest.is_safe_to_resend());
    }

    #[test]
    fn only_a_finished_mutation_is_settled() {
        assert!(MutationState::Done.is_settled());
        assert!(MutationState::Failed.is_settled());
        assert!(!MutationState::Pending.is_settled());
        assert!(!MutationState::InFlight.is_settled());
    }
}
