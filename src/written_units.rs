//! Written-unit input APIs — a port of `normalize_written_units`,
//! `normalize_positioned_written_units` and `_parse_written_units` from `mongol_norm/shaper.py`.

use crate::duplicates::collapse;
use crate::generated::enums::WrittenUnit;
use crate::normalize::{is_joiner, slot_position};
use crate::shaper::Shaper;
use crate::tables::{Position, UnitPosition};
use crate::Error;

/// One record of [`Shaper::normalize_positioned_written_units`]: a written unit with its
/// authoritative HUD inventory position (`Control` for `Mvs` / `Nirugu`).
///
/// This is an *input* type: both fields are public and it is deliberately not
/// `#[non_exhaustive]` (unlike the crate's result types), so callers can build records with a
/// struct literal or [`PositionedWrittenUnit::new`] and destructure them freely.
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

/// Does `part` cursively join the chain next to it? Only a joiner control does; a neighbouring
/// chain never does (Python tests `parts[i][0] in _JOINER_TOKENS`, and a chain part's tag is the
/// literal `"chain"`).
fn is_joiner_part(part: &PositionedPart) -> bool {
    matches!(part, PositionedPart::Control(unit) if is_joiner(*unit))
}

impl Shaper {
    /// Encode an ordered written-unit sequence (e.g. the output of [`Shaper::shape`]) as
    /// canonical Unicode. Letter positions are inferred from order and the structural tokens;
    /// ZWJ is emitted only where `Zwj` is present in the request. The result is accepted only if
    /// it reshapes to exactly the requested sequence.
    ///
    /// An empty sequence returns `""` — without consulting the table, so it succeeds on every
    /// locale.
    ///
    /// # Errors
    ///
    /// - [`Error::NormalizeUnsupported`] — this locale has no bundled normalize table.
    /// - [`Error::UnsupportedWrittenUnit`] — `units[index]` is outside the table's vocabulary
    ///   (for example the Todo unit `E` on an MNG shaper). The first offender is reported.
    /// - [`Error::NoCanonicalEncoding`] — the table covers every unit, but the sequence has no
    ///   encoding that reshapes back to it.
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
        // Duplicate encodings are accepted on input — `Dd`, medial `H`/`Hx` are real units in
        // the standard and in callers' data — and folded before encoding, so they get the same
        // canonical text as the sequence they render identically to.
        let units = collapse(units);
        let canonical = self.canonical_for_shape(&units)?;
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
    ///
    /// # Validation order
    ///
    /// Checks run in a fixed order, and it is observable — callers such as `zvvnmod-utn57`
    /// dispatch on the variant, so the *first* failing check decides which one they see:
    ///
    /// 1. record limit — 2000 records containing a `Zwj` is [`Error::TooManyRecords`], not
    ///    [`Error::ExplicitZwj`], and an over-limit request on a table-less locale is
    ///    [`Error::TooManyRecords`], not [`Error::NormalizeUnsupported`];
    /// 2. explicit `Zwj` anywhere in the request;
    /// 3. empty request — returns `""` before the table is consulted, so it succeeds on every
    ///    locale;
    /// 4. the normalize table;
    /// 5. per record, the control/inventory check (`Mvs` and `Nirugu` need `Control`; every other
    ///    `(unit, position)` must be in the HUD inventory), reporting the first offender;
    /// 6. per multi-record chain, that the declared positions match the padded chain;
    /// 7. delegation to [`Shaper::normalize_written_units`].
    ///
    /// # Errors
    ///
    /// - [`Error::TooManyRecords`], [`Error::ExplicitZwj`], [`Error::NormalizeUnsupported`],
    ///   [`Error::ControlRequiresControlPosition`], [`Error::UnsupportedPositionedUnit`],
    ///   [`Error::ChainPositionMismatch`] — as numbered above.
    /// - Anything [`Shaper::normalize_written_units`] returns. **Its indices point into the
    ///   expanded written-unit sequence — after implicit ZWJs were inserted — not into
    ///   `records`.**
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
                // `records.len()`, not `body.len()`: the trailing ZWJ is for a request that is
                // NOTHING but `O:init`. `[Nirugu:control, O:init]` also has a one-unit body, but
                // the nirugu already supplies the joining context — it must not get the ZWJ.
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
            // One flag per side folds together the two ways a side can be non-initial/non-final:
            // a joiner control already sits there, or the chain simply does not start at `init` /
            // end at `fina` (so a ZWJ has to be invented). Either way the chain's positions are
            // computed as if one extra unit padded that side, which is exactly `padded_count`.
            let padded_left = joined_left || body[0].1 != Position::Init;
            let padded_right = joined_right || body[body.len() - 1].1 != Position::Fina;
            let padded_count = body.len() + usize::from(padded_left) + usize::from(padded_right);
            // `length = 1` because each record is one written unit occupying one slot; multi-unit
            // letters are the *encoder's* concern (`unit_partition`), not the caller's request.
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
    ///
    /// Every name — explicit or compact — must belong to *this shaper's normalize table*, not
    /// merely to [`WrittenUnit`]: `E` is a Todo unit, so an MNG shaper rejects it. That keeps the
    /// reported index identical to Python's, which defers the same check to
    /// `normalize_written_units`.
    ///
    /// # Errors
    ///
    /// - [`Error::NormalizeUnsupported`] — this locale has no bundled normalize table.
    /// - [`Error::InvalidUnitSpec`] — the spec contains whitespace or an empty `+` field, or the
    ///   compact string has more than one segmentation.
    /// - [`Error::UnknownWrittenUnit`] — a name is outside the table's vocabulary; on the compact
    ///   path an unsegmentable string is reported whole, at index 0.
    pub fn parse_written_units(&self, text: &str) -> Result<Vec<WrittenUnit>, Error> {
        let table = self.table()?;
        parse_unit_names(text, &table.sorted_vocabulary)?
            .into_iter()
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
                        unit: name.to_owned(),
                    })
            })
            .collect()
    }
}

