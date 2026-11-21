//! Written-unit input APIs — a port of `normalize_written_units`,
//! `normalize_positioned_written_units` and `_parse_written_units` from `mongol_norm/shaper.py`.

use crate::generated::enums::WrittenUnit;
use crate::normalize::is_joiner;
use crate::shaper::Shaper;
use crate::tables::{Position, UnitPosition};
use crate::Error;

/// One record of [`Shaper::canonical_positioned_units`] and
/// [`Shaper::normalize_positioned_written_units`]: a written unit with its authoritative HUD
/// inventory position (`Control` for `Mvs` / `Nirugu`).
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

/// Does `part` cursively join the chain next to it? Only a joiner control (`Nirugu`) does; a
/// neighbouring chain or break control (`Mvs`, `Zwnj`) never does.
fn is_joiner_part(part: &PositionedPart) -> bool {
    matches!(part, PositionedPart::Control(unit) if is_joiner(*unit))
}

/// Must an implicit `Zwnj` be inserted between two adjacent positioned records?
///
/// `Mvs` is already a cursive break (`!is_joiner(WrittenUnit::Mvs)`), so any boundary touching
/// `Mvs` needs no `Zwnj`. Between letters and/or joiner controls (`Nirugu`), a `Zwnj` is needed
/// whenever the left record's right edge is closed (`fina` / `isol`) or the right record's left
/// edge is closed (`init` / `isol`).
fn needs_zwnj_between(prev: PositionedWrittenUnit, next: PositionedWrittenUnit) -> bool {
    let prev_participates = prev.position != UnitPosition::Control || is_joiner(prev.unit);
    let next_participates = next.position != UnitPosition::Control || is_joiner(next.unit);
    if !prev_participates || !next_participates {
        return false;
    }
    let prev_closed_right = matches!(prev.position, UnitPosition::Fina | UnitPosition::Isol);
    let next_closed_left = matches!(next.position, UnitPosition::Init | UnitPosition::Isol);
    prev_closed_right || next_closed_left
}

impl Shaper {
    /// Encode an ordered written-unit sequence (e.g. the output of [`Shaper::shape`]) as
    /// canonical Unicode. Letter positions are inferred from order and the structural tokens;
    /// ZWJ is emitted only where `Zwj` is present in the request *and* changes the ink — the
    /// request goes through the same control canonicalization and duplicate unification as
    /// [`Shaper::shape`], so `[L, I, Zwj, Aa]` is encoded as `L G`, exactly like the text
    /// `ᠯᠢ\u{200D}ᠡ᠋`. The result is accepted only if it reshapes to that folded sequence.
    ///
    /// Redundant ZWNJs are removed and consecutive identical ZWJ/ZWNJ/MVS units are merged.
    /// Each segment between remaining breaks is encoded independently and must reshape to its
    /// canonical target even on its own. An empty sequence returns `""`.
    ///
    /// # Errors
    ///
    /// - [`Error::UnsupportedWrittenUnit`] — `units[index]` is outside the table's vocabulary.
    ///   The first offender is reported. Every [`WrittenUnit`] is in the bundled table, so this
    ///   is a guard rather than an expected outcome.
    /// - [`Error::NoCanonicalEncoding`] — the table covers every unit, but the sequence has no
    ///   encoding that reshapes back to it.
    pub fn normalize_written_units(&self, units: &[WrittenUnit]) -> Result<String, Error> {
        if units.is_empty() {
            return Ok(String::new());
        }
        let table = self.table();
        for (index, unit) in units.iter().enumerate() {
            if !table.known_units.contains(unit) {
                return Err(Error::UnsupportedWrittenUnit { index, unit: *unit });
            }
        }
        // Duplicate encodings and redundant controls are accepted on input — `Dd`, medial H/Hx
        // and structural controls are real units in the standard and callers' data — and folded
        // before encoding, exactly as `shape` folds them, so they get the same canonical text as
        // the sequence they render identically to.
        let units = self.canonical_units(units);
        let canonical = self.canonical_for_shape(&units)?;
        if canonical.is_empty() || self.shape(&canonical)? != units {
            return Err(Error::NoCanonicalEncoding);
        }
        Ok(canonical)
    }

