//! Written-unit input APIs — a port of `normalize_written_units`,
//! `normalize_positioned_written_units` and `_parse_written_units` from `mongol_norm/shaper.py`.

use crate::generated::enums::WrittenUnit;
use crate::normalize::{is_joiner, slot_position};
use crate::shaper::Shaper;
use crate::tables::{Position, UnitPosition};
use crate::Error;

/// One record of [`Shaper::normalize_positioned_written_units`]: a written unit with its
/// authoritative HUD inventory position (`Control` for `Mvs` / `Nirugu`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PositionedWrittenUnit {
    /// The written unit.
    pub unit: WrittenUnit,
    /// Its HUD inventory position.
    pub position: UnitPosition,
}

impl PositionedWrittenUnit {
    /// Build a record.
    pub const fn new(unit: WrittenUnit, position: UnitPosition) -> PositionedWrittenUnit {
        PositionedWrittenUnit { unit, position }
    }
}

/// The positioned API accepts at most this many records (Python parity).
pub const MAX_POSITIONED_RECORDS: usize = 1024;

enum PositionedPart {
    Control(WrittenUnit),
    Chain(Vec<(WrittenUnit, Position)>),
}

impl Shaper {
    /// Encode an ordered written-unit sequence (e.g. the output of [`Shaper::shape`]) as
    /// canonical Unicode. Letter positions are inferred from order and the structural tokens;
    /// ZWJ is emitted only where `Zwj` is present in the request. The result is accepted only if
    /// it reshapes to exactly the requested sequence.
    pub fn normalize_written_units(&self, units: &[WrittenUnit]) -> Result<String, Error> {
        if units.is_empty() {
            return Ok(String::new());
        }
        let table = self.table()?;
        for (index, unit) in units.iter().enumerate() {
            if !table.known_units.contains(unit) {
                return Err(Error::UnsupportedWrittenUnit { index, unit: *unit });
            }
        }
        let canonical = self.canonical_for_shape(units)?;
        if canonical.is_empty() || self.shape(&canonical)? != units {
            return Err(Error::NoCanonicalEncoding);
        }
        Ok(canonical)
    }

