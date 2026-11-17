//! `forge-worker`: the data plane (docs/platform/01-architecture.md §2).
//!
//! The protocol types are always available. The engine role (which links the original crates)
//! is behind the default `engine` feature, so `forge-api` can depend on this crate for the
//! protocol alone, with the engine off.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod protocol;

#[cfg(feature = "engine")]
pub mod catalog;
#[cfg(feature = "engine")]
pub mod engine_role;

#[cfg(test)]
mod engine_tests;
