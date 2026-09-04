use std::collections::BTreeSet;

const IMPLICATIONS: [(&str, &[&str]); 5] = [
    (
        "repo",
        &[
            "repo:status",
            "repo_deployment",
            "public_repo",
            "repo:invite",
            "security_events",
        ],
    ),
    ("admin:org", &["write:org", "read:org"]),
    ("write:org", &["read:org"]),
    ("user", &["read:user", "user:email", "user:follow"]),
    ("admin:repo_hook", &["write:repo_hook", "read:repo_hook"]),
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrantedScopes {
    granted: BTreeSet<String>,
}

impl GrantedScopes {
    pub fn parse(header: &str) -> Self {
        let mut granted: BTreeSet<String> = header
            .split(',')
            .map(str::trim)
            .filter(|scope| !scope.is_empty())
            .map(str::to_owned)
            .collect();

        let mut expanded = true;
        while expanded {
            expanded = false;
            for (parent, children) in IMPLICATIONS {
                if granted.contains(parent) {
                    for child in children {
                        if granted.insert((*child).to_owned()) {
                            expanded = true;
                        }
                    }
                }
            }
        }
        Self { granted }
    }

    pub fn contains(&self, scope: &str) -> bool {
        self.granted.contains(scope)
    }

    pub fn is_empty(&self) -> bool {
        self.granted.is_empty()
    }

    pub fn listed(&self) -> Vec<&str> {
        self.granted.iter().map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::GrantedScopes;

    #[test]
    fn an_absent_scope_header_grants_nothing() {
        assert!(GrantedScopes::parse("").is_empty());
        assert!(!GrantedScopes::parse("").contains("repo"));
    }

    #[test]
    fn a_scope_header_is_split_on_commas_and_trimmed() {
        let scopes = GrantedScopes::parse(" repo , notifications,read:org ");
        assert!(scopes.contains("repo"));
        assert!(scopes.contains("notifications"));
        assert!(scopes.contains("read:org"));
        assert!(!scopes.contains("workflow"));
    }

    #[test]
    fn an_organization_admin_scope_grants_the_read_scope_it_implies() {
        let scopes = GrantedScopes::parse("admin:org");
        assert!(scopes.contains("write:org"));
        assert!(scopes.contains("read:org"));
    }

    #[test]
    fn a_write_organization_scope_grants_the_read_scope_it_implies() {
        assert!(GrantedScopes::parse("write:org").contains("read:org"));
    }

    #[test]
    fn the_repository_scope_grants_the_narrower_repository_scopes() {
        let scopes = GrantedScopes::parse("repo");
        assert!(scopes.contains("public_repo"));
        assert!(scopes.contains("repo:status"));
        assert!(!scopes.contains("workflow"));
    }

    #[test]
    fn the_user_scope_grants_the_narrower_user_scopes() {
        assert!(GrantedScopes::parse("user").contains("read:user"));
    }

    #[test]
    fn implication_never_invents_an_unrelated_scope() {
        let scopes = GrantedScopes::parse("repo");
        assert!(!scopes.contains("notifications"));
        assert!(!scopes.contains("read:org"));
        assert!(!scopes.contains("admin:org"));
    }
}