    /// Encode explicit HUD-position records as canonical Unicode (the API `zvvnmod-utn57`
    /// uses). A complete multi-record chain runs `init…fina`; an incomplete edge gets an implicit
    /// ZWJ; a single `init` record is encoded bare except `O:init`, which takes a trailing ZWJ;
    /// single `medi` / `fina` records get the joining context their position needs. `Mvs` and
    /// `Nirugu` require `Control`; explicit `Zwj` is rejected; at most
    /// [`MAX_POSITIONED_RECORDS`] records.
    pub fn normalize_positioned_written_units(
        &self,
        records: &[PositionedWrittenUnit],
    ) -> Result<String, Error> {
        if records.len() > MAX_POSITIONED_RECORDS {
            return Err(Error::TooManyRecords {
                max: MAX_POSITIONED_RECORDS,
            });
        }
        if records.iter().any(|record| record.unit == WrittenUnit::Zwj) {
            return Err(Error::ExplicitZwj);
        }
        if records.is_empty() {
            return Ok(String::new());
        }
        let table = self.table()?;
        for (index, record) in records.iter().enumerate() {
            if matches!(record.unit, WrittenUnit::Mvs | WrittenUnit::Nirugu) {
                if record.position != UnitPosition::Control {
                    return Err(Error::ControlRequiresControlPosition {
                        index,
                        unit: record.unit,
                    });
                }
                continue;
            }
            let supported = record
                .position
                .as_position()
                .is_some_and(|position| table.positioned_units.contains(&(record.unit, position)));
            if !supported {
                return Err(Error::UnsupportedPositionedUnit {
                    index,
                    unit: record.unit,
                    position: record.position,
                });
            }
        }

        let mut parts: Vec<PositionedPart> = Vec::new();
        let mut chain: Vec<(WrittenUnit, Position)> = Vec::new();
        for record in records {
            match record.position.as_position() {
                None => {
                    if !chain.is_empty() {
                        parts.push(PositionedPart::Chain(std::mem::take(&mut chain)));
                    }
                    parts.push(PositionedPart::Control(record.unit));
                }
                Some(position) => chain.push((record.unit, position)),
            }
        }
        if !chain.is_empty() {
            parts.push(PositionedPart::Chain(chain));
        }
        let is_joiner_part = |part: &PositionedPart| matches!(part, PositionedPart::Control(unit) if is_joiner(*unit));

        let mut written: Vec<WrittenUnit> = Vec::new();
        for index in 0..parts.len() {
            let body = match &parts[index] {
                PositionedPart::Control(unit) => {
                    written.push(*unit);
                    continue;
                }
                PositionedPart::Chain(body) => body,
            };
            let joined_left = index > 0 && is_joiner_part(&parts[index - 1]);
            let joined_right = index + 1 < parts.len() && is_joiner_part(&parts[index + 1]);
            if body.len() == 1 {
                let (unit, position) = body[0];
                if records.len() == 1 && unit == WrittenUnit::O && position == Position::Init {
                    written.extend([unit, WrittenUnit::Zwj]);
                    continue;
                }
                if matches!(position, Position::Medi | Position::Fina) && !joined_left {
                    written.push(WrittenUnit::Zwj);
                }
                written.push(unit);
                if position == Position::Medi && !joined_right {
                    written.push(WrittenUnit::Zwj);
                }
                continue;
            }
            let padded_left = joined_left || body[0].1 != Position::Init;
            let padded_right = joined_right || body[body.len() - 1].1 != Position::Fina;
            let padded_count = body.len() + usize::from(padded_left) + usize::from(padded_right);
            for (offset, (_unit, position)) in body.iter().enumerate() {
                let expected = slot_position(offset + usize::from(padded_left), 1, padded_count);
                if *position != expected {
                    return Err(Error::ChainPositionMismatch);
                }
            }
            if padded_left && !joined_left {
                written.push(WrittenUnit::Zwj);
            }
            written.extend(body.iter().map(|(unit, _)| *unit));
            if padded_right && !joined_right {
                written.push(WrittenUnit::Zwj);
            }
        }
        self.normalize_written_units(&written)
    }

    /// Parse the CLI's written-unit spelling: explicit `+`-separated names (`B+Aa`) or a
    /// compact PascalCase string with exactly one segmentation over this shaper's known units
    /// (`BZwj`). One trailing newline is tolerated; an ambiguous compact string is rejected.
    pub fn parse_written_units(&self, text: &str) -> Result<Vec<WrittenUnit>, Error> {
        let table = self.table()?;
        let mut vocabulary: Vec<&str> =
            table.known_units.iter().map(|unit| unit.as_str()).collect();
        parse_unit_names(text, &mut vocabulary)?
            .iter()
            .enumerate()
            .map(|(index, name)| {
                // Membership is tested against the TABLE's vocabulary, not merely "is this a
                // `WrittenUnit` variant": Python leaves every name unresolved here and lets
                // `normalize_written_units` reject the first one missing from `known_units`, so a
                // unit the enum knows but this locale's table does not (e.g. the Todo unit `E`)
                // must fail at Python's index too. — see shaper.py::normalize_written_units
                name.parse::<WrittenUnit>()
                    .ok()
                    .filter(|unit| table.known_units.contains(unit))
                    .ok_or_else(|| Error::UnknownWrittenUnit {
                        index,
                        unit: name.clone(),
                    })
            })
            .collect()
    }
}

fn strip_one_newline(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .or_else(|| text.strip_suffix('\r'))
        .unwrap_or(text)
}

