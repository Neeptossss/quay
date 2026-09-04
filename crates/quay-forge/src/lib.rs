#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod auth;
mod backoff;
mod budget;
mod capabilities;
mod conditional;
mod detail;
mod error;
mod governor;
mod keychain;
mod locks;
mod mutation_request;
mod notifications;
mod polling;
mod scopes;
mod search;
mod token;
mod transport;

pub use auth::{Identity, SingleSignOn, TokenKind, forge_message, read_identity};
pub use backoff::Backoff;
pub use budget::SlidingBudget;
pub use capabilities::{Capabilities, Capability, Reason, Support};
pub use conditional::CacheValidators;
pub use detail::fetch as fetch_pull_request;
pub use error::ForgeError;
pub use governor::{
    ForgeResponse, GovernorConfig, Health, OutboundRequest, RateGovernor, RateLimitSnapshot,
    RequestKind,
};
pub use keychain::Keychain;
pub use locks::DevLocks;
pub use mutation_request::{MutationTarget, build as build_mutation_request};
pub use notifications::{pull_request_locator, signal_from};
pub use polling::{PollingSource, SharedValidators, epoch_seconds};
pub use scopes::GrantedScopes;
pub use search::{SearchSource, repository_from_url};
pub use token::Token;
pub use transport::{
    ErrorPayload, NotificationPayload, NotificationRepositoryPayload, NotificationSubjectPayload,
    OrganizationPayload, UserPayload,
};