    /// Encode explicit HUD-position records as canonical Unicode (the API `zvvnmod-utn57`
    /// uses). A complete multi-record chain runs `init…fina`; an incomplete edge gets an implicit
    /// ZWJ; a single `init` record is encoded bare where that bare letter reads back as the same
    /// record — every consonant, whose isolated form is its initial unit — and takes a trailing
    /// ZWJ where it would not: `A` and `I`, which the inventory also carries at `isol`, and `O`,
    /// which has no bare spelling; single `medi` / `fina` records get the joining context their
    /// position needs. Adjacent letter records whose positions mark a sub-chain boundary (after
    /// `fina` / `isol` or a lone `init`, or before a new `init` / `isol`), as well as a `Nirugu`
    /// beside a non-joining letter edge (after `fina` / `isol`, or before `init` / `isol`), are
    /// separated with an implicit `Zwnj`, which the canonical control pass keeps only where
    /// needed. Every `Aa:isol` record (chachlag) automatically receives an implicit
    /// leading `Mvs`; an explicit `Mvs:control` directly before `Aa:isol` is rejected with
    /// [`Error::RedundantChachlagMvs`], and an explicit `Mvs:control` at the end of `records` or
    /// followed by `control` / `medi` / `fina` is rejected with [`Error::ChainPositionMismatch`].
    /// `Mvs` and `Nirugu` require `Control`; explicit `Zwj` and `Zwnj` are rejected; at most
    /// [`MAX_POSITIONED_RECORDS`] records.
    ///
    /// # Validation order
    ///
    /// Checks run in a fixed order, and it is observable — callers such as `zvvnmod-utn57`
    /// dispatch on the variant, so the *first* failing check decides which one they see:
    ///
    /// 1. record limit — 2000 records containing a `Zwj` or `Zwnj` is [`Error::TooManyRecords`],
    ///    not [`Error::ExplicitZwj`] or [`Error::ExplicitZwnj`];
    /// 2. explicit `Zwj` or `Zwnj` anywhere in the request, reporting whichever comes first;
    /// 3. empty request — returns `""`;
    /// 4. per record, the control/inventory check (`Mvs` and `Nirugu` need `Control`;
    ///    every other `(unit, position)` must be in the HUD inventory), reporting the first offender;
    /// 5. per explicit `Mvs:control`, its right-hand record (`Aa:isol` raises
    ///    [`Error::RedundantChachlagMvs`]; end-of-sequence or `control` / `medi` / `fina` raises
    ///    [`Error::ChainPositionMismatch`]), reporting the first offender;
    /// 6. delegation to [`Shaper::normalize_written_units`].
    ///
    /// # Errors
    ///
    /// - [`Error::TooManyRecords`], [`Error::ExplicitZwj`], [`Error::ExplicitZwnj`],
    ///   [`Error::ControlRequiresControlPosition`], [`Error::UnsupportedPositionedUnit`],
    ///   [`Error::RedundantChachlagMvs`], [`Error::ChainPositionMismatch`] — as numbered above.
    /// - Anything [`Shaper::normalize_written_units`] returns. **Its indices point into the
    ///   expanded written-unit sequence — after implicit MVS, ZWJs and ZWNJs were inserted — not
    ///   into `records`.**
    pub fn normalize_positioned_written_units(
        &self,
        records: &[PositionedWrittenUnit],
    ) -> Result<String, Error> {
        let written = self.expand_positioned_written_units(records)?;
        self.normalize_written_units(&written)
    }

