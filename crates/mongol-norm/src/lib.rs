#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! # mongol-norm
//!
//! Shape-aware normalizer for Traditional Mongolian (Hudum) script — a pure-Rust twin of the
//! `mongol-norm` Python package living in the same repository.
//!
//! Design: `docs/superpowers/specs/2026-09-01-rust-core-design.md`.

mod error;
// dead_code: TEMPORARY — `generated::mng_normalize` (the MNG canonical-normalization table) has
// no consumer until `normalize.rs` lands (Task 8); remove once it does. clippy::all silences
// style lints on generated code (see the design doc).
#[allow(clippy::all, dead_code)]
mod generated;
mod rules;
mod shaper;
mod tables;
mod token;
mod unicode;

pub use error::Error;
pub use generated::enums::{Alias, Condition, WrittenUnit};
pub use shaper::{ConditionChange, RuleTransition, ShapeTrace, Shaper, TokenDetail};
pub use tables::{Fvs, Locale, Position, UnitPosition};
pub use unicode::{is_mongolian_letter, is_mongolian_word_char};

/// Crate version (the Cargo package version; lockstep with the Python package version).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
