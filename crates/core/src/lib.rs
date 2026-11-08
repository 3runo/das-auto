//! Das Auto — domain logic. Pure crate: no UI, no I/O, no async.
//!
//! This crate must compile for both host targets (tests, clippy) and
//! `wasm32-unknown-unknown` (used by the app).

pub mod money;
pub mod plate;
