#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! # mongol-norm
//!
//! Traditional Mongolian (Hudum) same-shape comparison and normalization tool — the engine
//! behind the `mongol-norm` PyPI package, whose PyO3 binding lives in the same repository
//! under `python/`. It implements the Mongolian shaping rules of
//! [UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) (GB/T 25914—2023) using
//! data and rules derived from [mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder).
//! It converts
//! Mongolian text that has the same written form but different Unicode encodings into a
//! unified encoding, making search, deduplication, and corpus processing easier.
//!
//! This crate is intended for Mongolian text under the UTN #57 (GB/T 25914—2023) rules.
//! Text that uses other standards or encodings — for example Delehi, Oyun, and Menk related
//! to GB/T 25914—2010, as well as Menk old shape codes, ZCode, Zvvnmod, and the like — must
//! first be converted to UTN #57 (GB/T 25914—2023) before this crate can be used. For encoding
//! conversion you may consider [mongol-convert](https://github.com/Satsrag/mongol-convert);
//! that library is also currently in Beta, so use it with care and verify the conversion
//! results.
//!
//! > **Beta.** `normalize` output is a unified encoding by **glyph shape**, **not standard
//! > spelling**, and may change in a later release. Keep the original text; when storing
//! > normalized keys, also record [`Shaper::canonical_version`] and rebuild after a version
//! > change.
//!
//! Within the bundled table's domain, `shape(x) == shape(y)` implies
//! `normalize(x) == normalize(y)`, and `shape(normalize(x)) == shape(x)`.
//!
//! * [`Shaper::shape`] — text → written-unit sequence (redundant controls removed, repeated MVS merged,
//!   and every `Nirugu` preserved), with
//!   the nine duplicate encodings unified (see [`Shaper::shape_raw`])
//! * [`Shaper::same_shape`] — do two encodings produce the same canonical written-unit sequence
//!   under the abstract UTN #57 v4 / `mongfontbuilder` shaping model? (Not font-specific
//!   rendering equivalence.)
//! * [`Shaper::normalize`] / [`Shaper::normalize_text`] — canonical, FVS-pinned Unicode
//! * [`Shaper::normalize_written_units`] / [`Shaper::normalize_positioned_written_units`] —
//!   encode written units directly
//! * [`Shaper::trace`] — per-rule condition transitions, for debugging and the golden fixtures
//! * [`FontShaper`] (Cargo feature `font`) — the same `shape` through HarfBuzz rules: harfrust
//!   running mongfontbuilder's unified font, bundled without its outlines, for every writing
//!   system of the block
//!
//! ## Model and limitations
//!
//! [`Shaper::shape`] and [`Shaper::same_shape`] define **Mongolian-specific shaping
//! equivalence** under `mongol-norm`'s abstract shaping model (UTN #57 v4 plus
//! `mongfontbuilder`-derived data and rules, along with control canonicalization and
//! duplicate-unit unification). The rule engine does not invoke HarfBuzz with a specific font
//! or compare rendered glyph IDs or outlines. For example, `"ᠰᠠᠢᠨ"` (`SA + A + I + NA`) and
//! `"ᠰᠨ᠌ᠢᠢᠨ"` (`SA + NA+FVS2 + I + I + NA`) both shape to `S+A+I+I+A`, yet a real-world font
//! with different FVS coverage or OpenType rules may render them differently. Even the
//! optional [`FontShaper`] (`font` feature) runs `harfrust` only on the single bundled
//! `mongfontbuilder` reference font (`fonts/hudum.otf`, without outlines) and compares
//! written units rather than arbitrary fonts or outlines. Applications requiring a specific
//! font's final rendering to match should validate directly with HarfBuzz and that font.
//!
//! The crate has no dependencies by default and builds for `wasm32-unknown-unknown` (the `font`
//! feature adds harfrust and needs Rust 1.85). Its data tables are generated from the
//! repository's JSON by `python/scripts/gen_rust_tables.py`, and the Python package of the same
//! version calls straight into this crate. Design:
//! <https://github.com/Satsrag/mongol-norm/blob/main/docs/superpowers/specs/2026-09-01-rust-core-design.md>.

/// The crate README is compiled and run as a doctest, so its example cannot rot.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

mod controls;
mod duplicates;
mod error;
#[cfg(feature = "font")]
mod font;
// clippy::all silences style lints on generated code (see the design doc).
#[allow(clippy::all)]
mod generated;
mod normalize;
mod redundant_zwj;
mod redundant_zwnj;
mod rules;
mod shaper;
mod tables;
mod token;
mod unicode;
mod written_units;

#[doc(hidden)]
pub mod cli;

pub use error::Error;
#[cfg(feature = "font")]
pub use font::{FontShaper, ShapedGlyph};
pub use generated::enums::{Alias, Condition, WrittenUnit};
pub use shaper::{ConditionChange, RuleTransition, ShapeTrace, Shaper, TokenDetail};
pub use tables::{Fvs, Locale, Position, UnitPosition};
pub use unicode::{is_mongolian_letter, is_mongolian_word_char};
pub use written_units::{PositionedWrittenUnit, MAX_POSITIONED_RECORDS};

/// Crate version (the Cargo package version; lockstep with the Python package version).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