impl Shaper {
    /// Every written unit this shaper's normalize table can encode, plus the structural
    /// controls (`Mvs`, `Nirugu`, `Zwj`), sorted by name.
    ///
    /// These are exactly the names [`Shaper::normalize_written_units`] accepts; the Python
    /// bindings use the list to validate their input with Python-formatted messages.
    ///
    /// # Errors
    ///
    /// [`Error::NormalizeUnsupported`] — this locale has no bundled normalize table.
    pub fn known_written_units(&self) -> Result<Vec<WrittenUnit>, Error> {
        let table = self.table()?;
        let mut units: Vec<WrittenUnit> = table.known_units.iter().copied().collect();
        units.sort_by_key(|unit| unit.as_str());
        Ok(units)
    }

    /// The HUD positioned inventory: every `(unit, position)` pair that
    /// [`Shaper::normalize_positioned_written_units`] accepts for a letter record (letter
    /// positions only — controls are not part of the inventory), sorted by unit name then
    /// position.
    ///
    /// # Errors
    ///
    /// [`Error::NormalizeUnsupported`] — this locale has no bundled normalize table.
    pub fn positioned_written_units(&self) -> Result<Vec<(WrittenUnit, Position)>, Error> {
        let table = self.table()?;
        let mut records: Vec<(WrittenUnit, Position)> =
            table.positioned_units.iter().copied().collect();
        records.sort_by_key(|(unit, position)| (unit.as_str(), position.as_str()));
        Ok(records)
    }
}

fn strip_one_newline(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .or_else(|| text.strip_suffix('\r'))
        .unwrap_or(text)
}

/// Python `_parse_written_units`: explicit `+` names, or a unique compact segmentation over
/// `vocabulary`. Returns borrowed names — slices of `text` on the `+` path, vocabulary entries on
/// the compact path — which the caller maps to units.
///
/// `vocabulary` is expected in Python's `(-len, name)` order (see
/// [`crate::normalize::NormalizeTable::sorted_vocabulary`]), but that order is *not* semantically load-bearing: the
/// per-offset count saturates at 2 and `choice` is cleared by any second match, so the final
/// `(count, choice)` pair is independent of iteration order. Sorting only lets the longest
/// candidates hit the `count == 2` early `break` sooner.
pub(crate) fn parse_unit_names<'v>(
    text: &'v str,
    vocabulary: &[&'v str],
) -> Result<Vec<&'v str>, Error> {
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
        return Ok(units);
    }
    // Right-to-left DP over byte offsets. Invariant: `parse_counts[i]` is the number of ways
    // `text[i..]` segments into vocabulary units, saturated at 2 — 0 = impossible, 1 = exactly
    // one segmentation, 2 = "two or more", i.e. ambiguous. The empty suffix has one (the empty)
    // segmentation, hence `parse_counts[n] = 1`. `choices[i]` is the unit that starts `text[i..]`
    // in its unique parse, plus where that unit ends.
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
            // `count` lands on exactly 1 only when a single match contributed, and that match
            // took the `count == 0 && parse_counts[end] == 1` branch — any second match would
            // both clear `choice` and push `count` to 2. So `choice` is `Some` here, which is
            // what the `expect` below relies on.
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
                units.push(unit);
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
        let error = parse_unit_names("AAA", &["AA", "A"]).unwrap_err();
        assert!(
            matches!(&error, Error::InvalidUnitSpec(m) if m.contains("ambiguous") && m.contains('+'))
        );
        assert_eq!(parse_unit_names("AA", &["AA", "A"]).unwrap_err(), error);
        assert_eq!(
            parse_unit_names("AAB", &["AA", "A", "B"]).unwrap_err(),
            error
        );
        assert_eq!(parse_unit_names("AAB", &["AA", "B"]).unwrap(), ["AA", "B"]);
    }

    /// The `(-len, name)` order is an optimisation, not semantics: any permutation of the same
    /// vocabulary yields the same result.
    #[test]
    fn vocabulary_order_does_not_change_the_parse() {
        // {AA, B} parses "AAB" uniquely; {A, AA} makes "AAA" ambiguous. Both hold in any order.
        for vocabulary in [["AA", "B"], ["B", "AA"]] {
            assert_eq!(parse_unit_names("AAB", &vocabulary).unwrap(), ["AA", "B"]);
        }
        for vocabulary in [["AA", "A"], ["A", "AA"]] {
            assert!(parse_unit_names("AAA", &vocabulary).is_err());
            assert_eq!(parse_unit_names("A", &vocabulary).unwrap(), ["A"]);
        }
    }

    #[test]
    fn multibyte_input_never_splits_a_char() {
        assert_eq!(
            parse_unit_names("A\u{1820}", &["A"]).unwrap_err(),
            Error::UnknownWrittenUnit {
                index: 0,
                unit: "A\u{1820}".to_owned()
            }
        );
    }

    #[test]
    fn trailing_transport_newline_is_stripped_once() {
        assert_eq!(parse_unit_names("A\r\n", &["A"]).unwrap(), ["A"]);
        assert_eq!(parse_unit_names("A\n", &["A"]).unwrap(), ["A"]);
        assert_eq!(parse_unit_names("\n", &["A"]).unwrap(), Vec::<&str>::new());
        assert!(parse_unit_names("A\n\n", &["A"]).is_err());
    }
}
