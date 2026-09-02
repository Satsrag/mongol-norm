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
            // `escape_debug` mirrors Python's `repr(ch)` for control characters only: a newline
            // prints as `\n`, a printable character as itself. Quotes and non-printable
            // non-ASCII are spelled differently — Rust escapes `'` as `\'` and writes `\u{85}`
            // where Python switches the quote style and writes `\x85`.
            Error::NonMongolianChar { ch, index } => write!(
                f,
                "non-Mongolian character '{}' (U+{:04X}) at index {}: shape() / normalize() \
                 accept only Mongolian letters + FVS/MVS/NNBSP/Nirugu/ZWJ. For mixed-script \
                 input use normalize_text().",
                ch.escape_debug(),
                *ch as u32,
                index
            ),
            Error::NormalizationFallback { written_units, .. } => {
                let names: Vec<&str> = written_units.iter().map(|u| u.as_str()).collect();
                write!(
                    f,
                    "normalization fallback: no canonical encoding for written units {}",
                    names.join("+")
                )
            }
            // Python appends "; generate it with scripts/gen_normalize_table.py"; deliberately
            // dropped here because the Rust tables are compiled in, not generated on demand.
            Error::NormalizeUnsupported { locale } => write!(
                f,
                "no bundled normalize table for locale '{}'",
                locale.as_str()
            ),
            Error::UnknownWrittenUnit { index, unit } => {
                // `unit` is arbitrary user text (Python renders it with `repr()`), so escape it —
                // a control character must never reach a terminal raw. Printable ASCII, which
                // every real unit name is, passes through unchanged. As above, the escaping
                // matches Python's `repr()` for control characters only; quotes and
                // non-printable non-ASCII use Rust's `\'` / `\u{…}` spelling instead.
                write!(
                    f,
                    "written_units[{index}] is unknown: '{}'",
                    unit.escape_debug()
                )
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

#[cfg(test)]
mod tests {
    use super::*;

    /// These strings are a byte-exact contract: later CLI tests assert on them directly, the
    /// same way the Python CLI's own error messages are asserted on.
    #[test]
    fn display_wording_matches_python() {
        assert_eq!(
            Error::NonMongolianChar { ch: 'x', index: 3 }.to_string(),
            "non-Mongolian character 'x' (U+0078) at index 3: shape() / normalize() accept only \
             Mongolian letters + FVS/MVS/NNBSP/Nirugu/ZWJ. For mixed-script input use \
             normalize_text()."
        );
        // Python's `{ch!r}` escapes a control character; so must ours (`repr('\n')` == r"'\n'").
        assert_eq!(
            Error::NonMongolianChar { ch: '\n', index: 4 }.to_string(),
            "non-Mongolian character '\\n' (U+000A) at index 4: shape() / normalize() accept only \
             Mongolian letters + FVS/MVS/NNBSP/Nirugu/ZWJ. For mixed-script input use \
             normalize_text()."
        );
        assert_eq!(
            Error::NormalizationFallback {
                text: "t".into(),
                written_units: vec![
                    WrittenUnit::S,
                    WrittenUnit::A,
                    WrittenUnit::I,
                    WrittenUnit::I,
                    WrittenUnit::A,
                ],
            }
            .to_string(),
            "normalization fallback: no canonical encoding for written units S+A+I+I+A"
        );
        assert_eq!(
            Error::NormalizeUnsupported {
                locale: Locale::Tod
            }
            .to_string(),
            "no bundled normalize table for locale 'TOD'"
        );
        assert_eq!(
            Error::UnknownWrittenUnit {
                index: 1,
                unit: "Unknown".into()
            }
            .to_string(),
            "written_units[1] is unknown: 'Unknown'"
        );
        // A name is arbitrary user text; control characters are escaped, never emitted raw.
        assert_eq!(
            Error::UnknownWrittenUnit {
                index: 0,
                unit: "A\0B".into()
            }
            .to_string(),
            "written_units[0] is unknown: 'A\\0B'"
        );
        assert_eq!(
            Error::UnsupportedWrittenUnit {
                index: 1,
                unit: WrittenUnit::E
            }
            .to_string(),
            "written_units[1] is unknown: 'E'"
        );
        assert_eq!(
            Error::NoCanonicalEncoding.to_string(),
            "written-unit sequence has no canonical MNG encoding"
        );
        assert_eq!(
            Error::ExplicitZwj.to_string(),
            "unsupported positioned control 'Zwj'"
        );
        assert_eq!(
            Error::ControlRequiresControlPosition {
                index: 0,
                unit: WrittenUnit::Mvs
            }
            .to_string(),
            "positioned_units[0] control 'Mvs' requires position 'control'"
        );
        assert_eq!(
            Error::UnsupportedPositionedUnit {
                index: 0,
                unit: WrittenUnit::F,
                position: UnitPosition::Isol,
            }
            .to_string(),
            "unsupported positioned written unit 'F:isol'"
        );
        assert_eq!(
            Error::ChainPositionMismatch.to_string(),
            "positioned written-unit sequence has no canonical MNG encoding in the supplied \
             context"
        );
        assert_eq!(
            Error::TooManyRecords { max: 1024 }.to_string(),
            "positioned_units accepts at most 1024 records"
        );
        assert_eq!(Error::InvalidUnitSpec("x".into()).to_string(), "x");
        assert_eq!(
            Error::UnknownName {
                kind: "locale",
                name: "XX".into()
            }
            .to_string(),
            "unknown locale 'XX'"
        );
    }
}
