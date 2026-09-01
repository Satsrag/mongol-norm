#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! # mongol-norm
//!
//! Shape-aware normalizer for Traditional Mongolian (Hudum) script — a pure-Rust twin of the
//! `mongol-norm` Python package living in the same repository.
//!
//! Design: `docs/superpowers/specs/2026-09-01-rust-core-design.md`.

/// Crate version (the Cargo package version; lockstep with the Python package version).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
