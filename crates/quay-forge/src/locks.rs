use std::collections::BTreeSet;

use crate::error::ForgeError;
use crate::governor::{OutboundRequest, RequestKind};

pub struct DevLocks {
    readonly: bool,
    write_allowlist: Option<BTreeSet<String>>,
}

impl DevLocks {
    pub fn new(readonly: bool, write_allowlist: Option<BTreeSet<String>>) -> Self {
        Self {
            readonly,
            write_allowlist,
        }
    }

    pub fn from_env() -> Self {
        Self::from_settings(
            std::env::var("QUAY_READONLY").ok().as_deref(),
            std::env::var("QUAY_WRITE_ALLOWLIST").ok().as_deref(),
        )
    }

    pub fn from_settings(readonly: Option<&str>, write_allowlist: Option<&str>) -> Self {
        let readonly = readonly.map(|value| value.trim() != "0").unwrap_or(true);
        let write_allowlist = write_allowlist.map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(str::to_owned)
                .collect()
        });
        Self::new(readonly, write_allowlist)
    }

    pub fn is_readonly(&self) -> bool {
        self.readonly
    }

    pub fn authorize(&self, request: &OutboundRequest) -> Result<(), ForgeError> {
        self.check_method_matches_kind(request)?;
        self.check_no_mutation_hides_in_a_query(request)?;
        if !request.kind.is_write() {
            return Ok(());
        }
        if self.readonly {
            return Err(ForgeError::ReadOnlyModeRejected {
                kind: request.kind.label(),
                url: request.url.clone(),
            });
        }
        self.check_write_allowlist(request)
    }

    fn check_method_matches_kind(&self, request: &OutboundRequest) -> Result<(), ForgeError> {
        let method = request.method.as_str();
        let consistent = match request.kind {
            RequestKind::RestRead => matches!(method, "GET" | "HEAD"),
            RequestKind::RestWrite => !matches!(method, "GET" | "HEAD"),
            RequestKind::GraphQlQuery | RequestKind::GraphQlMutation => method == "POST",
        };
        if consistent {
            Ok(())
        } else {
            Err(ForgeError::MethodMismatch {
                kind: request.kind.label(),
                method: method.to_owned(),
            })
        }
    }

    fn check_no_mutation_hides_in_a_query(
        &self,
        request: &OutboundRequest,
    ) -> Result<(), ForgeError> {
        if request.kind != RequestKind::GraphQlQuery {
            return Ok(());
        }
        let declares_mutation = request
            .body
            .as_deref()
            .is_some_and(declares_mutation_at_top_level);
        if declares_mutation {
            Err(ForgeError::MutationDeclaredAsQuery)
        } else {
            Ok(())
        }
    }

    fn check_write_allowlist(&self, request: &OutboundRequest) -> Result<(), ForgeError> {
        let Some(allowlist) = self.write_allowlist.as_ref() else {
            return Ok(());
        };
        let Some(repo) = request.repo.as_ref() else {
            return Err(ForgeError::WriteTargetMissing);
        };
        if allowlist.contains(repo) {
            Ok(())
        } else {
            Err(ForgeError::WriteAllowlistRejected { repo: repo.clone() })
        }
    }
}