    /// Fold explicit HUD-position records into their canonical written-unit shape without
    /// encoding to Unicode or running a [`Shaper::shape`] round trip.
    ///
    /// Validates `records` and synthesizes implicit controls identically to
    /// [`Shaper::normalize_positioned_written_units`]:
    ///
    /// - every `Aa:isol` record (chachlag) receives an implicit leading `Mvs`, while an explicit
    ///   `Mvs:control` directly before `Aa:isol` raises [`Error::RedundantChachlagMvs`] and an
    ///   explicit `Mvs:control` at the end of `records` or followed by `control` / `medi` /
    ///   `fina` raises [`Error::ChainPositionMismatch`];
    /// - adjacent letter records whose positions mark a sub-chain boundary (after `fina` / `isol`
    ///   or a lone `init`, or before a new `init` / `isol`), as well as a `Nirugu` beside a
    ///   non-joining letter edge (after `fina` / `isol`, or before `init` / `isol`), receive an
    ///   implicit `Zwnj`;
    /// - incomplete `medi` / `fina` edges without an adjacent `Nirugu`, plus lone `A:init`,
    ///   `I:init` and `O:init` sub-chains, receive an implicit `Zwj`;
    /// - the expanded written-unit sequence is then folded with [`Shaper::canonical_units`]
    ///   (single-unit duplicate expansion, control canonicalization, and chain-tail contraction).
    ///
    /// Whenever [`Shaper::normalize_positioned_written_units`] succeeds on `records`, its output
    /// reshapes via [`Shaper::shape`] to the exact sequence returned here. An empty slice returns
    /// an empty `Vec`.
    ///
    /// # Validation order
    ///
    /// Identical to steps 1–5 of [`Shaper::normalize_positioned_written_units`]:
    ///
    /// 1. record limit ([`MAX_POSITIONED_RECORDS`]) → [`Error::TooManyRecords`];
    /// 2. explicit `Zwj` or `Zwnj` anywhere in `records` → [`Error::ExplicitZwj`] /
    ///    [`Error::ExplicitZwnj`];
    /// 3. empty request → `Ok(Vec::new())`;
    /// 4. per record, control position and HUD inventory membership →
    ///    [`Error::ControlRequiresControlPosition`] / [`Error::UnsupportedPositionedUnit`];
    /// 5. per explicit `Mvs:control`, its right-hand neighbor →
    ///    [`Error::RedundantChachlagMvs`] / [`Error::ChainPositionMismatch`].
    ///
    /// # Errors
    ///
    /// - [`Error::TooManyRecords`], [`Error::ExplicitZwj`], [`Error::ExplicitZwnj`],
    ///   [`Error::ControlRequiresControlPosition`], [`Error::UnsupportedPositionedUnit`],
    ///   [`Error::RedundantChachlagMvs`], [`Error::ChainPositionMismatch`] — as numbered above.
    pub fn canonical_positioned_units(
        &self,
        records: &[PositionedWrittenUnit],
    ) -> Result<Vec<WrittenUnit>, Error> {
        let written = self.expand_positioned_written_units(records)?;
        Ok(self.canonical_units(&written))
    }

