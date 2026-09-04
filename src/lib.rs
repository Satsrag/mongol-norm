#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! # mongol-norm
//!
//! Shape-aware normalizer for Traditional Mongolian (Hudum) script — the engine behind the
//! `mongol-norm` PyPI package, whose PyO3 binding lives in the same repository under `python/`.
//!
//! Traditional Mongolian in Unicode has a fundamental problem: the same visible word can be
//! encoded in many different code-point sequences (letters share glyphs, FVS selectors create
//! equivalent spellings, joiners and MVS suffixes collapse more letters together). This crate
//! implements a lightweight shaping engine — the UTN #57 v4 Mongolian-specific phase, driven only
//! by the rule data of UTN #57 / mongfontbuilder, no font needed — and, on top of it, a canonical
//! normalizer: within the bundled table's domain, `shape(x) == shape(y)` implies
//! `normalize(x) == normalize(y)`, and `shape(normalize(x)) == shape(x)`.
//!
//! * [`Shaper::shape`] — text → written-unit sequence (`Mvs` / `Nirugu` / `Zwj` appear verbatim),
//!   with the nine duplicate encodings unified (see [`Shaper::shape_raw`])
//! * [`Shaper::same_shape`] — do two encodings render identically?
//! * [`Shaper::normalize`] / [`Shaper::normalize_text`] — canonical, FVS-pinned Unicode
//! * [`Shaper::normalize_written_units`] / [`Shaper::normalize_positioned_written_units`] —
//!   encode written units directly
//! * [`Shaper::trace`] — per-rule condition transitions, for debugging and the golden fixtures
//!
//! The crate has no dependencies and builds for `wasm32-unknown-unknown`. Its data tables are
//! generated from the repository's JSON by `python/scripts/gen_rust_tables.py`, and the Python
//! package of the same version calls straight into this crate. Design:
//! <https://github.com/Satsrag/mongol-norm/blob/main/docs/superpowers/specs/2026-09-01-rust-core-design.md>.

/// The crate README is compiled and run as a doctest, so its example cannot rot.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

mod duplicates;
mod error;
// clippy::all silences style lints on generated code (see the design doc).
#[allow(clippy::all)]
mod generated;
mod normalize;
mod rules;
mod shaper;
mod tables;
mod token;
mod unicode;
mod written_units;

#[doc(hidden)]
pub mod cli;

pub use error::Error;
pub use generated::enums::{Alias, Condition, WrittenUnit};
pub use shaper::{ConditionChange, RuleTransition, ShapeTrace, Shaper, TokenDetail};
pub use tables::{Fvs, Locale, Position, UnitPosition};
pub use unicode::{is_mongolian_letter, is_mongolian_word_char};
pub use written_units::{PositionedWrittenUnit, MAX_POSITIONED_RECORDS};

/// Crate version (the Cargo package version; lockstep with the Python package version).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