fn declares_mutation_at_top_level(document: &str) -> bool {
    let mut depth = 0i32;
    let mut word = String::new();
    for character in document.chars() {
        match character {
            '{' | '(' | '[' => {
                if depth == 0 && word == "mutation" {
                    return true;
                }
                depth += 1;
                word.clear();
            }
            '}' | ')' | ']' => {
                depth -= 1;
                word.clear();
            }
            character if character.is_alphanumeric() || character == '_' => {
                if depth == 0 {
                    word.push(character);
                }
            }
            _ => {
                if depth == 0 && word == "mutation" {
                    return true;
                }
                word.clear();
            }
        }
    }
    word == "mutation"
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use quay_core::Priority;

    use super::DevLocks;
    use crate::error::ForgeError;
    use crate::governor::OutboundRequest;

    fn allowlist(entries: &[&str]) -> BTreeSet<String> {
        entries.iter().map(|entry| (*entry).to_owned()).collect()
    }

    fn read() -> OutboundRequest {
        OutboundRequest::rest_read("https://api.github.com/user", Priority::User)
    }

    fn write() -> OutboundRequest {
        OutboundRequest::rest_write(
            "https://api.github.com/repos/acme/api/pulls/1/reviews",
            "{}",
            "acme/api",
            Priority::User,
        )
    }

    #[test]
    fn read_only_mode_is_the_default_when_the_variable_is_absent() {
        assert!(DevLocks::from_settings(None, None).is_readonly());
    }

    #[test]
    fn read_only_mode_is_only_lifted_by_an_explicit_zero() {
        assert!(DevLocks::from_settings(Some("1"), None).is_readonly());
        assert!(DevLocks::from_settings(Some(""), None).is_readonly());
        assert!(DevLocks::from_settings(Some("false"), None).is_readonly());
        assert!(!DevLocks::from_settings(Some("0"), None).is_readonly());
    }

    #[test]
    fn an_empty_allowlist_variable_blocks_every_write() {
        let locks = DevLocks::from_settings(Some("0"), Some(""));
        assert!(matches!(
            locks.authorize(&write()),
            Err(ForgeError::WriteAllowlistRejected { .. })
        ));
    }

    #[test]
    fn read_only_mode_rejects_a_rest_write() {
        let locks = DevLocks::new(true, None);
        assert!(matches!(
            locks.authorize(&write()),
            Err(ForgeError::ReadOnlyModeRejected { .. })
        ));
    }

    #[test]
    fn read_only_mode_rejects_a_graphql_mutation() {
        let locks = DevLocks::new(true, None);
        let request = OutboundRequest::graphql_mutation(
            "https://api.github.com/graphql",
            "mutation { resolveReviewThread(input: {}) { clientMutationId } }",
            "acme/api",
            Priority::User,
        );
        assert!(matches!(
            locks.authorize(&request),
            Err(ForgeError::ReadOnlyModeRejected { .. })
        ));
    }

    #[test]
    fn read_only_mode_still_allows_a_graphql_query_over_post() {
        let locks = DevLocks::new(true, None);
        let request = OutboundRequest::graphql_query(
            "https://api.github.com/graphql",
            "query PrDetail { repository(owner: \"acme\", name: \"api\") { id } }",
            Priority::Hot,
        );
        assert!(locks.authorize(&request).is_ok());
    }

    #[test]
    fn a_mutation_disguised_as_a_query_is_rejected_even_outside_read_only_mode() {
        let locks = DevLocks::new(false, None);
        let request = OutboundRequest::graphql_query(
            "https://api.github.com/graphql",
            "mutation { addComment(input: {}) { clientMutationId } }",
            Priority::User,
        );
        assert!(matches!(
            locks.authorize(&request),
            Err(ForgeError::MutationDeclaredAsQuery)
        ));
    }

    #[test]
    fn a_query_naming_a_field_called_mutation_is_not_mistaken_for_a_mutation() {
        let locks = DevLocks::new(false, None);
        let request = OutboundRequest::graphql_query(
            "https://api.github.com/graphql",
            "query { repository { mutation } }",
            Priority::Hot,
        );
        assert!(locks.authorize(&request).is_ok());
    }

    #[test]
    fn a_write_disguised_as_a_read_is_rejected_on_the_method() {
        let locks = DevLocks::new(false, None);
        let request = OutboundRequest {
            kind: crate::governor::RequestKind::RestRead,
            method: reqwest::Method::POST,
            url: "https://api.github.com/repos/acme/api/issues".to_owned(),
            body: None,
            repo: None,
            priority: Priority::User,
        };
        assert!(matches!(
            locks.authorize(&request),
            Err(ForgeError::MethodMismatch { .. })
        ));
    }

    #[test]
    fn the_write_allowlist_rejects_a_repository_it_does_not_name() {
        let locks = DevLocks::new(false, Some(allowlist(&["neeptossss/scratch"])));
        assert!(matches!(
            locks.authorize(&write()),
            Err(ForgeError::WriteAllowlistRejected { .. })
        ));
    }

    #[test]
    fn the_write_allowlist_rejects_a_write_that_names_no_repository() {
        let locks = DevLocks::new(false, Some(allowlist(&["neeptossss/scratch"])));
        let request = OutboundRequest {
            repo: None,
            ..write()
        };
        assert!(matches!(
            locks.authorize(&request),
            Err(ForgeError::WriteTargetMissing)
        ));
    }

    #[test]
    fn the_write_allowlist_accepts_a_repository_it_names() {
        let locks = DevLocks::new(false, Some(allowlist(&["acme/api"])));
        assert!(locks.authorize(&write()).is_ok());
    }

    #[test]
    fn a_read_is_never_blocked_by_either_lock() {
        let locks = DevLocks::new(true, Some(allowlist(&[])));
        assert!(locks.authorize(&read()).is_ok());
    }
}
