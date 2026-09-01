//! The crate's single error type. `Display` wording mirrors the Python exceptions so the CLI
//! prints the same messages as the Python CLI.

use std::fmt;

use crate::{Locale, UnitPosition, WrittenUnit};

/// Everything that can go wrong in this crate.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// `shape()` / `normalize()` input contains a character that is not a Mongolian letter,
    /// FVS, MVS, NNBSP, nirugu or ZWJ. `index` is a `char` index (not a byte offset).
    NonMongolianChar {
        /// The offending character.
        ch: char,
        /// Its `char` index in the input.
        index: usize,
    },
    /// Strict normalization found no canonical encoding for the written-unit shape
    /// (Python `NormalizationFallbackError`).
    NormalizationFallback {
        /// The input text.
        text: String,
        /// Its shape, which the normalize table could not encode.
        written_units: Vec<WrittenUnit>,
    },
    /// The locale ships no normalize table (only `MNG` has one).
    NormalizeUnsupported {
        /// The shaper's locale.
        locale: Locale,
    },
    /// `parse_written_units`: a name that is neither a unit known to the normalize table nor
    /// `Mvs`/`Nirugu`/`Zwj`.
    UnknownWrittenUnit {
        /// Index of the unit in the parsed sequence.
        index: usize,
        /// The unparseable name.
        unit: String,
    },
    /// `normalize_written_units`: a written unit this shaper's normalize table does not know.
    UnsupportedWrittenUnit {
        /// Index of the unit in the input sequence.
        index: usize,
        /// The unsupported unit.
        unit: WrittenUnit,
    },
    /// `normalize_written_units`: the sequence has no canonical MNG encoding (or the candidate
    /// does not reshape to exactly the requested sequence).
    NoCanonicalEncoding,
    /// `normalize_positioned_written_units`: explicit `Zwj` records are rejected.
    ExplicitZwj,
    /// `normalize_positioned_written_units`: `Mvs` / `Nirugu` need [`UnitPosition::Control`].
    ControlRequiresControlPosition {
        /// Index of the record.
        index: usize,
        /// The control unit.
        unit: WrittenUnit,
    },
    /// `normalize_positioned_written_units`: `(unit, position)` is not in the HUD inventory
    /// (a letter with `Control` lands here too).
    UnsupportedPositionedUnit {
        /// Index of the record.
        index: usize,
        /// The unit.
        unit: WrittenUnit,
        /// The requested position.
        position: UnitPosition,
    },
    /// `normalize_positioned_written_units`: the records do not form a valid init…fina chain in
    /// the supplied joining context.
    ChainPositionMismatch,
    /// `normalize_positioned_written_units`: more than the maximum number of records.
    TooManyRecords {
        /// The maximum accepted.
        max: usize,
    },
    /// `parse_written_units`: empty unit, whitespace, or an ambiguous compact segmentation.
    InvalidUnitSpec(String),
    /// A contract name (`Locale`, `Position`, `WrittenUnit`, …) failed to parse.
    UnknownName {
        /// What kind of name was expected (`"locale"`, `"written unit"`, …).
        kind: &'static str,
        /// The rejected text.
        name: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NonMongolianChar { ch, index } => write!(
                f,
                "non-Mongolian character '{}' (U+{:04X}) at index {}: shape() / normalize() \
                 accept only Mongolian letters + FVS/MVS/NNBSP/Nirugu/ZWJ. For mixed-script \
                 input use normalize_text().",
                ch, *ch as u32, index
            ),
            Error::NormalizationFallback { written_units, .. } => {
                let names: Vec<&str> = written_units.iter().map(|u| u.as_str()).collect();
                write!(
                    f,
                    "normalization fallback: no canonical encoding for written units {}",
                    names.join("+")
                )
            }
            Error::NormalizeUnsupported { locale } => write!(
                f,
                "no bundled normalize table for locale '{}'",
                locale.as_str()
            ),
            Error::UnknownWrittenUnit { index, unit } => {
                write!(f, "written_units[{index}] is unknown: '{unit}'")
            }
            Error::UnsupportedWrittenUnit { index, unit } => {
                write!(f, "written_units[{index}] is unknown: '{}'", unit.as_str())
            }
            Error::NoCanonicalEncoding => {
                f.write_str("written-unit sequence has no canonical MNG encoding")
            }
            Error::ExplicitZwj => f.write_str("unsupported positioned control 'Zwj'"),
            Error::ControlRequiresControlPosition { index, unit } => write!(
                f,
                "positioned_units[{index}] control '{}' requires position 'control'",
                unit.as_str()
            ),
            Error::UnsupportedPositionedUnit { unit, position, .. } => write!(
                f,
                "unsupported positioned written unit '{}:{}'",
                unit.as_str(),
                position.as_str()
            ),
            Error::ChainPositionMismatch => f.write_str(
                "positioned written-unit sequence has no canonical MNG encoding in the \
                 supplied context",
            ),
            Error::TooManyRecords { max } => {
                write!(f, "positioned_units accepts at most {max} records")
            }
            Error::InvalidUnitSpec(message) => f.write_str(message),
            Error::UnknownName { kind, name } => write!(f, "unknown {kind} '{name}'"),
        }
    }
}

impl std::error::Error for Error {}
