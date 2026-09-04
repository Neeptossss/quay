#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod engine;
mod error;

pub use engine::{SyncEngine, TickReport};
pub use error::SyncError;
