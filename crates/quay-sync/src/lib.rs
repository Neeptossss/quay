#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod engine;
mod error;

pub use engine::{
    NOTIFICATIONS_KEY, SyncEngine, TickReport, notification_validators,
    remember_notification_validators,
};
pub use error::SyncError;