/// Python `_parse_written_units`: explicit `+` names, or a unique compact segmentation over
/// `vocabulary` (sorted here by `(-len, name)`, the Python tie-break). Returns names; the caller
/// maps them to units.
pub(crate) fn parse_unit_names<'v>(
    text: &str,
    vocabulary: &mut Vec<&'v str>,
) -> Result<Vec<String>, Error> {
    let text = strip_one_newline(text);
    if text.is_empty() {
        return Ok(Vec::new());
    }
    if text.chars().any(char::is_whitespace) {
        return Err(Error::InvalidUnitSpec(
            "written units cannot be empty or contain whitespace".to_owned(),
        ));
    }
    if text.contains('+') {
        let units: Vec<&str> = text.split('+').collect();
        if units
            .iter()
            .any(|unit| unit.is_empty() || *unit != unit.trim())
        {
            return Err(Error::InvalidUnitSpec(
                "written units cannot be empty or contain whitespace; separate explicit units \
                 with '+' (for example A+Aa+B+Zwj)"
                    .to_owned(),
            ));
        }
        return Ok(units.into_iter().map(str::to_owned).collect());
    }
    vocabulary.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    let n = text.len();
    let mut parse_counts = vec![0u8; n + 1];
    let mut choices: Vec<Option<(&'v str, usize)>> = vec![None; n + 1];
    parse_counts[n] = 1;
    for offset in (0..n).rev() {
        if !text.is_char_boundary(offset) {
            continue; // inside a multi-byte char: no unit can start here (count stays 0)
        }
        let mut count = 0u8;
        let mut choice = None;
        for unit in vocabulary.iter() {
            let end = offset + unit.len();
            if end > n || !text[offset..].starts_with(unit) || parse_counts[end] == 0 {
                continue;
            }
            if count == 0 && parse_counts[end] == 1 {
                choice = Some((*unit, end));
            } else {
                choice = None;
            }
            count = (count + parse_counts[end]).min(2);
            if count == 2 {
                break;
            }
        }
        parse_counts[offset] = count;
        if count == 1 {
            choices[offset] = choice;
        }
    }
    match parse_counts[0] {
        0 => Err(Error::UnknownWrittenUnit {
            index: 0,
            unit: text.to_owned(),
        }),
        1 => {
            let mut units = Vec::new();
            let mut offset = 0;
            while offset < n {
                let (unit, end) = choices[offset].expect("a unique parse records its choice");
                units.push(unit.to_owned());
                offset = end;
            }
            Ok(units)
        }
        _ => Err(Error::InvalidUnitSpec(
            "compact written-unit sequence is ambiguous; separate units with '+'".to_owned(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambiguous_compact_units_require_plus_separators() {
        // Python: `_parse_written_units("AAA", {"A", "AA"})` → "ambiguous … +"
        let mut vocabulary = vec!["A", "AA"];
        let error = parse_unit_names("AAA", &mut vocabulary).unwrap_err();
        assert!(
            matches!(&error, Error::InvalidUnitSpec(m) if m.contains("ambiguous") && m.contains('+'))
        );
        assert_eq!(
            parse_unit_names("AA", &mut vec!["A", "AA"]).unwrap_err(),
            error
        );
        assert_eq!(
            parse_unit_names("AAB", &mut vec!["A", "AA", "B"]).unwrap_err(),
            error
        );
        assert_eq!(
            parse_unit_names("AAB", &mut vec!["AA", "B"]).unwrap(),
            ["AA", "B"]
        );
    }

    #[test]
    fn multibyte_input_never_splits_a_char() {
        assert_eq!(
            parse_unit_names("A\u{1820}", &mut vec!["A"]).unwrap_err(),
            Error::UnknownWrittenUnit {
                index: 0,
                unit: "A\u{1820}".to_owned()
            }
        );
    }

    #[test]
    fn trailing_transport_newline_is_stripped_once() {
        assert_eq!(parse_unit_names("A\r\n", &mut vec!["A"]).unwrap(), ["A"]);
        assert_eq!(parse_unit_names("A\n", &mut vec!["A"]).unwrap(), ["A"]);
        assert_eq!(
            parse_unit_names("\n", &mut vec!["A"]).unwrap(),
            Vec::<String>::new()
        );
        assert!(parse_unit_names("A\n\n", &mut vec!["A"]).is_err());
    }
}
