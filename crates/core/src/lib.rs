//! Das Auto — domain logic. Pure crate: no UI, no blocking I/O.
//!
//! Platform-agnostic async *contracts* live in `api` (and their in-memory
//! fakes in `fakes`, behind the `fakes` feature); the actual I/O
//! implementations live in `dasauto-net`. This crate must compile for both
//! host targets (tests, clippy) and `wasm32-unknown-unknown` (used by the
//! app).

pub mod api;
pub mod entities;
pub mod money;
pub mod plate;
pub mod session;
#[cfg(feature = "fakes")]
pub mod fakes;
pub mod store;
pub mod sync;
pub mod vehicle_cache;

/// Re-exported for consumers that carry timestamps (entities, sync).
pub use chrono::{DateTime, NaiveDate, Utc};
/// Re-exported for consumers that generate row ids (api, app).
pub use uuid::Uuid;
