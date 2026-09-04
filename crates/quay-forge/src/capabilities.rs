use crate::auth::{Identity, SingleSignOn, TokenKind};
use crate::scopes::GrantedScopes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Notifications,
    ReviewSubmit,
    ThreadResolve,
    Merge,
    ActionsRerun,
    OrgTeams,
}

impl Capability {
    pub const ALL: [Capability; 6] = [
        Capability::Notifications,
        Capability::ReviewSubmit,
        Capability::ThreadResolve,
        Capability::Merge,
        Capability::ActionsRerun,
        Capability::OrgTeams,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Capability::Notifications => "notifications",
            Capability::ReviewSubmit => "review_submit",
            Capability::ThreadResolve => "thread_resolve",
            Capability::Merge => "merge",
            Capability::ActionsRerun => "actions_rerun",
            Capability::OrgTeams => "org_teams",
        }
    }

    pub fn required_scope(self) -> &'static str {
        match self {
            Capability::Notifications => "notifications",
            Capability::ReviewSubmit | Capability::ThreadResolve | Capability::Merge => "repo",
            Capability::ActionsRerun => "workflow",
            Capability::OrgTeams => "read:org",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    ScopeMissing(&'static str),
    AuthModeUnsupported,
    OrgPolicy(String),
    SsoUnauthorized(String),
}

impl Reason {
    pub fn is_recoverable_by_the_user(&self) -> bool {
        matches!(self, Reason::ScopeMissing(_) | Reason::SsoUnauthorized(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Support {
    Available,
    Unavailable { reason: Reason },
    Unknown,
}

impl Support {
    pub fn is_discoverable(&self) -> bool {
        match self {
            Support::Available | Support::Unknown => true,
            Support::Unavailable { reason } => reason.is_recoverable_by_the_user(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        matches!(self, Support::Available | Support::Unknown)
    }
}

#[derive(Debug, Clone)]
pub struct Capabilities {
    token_kind: TokenKind,
    support: [(Capability, Support); 6],
}

impl Capabilities {
    pub fn from_identity(identity: &Identity) -> Self {
        let mut capabilities = Self::from_scopes(identity.token_kind, &identity.scopes);
        if let SingleSignOn::AuthorizationRequired { url } = &identity.single_sign_on {
            for capability in [
                Capability::ReviewSubmit,
                Capability::ThreadResolve,
                Capability::Merge,
                Capability::OrgTeams,
            ] {
                capabilities.set(
                    capability,
                    Support::Unavailable {
                        reason: Reason::SsoUnauthorized(url.clone()),
                    },
                );
            }
        }
        capabilities
    }

    pub fn from_scopes(token_kind: TokenKind, scopes: &GrantedScopes) -> Self {
        let evaluate = |capability: Capability| {
            if scopes.is_empty() {
                Support::Unknown
            } else if scopes.contains(capability.required_scope()) {
                Support::Available
            } else {
                Support::Unavailable {
                    reason: Reason::ScopeMissing(capability.required_scope()),
                }
            }
        };
        Self {
            token_kind,
            support: Capability::ALL.map(|capability| (capability, evaluate(capability))),
        }
    }

    pub fn token_kind(&self) -> TokenKind {
        self.token_kind
    }

    pub fn get(&self, capability: Capability) -> &Support {
        self.support
            .iter()
            .find(|(declared, _)| *declared == capability)
            .map(|(_, support)| support)
            .unwrap_or(&Support::Unknown)
    }

    pub fn set(&mut self, capability: Capability, support: Support) {
        if let Some(entry) = self
            .support
            .iter_mut()
            .find(|(declared, _)| *declared == capability)
        {
            entry.1 = support;
        }
    }

    pub fn observe_probe(&mut self, capability: Capability, status: u16, message: &str) {
        self.set(capability, support_from_status(capability, status, message));
    }

    pub fn observe_failure(&mut self, capability: Capability, status: u16, message: &str) {
        if matches!(status, 403 | 404) {
            self.set(capability, support_from_status(capability, status, message));
        }
    }

    pub fn discoverable(&self) -> Vec<Capability> {
        self.support
            .iter()
            .filter(|(_, support)| support.is_discoverable())
            .map(|(capability, _)| *capability)
            .collect()
    }
}

fn support_from_status(capability: Capability, status: u16, message: &str) -> Support {
    match status {
        200 | 201 | 204 | 304 => Support::Available,
        403 | 404 => Support::Unavailable {
            reason: reason_from_message(capability, message),
        },
        _ => Support::Unknown,
    }
}

fn reason_from_message(capability: Capability, message: &str) -> Reason {
    let lowered = message.to_lowercase();
    if lowered.contains("saml") || lowered.contains("sso") || lowered.contains("single sign") {
        Reason::SsoUnauthorized(message.to_owned())
    } else if lowered.contains("policy")
        || lowered.contains("not allowed")
        || lowered.contains("organization")
    {
        Reason::OrgPolicy(message.to_owned())
    } else if lowered.contains("scope") {
        Reason::ScopeMissing(capability.required_scope())
    } else {
        Reason::AuthModeUnsupported
    }
}

#[cfg(test)]
mod tests {
    use super::{Capabilities, Capability, Reason, Support};
    use crate::auth::TokenKind;
    use crate::scopes::GrantedScopes;

    fn with(scopes: &str) -> Capabilities {
        Capabilities::from_scopes(TokenKind::PatClassic, &GrantedScopes::parse(scopes))
    }

    #[test]
    fn a_capability_whose_scope_is_missing_is_unavailable_and_names_the_scope() {
        let capabilities = with("repo");
        assert_eq!(
            capabilities.get(Capability::ActionsRerun),
            &Support::Unavailable {
                reason: Reason::ScopeMissing("workflow")
            }
        );
    }

    #[test]
    fn a_capability_whose_scope_is_granted_is_available() {
        let capabilities = with("repo, notifications, read:org, workflow");
        for capability in Capability::ALL {
            assert_eq!(
                capabilities.get(capability),
                &Support::Available,
                "{}",
                capability.id()
            );
        }
    }

    #[test]
    fn a_token_without_a_scope_header_leaves_every_capability_unknown() {
        let capabilities = with("");
        for capability in Capability::ALL {
            assert_eq!(capabilities.get(capability), &Support::Unknown);
        }
    }

    #[test]
    fn an_unknown_capability_stays_discoverable_because_we_never_block_by_precaution() {
        assert!(Support::Unknown.is_discoverable());
        assert!(Support::Unknown.is_enabled());
    }

    #[test]
    fn a_missing_scope_stays_discoverable_because_the_user_can_fix_it() {
        let capabilities = with("repo");
        assert!(capabilities.get(Capability::ActionsRerun).is_discoverable());
        assert!(!capabilities.get(Capability::ActionsRerun).is_enabled());
        assert!(
            capabilities
                .discoverable()
                .contains(&Capability::ActionsRerun)
        );
    }

    #[test]
    fn an_organization_policy_removes_the_capability_from_the_command_registry() {
        let mut capabilities = with("repo, notifications, read:org, workflow");
        capabilities.observe_failure(
            Capability::Merge,
            403,
            "Organization has enabled a policy that blocks this",
        );
        assert!(!capabilities.get(Capability::Merge).is_discoverable());
        assert!(!capabilities.discoverable().contains(&Capability::Merge));
    }

    #[test]
    fn a_single_sign_on_failure_stays_discoverable_with_its_corrective_action() {
        let mut capabilities = with("repo, notifications, read:org, workflow");
        capabilities.observe_failure(
            Capability::ThreadResolve,
            403,
            "Resource protected by organization SAML enforcement",
        );
        match capabilities.get(Capability::ThreadResolve) {
            Support::Unavailable { reason } => {
                assert!(reason.is_recoverable_by_the_user());
                assert!(matches!(reason, Reason::SsoUnauthorized(_)));
            }
            other => panic!("expected an unavailable capability, got {other:?}"),
        }
    }

    #[test]
    fn a_probe_that_succeeds_marks_the_capability_available() {
        let mut capabilities = with("");
        capabilities.observe_probe(Capability::Notifications, 200, "");
        assert_eq!(
            capabilities.get(Capability::Notifications),
            &Support::Available
        );
    }

    #[test]
    fn a_probe_answering_not_modified_still_marks_the_capability_available() {
        let mut capabilities = with("");
        capabilities.observe_probe(Capability::Notifications, 304, "");
        assert_eq!(
            capabilities.get(Capability::Notifications),
            &Support::Available
        );
    }

    #[test]
    fn a_status_that_is_neither_success_nor_refusal_never_disables_a_capability() {
        let mut capabilities = with("repo, notifications, read:org, workflow");
        capabilities.observe_failure(Capability::Merge, 500, "Internal error");
        assert_eq!(capabilities.get(Capability::Merge), &Support::Available);
    }

    #[test]
    fn learning_from_failure_only_reacts_to_a_refusal() {
        let mut capabilities = with("repo, notifications, read:org, workflow");
        capabilities.observe_failure(Capability::Merge, 422, "Unprocessable");
        assert_eq!(capabilities.get(Capability::Merge), &Support::Available);
        capabilities.observe_failure(Capability::Merge, 404, "Not Found");
        assert!(!capabilities.get(Capability::Merge).is_enabled());
    }
}
