use serde::Deserialize;

use crate::error::ForgeError;
use crate::governor::ForgeResponse;
use crate::scopes::GrantedScopes;
use crate::transport::{ErrorPayload, OrganizationPayload, UserPayload};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    PatClassic,
    PatFineGrained,
    OAuthApp,
    GitHubApp,
    Unrecognised,
}

impl TokenKind {
    pub fn of(token: &str) -> Self {
        if token.starts_with("github_pat_") {
            TokenKind::PatFineGrained
        } else if token.starts_with("ghp_") {
            TokenKind::PatClassic
        } else if token.starts_with("gho_") {
            TokenKind::OAuthApp
        } else if token.starts_with("ghu_") || token.starts_with("ghs_") {
            TokenKind::GitHubApp
        } else {
            TokenKind::Unrecognised
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TokenKind::PatClassic => "classic personal access token",
            TokenKind::PatFineGrained => "fine-grained personal access token",
            TokenKind::OAuthApp => "oauth app token",
            TokenKind::GitHubApp => "github app token",
            TokenKind::Unrecognised => "unrecognised token",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SingleSignOn {
    NoRestriction,
    PartialResults { organization_ids: Vec<u64> },
    AuthorizationRequired { url: String },
}

impl SingleSignOn {
    pub fn parse(header: Option<&str>) -> Self {
        let Some(header) = header else {
            return SingleSignOn::NoRestriction;
        };
        if header.contains("partial-results") {
            return SingleSignOn::PartialResults {
                organization_ids: directive_value(header, "organizations")
                    .unwrap_or_default()
                    .split(',')
                    .filter_map(|identifier| identifier.trim().parse::<u64>().ok())
                    .collect(),
            };
        }
        if header.contains("required") {
            return SingleSignOn::AuthorizationRequired {
                url: directive_value(header, "url").unwrap_or_default(),
            };
        }
        SingleSignOn::NoRestriction
    }

    pub fn hides_organizations(&self) -> bool {
        !matches!(self, SingleSignOn::NoRestriction)
    }
}

fn directive_value(header: &str, key: &str) -> Option<String> {
    header.split(';').find_map(|part| {
        let (name, value) = part.split_once('=')?;
        if name.trim() == key {
            Some(value.trim().to_owned())
        } else {
            None
        }
    })
}

#[derive(Debug, Clone)]
pub struct Identity {
    pub login: String,
    pub node_id: String,
    pub organizations: Vec<String>,
    pub scopes: GrantedScopes,
    pub token_kind: TokenKind,
    pub single_sign_on: SingleSignOn,
}

pub fn read_identity(
    token_kind: TokenKind,
    user: &ForgeResponse,
    organizations: &ForgeResponse,
) -> Result<Identity, ForgeError> {
    reject_unless_accepted(user)?;
    reject_unless_accepted(organizations)?;

    let user_payload: UserPayload = decode(&user.body)?;
    let organization_payload: Vec<OrganizationPayload> = decode(&organizations.body)?;

    Ok(Identity {
        login: user_payload.login,
        node_id: user_payload.node_id,
        organizations: organization_payload
            .into_iter()
            .map(|organization| organization.login)
            .collect(),
        scopes: GrantedScopes::parse(user.granted_scopes.as_deref().unwrap_or("")),
        token_kind,
        single_sign_on: SingleSignOn::parse(organizations.single_sign_on.as_deref()),
    })
}

fn reject_unless_accepted(response: &ForgeResponse) -> Result<(), ForgeError> {
    match response.status {
        200 => Ok(()),
        401 => Err(ForgeError::TokenRejected {
            message: forge_message(&response.body),
        }),
        403 => Err(ForgeError::AccessForbidden {
            message: forge_message(&response.body),
        }),
        status => Err(ForgeError::UnexpectedStatus {
            status,
            message: forge_message(&response.body),
        }),
    }
}

fn decode<T: for<'a> Deserialize<'a>>(body: &str) -> Result<T, ForgeError> {
    serde_json::from_str(body).map_err(|error| ForgeError::MalformedPayload {
        message: error.to_string(),
    })
}

pub fn forge_message(body: &str) -> String {
    serde_json::from_str::<ErrorPayload>(body)
        .map(|payload| payload.message)
        .unwrap_or_else(|_| "the forge returned no readable message".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{SingleSignOn, TokenKind};

    #[test]
    fn a_classic_token_is_told_apart_from_a_fine_grained_one() {
        assert_eq!(TokenKind::of("ghp_abc"), TokenKind::PatClassic);
        assert_eq!(
            TokenKind::of("github_pat_11ABCDEF"),
            TokenKind::PatFineGrained
        );
        assert_eq!(TokenKind::of("gho_abc"), TokenKind::OAuthApp);
        assert_eq!(TokenKind::of("ghu_abc"), TokenKind::GitHubApp);
        assert_eq!(TokenKind::of("abc"), TokenKind::Unrecognised);
    }

    #[test]
    fn an_absent_single_sign_on_header_means_nothing_is_hidden() {
        assert_eq!(SingleSignOn::parse(None), SingleSignOn::NoRestriction);
        assert!(!SingleSignOn::parse(None).hides_organizations());
    }

    #[test]
    fn a_partial_results_header_names_the_organizations_that_were_hidden() {
        match SingleSignOn::parse(Some("partial-results; organizations=21,7")) {
            SingleSignOn::PartialResults { organization_ids } => {
                assert_eq!(organization_ids, vec![21, 7]);
            }
            other => panic!("expected partial results, got {other:?}"),
        }
    }

    #[test]
    fn a_required_header_carries_the_authorisation_url_to_show_the_user() {
        match SingleSignOn::parse(Some(
            "required; url=https://github.com/orgs/acme/sso?authorization_request=x",
        )) {
            SingleSignOn::AuthorizationRequired { url } => {
                assert!(url.starts_with("https://github.com/orgs/acme/sso"));
            }
            other => panic!("expected an authorisation request, got {other:?}"),
        }
    }

    #[test]
    fn an_unreadable_single_sign_on_header_is_treated_as_no_restriction() {
        assert_eq!(
            SingleSignOn::parse(Some("something-else")),
            SingleSignOn::NoRestriction
        );
    }

    #[test]
    fn a_partial_results_header_without_identifiers_still_signals_hidden_organizations() {
        let parsed = SingleSignOn::parse(Some("partial-results"));
        assert!(parsed.hides_organizations());
    }
}
