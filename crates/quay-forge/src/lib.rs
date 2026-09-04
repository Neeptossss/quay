#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod auth;
mod backoff;
mod budget;
mod capabilities;
mod conditional;
mod error;
mod governor;
mod keychain;
mod locks;
mod scopes;
mod token;
mod transport;

pub use auth::{Identity, SingleSignOn, TokenKind, forge_message, read_identity};
pub use backoff::Backoff;
pub use budget::SlidingBudget;
pub use capabilities::{Capabilities, Capability, Reason, Support};
pub use conditional::CacheValidators;
pub use error::ForgeError;
pub use governor::{
    ForgeResponse, GovernorConfig, Health, OutboundRequest, RateGovernor, RateLimitSnapshot,
    RequestKind,
};
pub use keychain::Keychain;
pub use locks::DevLocks;
pub use scopes::GrantedScopes;
pub use token::Token;
pub use transport::{
    ErrorPayload, NotificationPayload, NotificationRepositoryPayload, NotificationSubjectPayload,
    OrganizationPayload, UserPayload,
};