    /// Validate `records` in observable steps 1–5 and expand implicit `Mvs`, `Zwnj` and `Zwj`
    /// controls before [`Shaper::canonical_units`] / [`Shaper::normalize_written_units`].
    fn expand_positioned_written_units(
        &self,
        records: &[PositionedWrittenUnit],
    ) -> Result<Vec<WrittenUnit>, Error> {
        if records.len() > MAX_POSITIONED_RECORDS {
            return Err(Error::TooManyRecords {
                max: MAX_POSITIONED_RECORDS,
            });
        }
        for record in records {
            match record.unit {
                WrittenUnit::Zwj => return Err(Error::ExplicitZwj),
                WrittenUnit::Zwnj => return Err(Error::ExplicitZwnj),
                _ => {}
            }
        }
        if records.is_empty() {
            return Ok(Vec::new());
        }
        let table = self.table();
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
        for (index, record) in records.iter().enumerate() {
            if record.unit != WrittenUnit::Mvs {
                continue;
            }
            match records.get(index + 1) {
                Some(PositionedWrittenUnit {
                    unit: WrittenUnit::Aa,
                    position: UnitPosition::Isol,
                }) => return Err(Error::RedundantChachlagMvs { index }),
                Some(PositionedWrittenUnit {
                    position: UnitPosition::Init | UnitPosition::Isol,
                    ..
                }) => {}
                None
                | Some(PositionedWrittenUnit {
                    position: UnitPosition::Control | UnitPosition::Medi | UnitPosition::Fina,
                    ..
                }) => return Err(Error::ChainPositionMismatch),
            }
        }

        let mut parts: Vec<PositionedPart> = Vec::new();
        let mut chain: Vec<(WrittenUnit, Position)> = Vec::new();
        let mut prev_record: Option<PositionedWrittenUnit> = None;
        for &record in records {
            // `Aa:isol` (chachlag) always owns a leading `Mvs`, which already breaks joining from
            // whatever preceded it; every other adjacent pair goes through `needs_zwnj_between`.
            if record.unit == WrittenUnit::Aa && record.position == UnitPosition::Isol {
                if !chain.is_empty() {
                    parts.push(PositionedPart::Chain(std::mem::take(&mut chain)));
                }
                parts.push(PositionedPart::Control(WrittenUnit::Mvs));
                chain.push((WrittenUnit::Aa, Position::Isol));
                prev_record = Some(record);
                continue;
            }
            if prev_record.is_some_and(|prev| needs_zwnj_between(prev, record)) {
                if !chain.is_empty() {
                    parts.push(PositionedPart::Chain(std::mem::take(&mut chain)));
                }
                parts.push(PositionedPart::Control(WrittenUnit::Zwnj));
            }
            match record.position.as_position() {
                None => {
                    if !chain.is_empty() {
                        parts.push(PositionedPart::Chain(std::mem::take(&mut chain)));
                    }
                    parts.push(PositionedPart::Control(record.unit));
                }
                Some(position) => chain.push((record.unit, position)),
            }
            prev_record = Some(record);
        }
        if !chain.is_empty() {
            parts.push(PositionedPart::Chain(chain));
        }

        // Because `needs_zwnj_between` splits whenever a letter closes on the right (`fina` /
        // `isol`) or opens on the left (`init` / `isol`), every resulting `Chain` is either a
        // single letter or a valid cursive run `[init|medi, medi*, medi|fina]`. Only open edges
        // without an adjacent `Nirugu` (plus lone `A:init` / `I:init` / `O:init`) need a `Zwj`.
        let mut written: Vec<WrittenUnit> = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            let body = match part {
                PositionedPart::Control(unit) => {
                    written.push(*unit);
                    continue;
                }
                PositionedPart::Chain(body) => body,
            };
            let joined_left = index > 0 && is_joiner_part(&parts[index - 1]);
            let joined_right = index + 1 < parts.len() && is_joiner_part(&parts[index + 1]);
            let (_, first_pos) = body[0];
            let &(last_unit, last_pos) = body.last().expect("letter sub-chain is non-empty");
            if !joined_left && matches!(first_pos, Position::Medi | Position::Fina) {
                written.push(WrittenUnit::Zwj);
            }
            written.extend(body.iter().map(|&(unit, _)| unit));
            let needs_right_zwj = last_pos == Position::Medi
                || (body.len() == 1
                    && !joined_left
                    && last_pos == Position::Init
                    && Self::lone_init_needs_zwj(last_unit));
            if !joined_right && needs_right_zwj {
                written.push(WrittenUnit::Zwj);
            }
        }
        Ok(written)
    }

    /// Does a lone `init` sub-chain (`[unit:init]` with no adjacent joiner) need a trailing ZWJ
    /// to keep its `init` shape?
    ///
    /// Bare, a lone letter is an isolated chain. That is already the `init` shape wherever
    /// `unit:init ≡ unit:isol` ([`crate::redundant_zwj::FINAL_ALONE`] — every single-unit
    /// consonant, plus `Cr`, whose `init` and `isol` both expand to `O O`) — so those stay bare.
    /// Two kinds of letter would read back as something else and take the joiner
    /// (Satsrag/meco-rust#45): one the inventory also carries at `isol` under the same bare
    /// spelling (`A`, `I`), and one with no bare spelling of its own (`O`, whose bare letter is
    /// `U:isol`).
    fn lone_init_needs_zwj(unit: WrittenUnit) -> bool {
        !crate::redundant_zwj::FINAL_ALONE.contains(&unit) && unit != WrittenUnit::Cr
    }

    /// Parse the CLI's written-unit spelling: explicit `+`-separated names (`B+Aa`) or a
    /// compact PascalCase string with exactly one segmentation over this shaper's known units
    /// (`BZwj`). One trailing newline is tolerated; an ambiguous compact string is rejected.
    ///
    /// Every name — explicit or compact — must belong to *this shaper's normalize table*, not
    /// merely to [`WrittenUnit`]. That keeps the reported index identical to Python's, which
    /// defers the same check to `normalize_written_units`.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidUnitSpec`] — the spec contains whitespace or an empty `+` field, or the
    ///   compact string has more than one segmentation.
    /// - [`Error::UnknownWrittenUnit`] — a name is outside the table's vocabulary; on the compact
    ///   path an unsegmentable string is reported whole, at index 0.
    pub fn parse_written_units(&self, text: &str) -> Result<Vec<WrittenUnit>, Error> {
        let table = self.table();
        parse_unit_names(text, &table.sorted_vocabulary)?
            .into_iter()
            .enumerate()
            .map(|(index, name)| {
                // Membership is tested against the TABLE's vocabulary, not merely "is this a
                // `WrittenUnit` variant": Python leaves every name unresolved here and lets
                // `normalize_written_units` reject the first one missing from `known_units`, so a
                // unit the enum knows but the table does not must fail at Python's index too.
                // — see shaper.py::normalize_written_units
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
    /// None: the `Result` remains from the locales without a normalize table the crate used to
    /// have.
    pub fn known_written_units(&self) -> Result<Vec<WrittenUnit>, Error> {
        let table = self.table();
        let mut units: Vec<WrittenUnit> = table.known_units.iter().copied().collect();
        units.sort_by_key(|unit| unit.as_str());
        Ok(units)
    }

    /// The HUD positioned inventory: every `(unit, position)` pair that
    /// [`Shaper::canonical_positioned_units`] and [`Shaper::normalize_positioned_written_units`]
    /// accept for a letter record (letter
    /// positions only — controls are not part of the inventory), sorted by unit name then
    /// position.
    ///
    /// # Errors
    ///
    /// None: the `Result` remains from the locales without a normalize table the crate used to
    /// have.
    pub fn positioned_written_units(&self) -> Result<Vec<(WrittenUnit, Position)>, Error> {
        let table = self.table();
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
    use crate::Locale;

    #[test]
    fn a_unit_outside_the_table_is_rejected_with_its_index() {
        // Every `WrittenUnit` is in the bundled table, so the guard is exercised on the empty
        // test table (which knows only the structural units).
        let shaper = Shaper::with_empty_normalize_table(Locale::Mng);
        let error = shaper
            .normalize_written_units(&[WrittenUnit::Mvs, WrittenUnit::B])
            .unwrap_err();
        assert_eq!(
            error,
            Error::UnsupportedWrittenUnit {
                index: 1,
                unit: WrittenUnit::B
            }
        );
        assert_eq!(error.to_string(), "written_units[1] is unknown: 'B'");
    }

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

    #[test]
    fn lone_init_needs_zwj_matches_inventory_and_bare_encoding_rule() {
        let shaper = Shaper::new(Locale::Mng);
        let table = shaper.table();
        for &(unit, position) in &table.positioned_units {
            if position != Position::Init {
                continue;
            }
            let expected = table.positioned_units.contains(&(unit, Position::Isol))
                || shaper.normalize_written_units(&[unit]).is_err();
            assert_eq!(Shaper::lone_init_needs_zwj(unit), expected, "{unit:?}");
        }
    }
}
