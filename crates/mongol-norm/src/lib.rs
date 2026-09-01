#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! # mongol-norm
//!
//! Shape-aware normalizer for Traditional Mongolian (Hudum) script — a pure-Rust twin of the
//! `mongol-norm` Python package living in the same repository.
//!
//! Design: `docs/superpowers/specs/2026-09-01-rust-core-design.md`.

mod error;
// dead_code: TEMPORARY — the shaper/rules/normalize modules (later tasks) consume this generated
// data; remove once they do. clippy::all silences style lints on generated code (see the design doc).
#[allow(clippy::all, dead_code)]
mod generated;
// dead_code: TEMPORARY — the shaper/rules/normalize modules (later tasks) consume these tables;
// remove once they do.
#[allow(dead_code)]
mod tables;
mod token;
mod unicode;

pub use error::Error;
pub use generated::enums::{Alias, Condition, WrittenUnit};
pub use tables::{Fvs, Locale, Position, UnitPosition};
pub use unicode::{is_mongolian_letter, is_mongolian_word_char};

/// Crate version (the Cargo package version; lockstep with the Python package version).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
