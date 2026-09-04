#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod dataset;
pub mod detail;
mod error;
pub mod inbox;
pub mod migrations;
pub mod mutations;
pub mod optimistic;
pub mod preload;
pub mod resource_cache;
pub mod schema;
mod store;
pub mod write;

pub use error::StoreError;
pub use mutations::{OptimisticChange, ReplayReport};
pub use preload::Utilisation;
pub use resource_cache::CacheEntry;
pub use store::{Durability, Store};
