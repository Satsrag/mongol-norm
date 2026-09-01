//! Canonical normalization — a port of the normalize core of `mongol_norm/shaper.py`
//! (`_canonical_for_shape`, `_unit_encode_chain`, `_unit_partition`, `_apply_velar_fem`,
//! `_slot_position`, `_letter_position`, `normalize`, `normalize_text`).
//!
//! Within the table's domain `normalize` is a pure function of shape: each written unit is
//! encoded by a context-independent, FVS-pinned `(letter, fvs)` from the per-`(position, unit)`
//! table; every chain is verified by reshaping it in full context; a chain right after MVS takes
//! its standalone canonical so a suffix's spelling never depends on the MVS. There is no search
//! fallback — an uncovered shape is reported (strict) or echoed back unchanged.
//!
//! The remedy for a genuine table gap is to widen the table, not the runtime: the table is
//! generated offline by `scripts/gen_normalize_table.py` (Python), so extending coverage means
//! regenerating it there and rerunning `scripts/gen_rust_tables.py`.

use std::collections::{HashMap, HashSet};

use crate::generated::enums::WrittenUnit;
use crate::shaper::Shaper;
use crate::tables::{Fvs, NormalizeData, Position, UnitEntry};
use crate::unicode::is_mongolian_word_char;
use crate::Error;

/// Longest written-unit tuple a table key holds (the generator asserts `unit_enc_max_len <= 3`).
const MAX_KEY_LEN: usize = 3;

/// A fixed-capacity `(written units)` key — lookups never allocate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct UnitKey {
    len: u8,
    units: [WrittenUnit; MAX_KEY_LEN],
}

impl UnitKey {
    fn new(units: &[WrittenUnit]) -> UnitKey {
        debug_assert!(!units.is_empty() && units.len() <= MAX_KEY_LEN);
        // The padding value is arbitrary but deterministic per slice, and `len` disambiguates
        // shorter keys from longer ones, so `Hash` and `Eq` stay consistent.
        let mut padded = [units[0]; MAX_KEY_LEN];
        padded[..units.len()].copy_from_slice(units);
        UnitKey {
            len: units.len() as u8,
            units: padded,
        }
    }
}

/// `(letter code point, FVS)`.
type Encoding = (u32, Option<Fvs>);

/// The runtime form of `MNG.normalize.json`.
pub(crate) struct NormalizeTable {
    pub canonical_version: &'static str,
    max_len: usize,
    table: HashMap<(Position, UnitKey), Encoding>,
    feminine: HashMap<(Position, UnitKey), Encoding>,
    velar_fem_units: HashSet<WrittenUnit>,
    masculine_cps: HashSet<u32>,
    /// Every unit that occurs in a table key, plus the three structural tokens — the vocabulary
    /// of `normalize_written_units` and `parse_written_units`.
    pub known_units: HashSet<WrittenUnit>,
    /// The authoritative HUD `(unit, position)` inventory.
    pub positioned_units: HashSet<(WrittenUnit, Position)>,
}

fn index_entries(entries: &'static [UnitEntry]) -> HashMap<(Position, UnitKey), Encoding> {
    entries
        .iter()
        .map(|entry| {
            (
                (entry.position, UnitKey::new(entry.units)),
                (entry.cp, entry.fvs),
            )
        })
        .collect()
}

impl NormalizeTable {
    pub fn new(data: &'static NormalizeData) -> NormalizeTable {
        let mut known_units: HashSet<WrittenUnit> = data
            .unit_table
            .iter()
            .flat_map(|entry| entry.units.iter().copied())
            .collect();
        known_units.extend([WrittenUnit::Mvs, WrittenUnit::Nirugu, WrittenUnit::Zwj]);
        NormalizeTable {
            canonical_version: data.canonical_version,
            max_len: data.unit_enc_max_len,
            table: index_entries(data.unit_table),
            feminine: index_entries(data.velar_fem),
            velar_fem_units: data.velar_fem_units.iter().copied().collect(),
            masculine_cps: data.masc_to_fem.iter().map(|(masc, _)| *masc).collect(),
            known_units,
            positioned_units: data.positioned_units.iter().copied().collect(),
        }
    }

    /// Python's monkeypatched empty table (`_unit_enc = {}`, `_unit_enc_max_len = 1`): no
    /// encodings at all, so every chain falls back.
    #[cfg(test)]
    pub fn empty(canonical_version: &'static str) -> NormalizeTable {
        NormalizeTable {
            canonical_version,
            max_len: 1,
            table: HashMap::new(),
            feminine: HashMap::new(),
            velar_fem_units: HashSet::new(),
            masculine_cps: HashSet::new(),
            known_units: [WrittenUnit::Mvs, WrittenUnit::Nirugu, WrittenUnit::Zwj]
                .into_iter()
                .collect(),
            positioned_units: HashSet::new(),
        }
    }

    fn get(&self, position: Position, units: &[WrittenUnit]) -> Option<Encoding> {
        self.table.get(&(position, UnitKey::new(units))).copied()
    }

    fn get_feminine(&self, position: Position, units: &[WrittenUnit]) -> Option<Encoding> {
        self.feminine.get(&(position, UnitKey::new(units))).copied()
    }
}

/// The character a structural shape token encodes to, verbatim (Python `_STRUCTURAL_CHARS`).
pub(crate) fn structural_char(unit: WrittenUnit) -> Option<char> {
    match unit {
        WrittenUnit::Mvs => Some('\u{180E}'),
        WrittenUnit::Nirugu => Some('\u{180A}'),
        WrittenUnit::Zwj => Some('\u{200D}'),
        _ => None,
    }
}

/// Joiners force cursive connection on the adjacent letter (Python `_JOINER_TOKENS`).
pub(crate) fn is_joiner(unit: WrittenUnit) -> bool {
    matches!(unit, WrittenUnit::Nirugu | WrittenUnit::Zwj)
}

fn structural_text(units: &[WrittenUnit]) -> String {
    units
        .iter()
        .map(|unit| structural_char(*unit).expect("structural token"))
        .collect()
}

enum Part {
    Structural(WrittenUnit),
    Chain(Vec<WrittenUnit>),
}

/// Split a shape at its structural tokens (which are copied through verbatim).
fn split_parts(shape: &[WrittenUnit]) -> Vec<Part> {
    let mut parts = Vec::new();
    let mut chain = Vec::new();
    for &unit in shape {
        if unit.is_structural() {
            if !chain.is_empty() {
                parts.push(Part::Chain(std::mem::take(&mut chain)));
            }
            parts.push(Part::Structural(unit));
        } else {
            chain.push(unit);
        }
    }
    if !chain.is_empty() {
        parts.push(Part::Chain(chain));
    }
    parts
}

/// Position of a letter spanning units `[start, start + length)` in a chain of `unit_count` units.
pub(crate) fn slot_position(start: usize, length: usize, unit_count: usize) -> Position {
    if start == 0 && start + length == unit_count {
        Position::Isol
    } else if start == 0 {
        Position::Init
    } else if start + length == unit_count {
        Position::Fina
    } else {
        Position::Medi
    }
}

/// Position of the `letter_index`-th letter out of `total` letters.
fn letter_position(letter_index: usize, total: usize) -> Position {
    if total == 1 {
        Position::Isol
    } else if letter_index == 0 {
        Position::Init
    } else if letter_index == total - 1 {
        Position::Fina
    } else {
        Position::Medi
    }
}

/// Python `_unit_partition`: single deterministic local partition + encode pass. At each index
/// take the single unit if the table has it, else the longest multi-unit entry. Joiners on either
/// side shift the positions as if one extra unit padded that side.
fn unit_partition(
    table: &NormalizeTable,
    chain: &[WrittenUnit],
    joined_left: bool,
    joined_right: bool,
) -> Option<String> {
    let unit_count = chain.len();
    let pad_left = usize::from(joined_left);
    let pad_right = usize::from(joined_right);
    let padded_count = unit_count + pad_left + pad_right;
    let mut letters: Vec<Encoding> = Vec::new();
    let mut unit_at: Vec<Option<WrittenUnit>> = Vec::new();
    let mut index = 0;
    while index < unit_count {
        let span = table.max_len.min(unit_count - index);
        let mut hit: Option<(Encoding, usize)> = None;
        // 1) single unit (preferred — clean output)
        let position = slot_position(index + pad_left, 1, padded_count);
        if let Some(encoding) = table.get(position, &chain[index..index + 1]) {
            hit = Some((encoding, 1));
        }
        // 2) else the longest available multi-unit entry (last resort)
        if hit.is_none() {
            for length in (2..=span).rev() {
                let position = slot_position(index + pad_left, length, padded_count);
                if let Some(encoding) = table.get(position, &chain[index..index + length]) {
                    hit = Some((encoding, length));
                    break;
                }
            }
        }
        let (encoding, length) = hit?;
        letters.push(encoding);
        unit_at.push((length == 1).then_some(chain[index]));
        index += length;
    }
    apply_velar_fem(table, &mut letters, &unit_at, pad_left, pad_right);
    let mut text = String::new();
    for (cp, fvs) in letters {
        text.push(char::from_u32(cp).expect("table code points are scalar values"));
        if let Some(fvs) = fvs {
            text.push(fvs.as_char());
        }
    }
    Some(text)
}

/// Python `_apply_velar_fem`: switch the vowel forward-coupled to each init/medi `G`/`Gx` velar to
/// its feminine letter (only masculine a/o/u flip; backward coupling is deliberately skipped for
/// prefix-stability).
fn apply_velar_fem(
    table: &NormalizeTable,
    letters: &mut [Encoding],
    unit_at: &[Option<WrittenUnit>],
    pad_left: usize,
    pad_right: usize,
) {
    // `letters` and `unit_at` are pushed in lockstep by `unit_partition`, so they have equal
    // length; iterating `unit_at` (a separate slice, so no borrow conflict with `letters`)
    // yields exactly the indices of `letters`.
    let total = letters.len();
    let padded_total = total + pad_left + pad_right;
    for (letter_index, unit) in unit_at.iter().enumerate() {
        let Some(unit) = *unit else {
            continue;
        };
        if !table.velar_fem_units.contains(&unit) {
            continue;
        }
        // FORWARD coupling only (init/medi velar → following vowel). Backward coupling (fina
        // velar → preceding vowel) is deliberately skipped: a fina velar becomes medi when a
        // suffix is appended, flipping its coupling direction, which would make the shared-prefix
        // vowel diverge between word B and word A. The FVS-pinned velar renders `G` regardless,
        // so a masculine preceding vowel still round-trips — that one's prettiness is traded for
        // prefix-stability. — see shaper.py::_apply_velar_fem
        let position = letter_position(letter_index + pad_left, padded_total);
        if !matches!(position, Position::Init | Position::Medi) {
            continue;
        }
        let target_index = letter_index + 1;
        if target_index >= total {
            continue;
        }
        let Some(target_unit) = unit_at[target_index] else {
            continue; // multi-unit coupled letter — leave it
        };
        let (cp, _) = letters[target_index];
        if !table.masculine_cps.contains(&cp) {
            continue; // only flip a currently-masculine vowel
        }
        let target_position = letter_position(target_index + pad_left, padded_total);
        let Some(feminine) = table.get_feminine(target_position, &[target_unit]) else {
            continue; // no round-trip-safe feminine form → leave masculine
        };
        letters[target_index] = feminine;
    }
}

impl Shaper {
    /// The normalize table, or [`Error::NormalizeUnsupported`] for locales without one.
    pub(crate) fn table(&self) -> Result<&NormalizeTable, Error> {
        self.normalize.as_ref().ok_or(Error::NormalizeUnsupported {
            locale: self.locale(),
        })
    }

    /// Version of the canonical Unicode selection policy (`"mng-canonical/1"` for MNG; `None`
    /// for locales without a normalize table). Persist it next to stored normalized keys.
    pub fn canonical_version(&self) -> Option<&'static str> {
        self.normalize.as_ref().map(|table| table.canonical_version)
    }

    /// Python `_canonical_for_shape`: encode a full shape (chains right-to-left so each chain is
    /// verified with the already-encoded suffix; structural tokens copied verbatim).
    ///
    /// Right-to-left with the encoded suffix in hand is necessary because rules interact across
    /// MVS: a masculine vowel after an MVS can propagate backward through the MVS and mark a g/h
    /// in the previous chain, changing its rendering between `G` and `H`. Per-chain verification
    /// with only the adjacent MVS is insufficient. — see shaper.py::_canonical_for_shape
    ///
    /// The table is fetched only when a chain has to be encoded (Python calls `_build_unit_enc`
    /// lazily), so a structural-only shape — e.g. a lone nirugu — is copied through even for
    /// locales without a normalize table.
    pub(crate) fn canonical_for_shape(&self, shape: &[WrittenUnit]) -> Result<String, Error> {
        let parts = split_parts(shape);
        // `suffix_text` accumulates the whole result: each part is prepended as it is encoded, so
        // after the last (leftmost) part it *is* the canonical text. (Python keeps a parallel
        // `encoded` list and joins it at the end — this deviates from that structure on purpose.)
        let mut suffix_text = String::new();
        let mut suffix_target: Vec<WrittenUnit> = Vec::new();
        for index in (0..parts.len()).rev() {
            match &parts[index] {
                Part::Structural(unit) => {
                    let text = structural_char(*unit)
                        .expect("structural token")
                        .to_string();
                    suffix_text.insert_str(0, &text);
                    suffix_target.insert(0, *unit);
                }
                Part::Chain(body) => {
                    let table = self.table()?;
                    // Context = the full run of structural tokens right before this chain, not
                    // just the adjacent one: an MVS behind a nirugu still matters, because
                    // chachlag looks through nirugu. — see shaper.py::_canonical_for_shape
                    let mut prefix_tokens: Vec<WrittenUnit> = Vec::new();
                    let mut scan = index;
                    while scan > 0 {
                        scan -= 1;
                        match &parts[scan] {
                            Part::Structural(unit) => prefix_tokens.insert(0, *unit),
                            Part::Chain(_) => break,
                        }
                    }
                    let mut chain_canonical: Option<String> = None;
                    // A chain directly after MVS is a suffix particle: encode it STANDALONE (drop
                    // the MVS, normalize, re-attach). Exception: chachlag `Aa` is bare `a`.
                    if prefix_tokens.last() == Some(&WrittenUnit::Mvs) {
                        let candidate = if body.as_slice() == [WrittenUnit::Aa] {
                            String::from('\u{1820}')
                        } else {
                            self.encode_chain_canonical(table, body, &[], "", &[])?
                        };
                        if !candidate.is_empty() {
                            let prefix_text = structural_text(&prefix_tokens);
                            let mut want = prefix_tokens.clone();
                            want.extend_from_slice(body);
                            want.extend_from_slice(&suffix_target);
                            if self.shape(&format!("{prefix_text}{candidate}{suffix_text}"))?
                                == want
                            {
                                chain_canonical = Some(candidate);
                            }
                        }
                    }
                    let chain_canonical = match chain_canonical {
                        Some(text) => text,
                        None => self.encode_chain_canonical(
                            table,
                            body,
                            &prefix_tokens,
                            &suffix_text,
                            &suffix_target,
                        )?,
                    };
                    suffix_text.insert_str(0, &chain_canonical);
                    let mut target = body.clone();
                    target.extend_from_slice(&suffix_target);
                    suffix_target = target;
                }
            }
        }
        Ok(suffix_text)
    }

    /// Python `_encode_chain_canonical` / `_compute_chain_canonical`: the table encoding of one
    /// chain in its structural context, or `""` on a genuine table gap.
    fn encode_chain_canonical(
        &self,
        table: &NormalizeTable,
        chain: &[WrittenUnit],
        prefix_tokens: &[WrittenUnit],
        suffix_text: &str,
        suffix_target: &[WrittenUnit],
    ) -> Result<String, Error> {
        Ok(self
            .unit_encode_chain(table, chain, prefix_tokens, suffix_text, suffix_target)?
            .unwrap_or_default())
    }

    /// Python `_unit_encode_chain`: partition + encode, then verify in FULL context (the
    /// structural prefix run and the already-encoded following chains).
    fn unit_encode_chain(
        &self,
        table: &NormalizeTable,
        chain: &[WrittenUnit],
        prefix_tokens: &[WrittenUnit],
        suffix_text: &str,
        suffix_target: &[WrittenUnit],
    ) -> Result<Option<String>, Error> {
        let joined_left = prefix_tokens.last().is_some_and(|unit| is_joiner(*unit));
        let joined_right = suffix_target.first().is_some_and(|unit| is_joiner(*unit));
        let Some(text) = unit_partition(table, chain, joined_left, joined_right) else {
            return Ok(None);
        };
        let prefix_text = structural_text(prefix_tokens);
        // `verify_target` MUST include `suffix_target`: without it the non-last chains of a
        // multi-chain word never verify, and every one of them falls back.
        // — see shaper.py::_unit_encode_chain
        let mut verify_target = prefix_tokens.to_vec();
        verify_target.extend_from_slice(chain);
        verify_target.extend_from_slice(suffix_target);
        if self.shape(&format!("{prefix_text}{text}{suffix_text}"))? == verify_target {
            Ok(Some(text))
        } else {
            Ok(None)
        }
    }

    fn normalize_impl(&self, text: &str, strict: bool) -> Result<String, Error> {
        if text.is_empty() {
            return Ok(String::new());
        }
        let target = self.shape(text)?;
        if target.is_empty() {
            // Only FVS marks, no letter — canonical is the empty string. (Joiners are *not*
            // dropped here: a lone nirugu/ZWJ shapes to a structural token and round-trips.)
            return Ok(String::new());
        }
        let canonical = self.canonical_for_shape(&target)?;
        if canonical.is_empty() || self.shape(&canonical)? != target {
            if strict {
                return Err(Error::NormalizationFallback {
                    text: text.to_owned(),
                    written_units: target,
                });
            }
            return Ok(text.to_owned());
        }
        Ok(canonical)
    }

    /// Canonical, FVS-pinned encoding of one Mongolian word: within the normalize table's domain,
    /// `shape(x) == shape(y)` ⟹ `normalize(x) == normalize(y)`, and
    /// `shape(normalize(x)) == shape(x)`.
    ///
    /// Strict (the Python default): an uncovered shape is [`Error::NormalizationFallback`].
    /// Errors with [`Error::NonMongolianChar`] on mixed-script input — see
    /// [`Shaper::normalize_text`].
    pub fn normalize(&self, text: &str) -> Result<String, Error> {
        self.normalize_impl(text, true)
    }

    /// Like [`Shaper::normalize`], but an uncovered shape returns the input unchanged
    /// (Python `strict=False`).
    pub fn normalize_allow_fallback(&self, text: &str) -> Result<String, Error> {
        self.normalize_impl(text, false)
    }

    fn normalize_text_impl(&self, text: &str, strict: bool) -> Result<String, Error> {
        if text.is_empty() {
            return Ok(String::new());
        }
        let mut out = String::with_capacity(text.len());
        let mut run = String::new();
        let mut run_is_mongolian: Option<bool> = None;
        for ch in text.chars() {
            let is_mongolian = is_mongolian_word_char(ch);
            match run_is_mongolian {
                Some(current) if current != is_mongolian => {
                    self.flush_run(&mut out, &run, current, strict)?;
                    run.clear();
                    run_is_mongolian = Some(is_mongolian);
                }
                Some(_) => {}
                None => run_is_mongolian = Some(is_mongolian),
            }
            run.push(ch);
        }
        if let Some(current) = run_is_mongolian {
            self.flush_run(&mut out, &run, current, strict)?;
        }
        Ok(out)
    }

    fn flush_run(
        &self,
        out: &mut String,
        run: &str,
        is_mongolian: bool,
        strict: bool,
    ) -> Result<(), Error> {
        if is_mongolian {
            out.push_str(&self.normalize_impl(run, strict)?);
        } else {
            out.push_str(run);
        }
        Ok(())
    }

    /// Normalize free-form text: every Mongolian word run is normalized independently, everything
    /// else (spaces, punctuation, Latin, …) is copied verbatim. Strict like [`Shaper::normalize`].
    pub fn normalize_text(&self, text: &str) -> Result<String, Error> {
        self.normalize_text_impl(text, true)
    }

    /// Like [`Shaper::normalize_text`], but an uncovered word is preserved unchanged.
    pub fn normalize_text_allow_fallback(&self, text: &str) -> Result<String, Error> {
        self.normalize_text_impl(text, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Locale;

    const SAIN: &str = "\u{1830}\u{1820}\u{1822}\u{1828}";

    #[test]
    fn strict_mode_raises_when_canonicalization_falls_back() {
        let shaper = Shaper::with_empty_normalize_table(Locale::Mng);
        let error = shaper.normalize(SAIN).unwrap_err();
        assert_eq!(
            error,
            Error::NormalizationFallback {
                text: SAIN.to_owned(),
                written_units: vec![
                    WrittenUnit::S,
                    WrittenUnit::A,
                    WrittenUnit::I,
                    WrittenUnit::I,
                    WrittenUnit::A
                ],
            }
        );
        assert_eq!(
            error.to_string(),
            "normalization fallback: no canonical encoding for written units S+A+I+I+A"
        );
    }

    #[test]
    fn allow_fallback_preserves_input_when_canonicalization_falls_back() {
        let shaper = Shaper::with_empty_normalize_table(Locale::Mng);
        assert_eq!(shaper.normalize_allow_fallback(SAIN).unwrap(), SAIN);
    }

    #[test]
    fn strict_mode_reports_a_fallback_inside_mixed_text() {
        let shaper = Shaper::with_empty_normalize_table(Locale::Mng);
        let text = format!("Hello {SAIN} world");
        assert!(matches!(
            shaper.normalize_text(&text),
            Err(Error::NormalizationFallback { .. })
        ));
    }

    #[test]
    fn allow_fallback_preserves_a_fallback_inside_mixed_text() {
        let shaper = Shaper::with_empty_normalize_table(Locale::Mng);
        let text = format!("Hello {SAIN} world");
        assert_eq!(shaper.normalize_text_allow_fallback(&text).unwrap(), text);
    }

    #[test]
    fn locales_without_a_table_reject_normalization_of_letters() {
        let shaper = Shaper::new(Locale::Tod);
        assert_eq!(shaper.canonical_version(), None);
        assert_eq!(shaper.normalize("").unwrap(), "");
        assert_eq!(shaper.normalize("\u{180B}").unwrap(), ""); // FVS only: empty shape short-circuits
        assert_eq!(shaper.normalize("\u{180A}").unwrap(), "\u{180A}"); // structural-only shape needs no table (Python parity)
        assert_eq!(
            shaper.normalize("\u{1820}"),
            Err(Error::NormalizeUnsupported {
                locale: Locale::Tod
            })
        );
        assert_eq!(
            shaper.normalize_written_units(&[WrittenUnit::Mvs]),
            Err(Error::NormalizeUnsupported {
                locale: Locale::Tod
            })
        );
        assert_eq!(
            Shaper::new(Locale::Mng).canonical_version(),
            Some("mng-canonical/1")
        );
    }

    #[test]
    fn positions_of_partition_slots_and_letters() {
        assert_eq!(slot_position(0, 1, 1), Position::Isol);
        assert_eq!(slot_position(0, 2, 2), Position::Isol);
        assert_eq!(slot_position(0, 1, 3), Position::Init);
        assert_eq!(slot_position(1, 1, 3), Position::Medi);
        assert_eq!(slot_position(1, 2, 3), Position::Fina);
        assert_eq!(letter_position(0, 1), Position::Isol);
        assert_eq!(letter_position(0, 2), Position::Init);
        assert_eq!(letter_position(1, 2), Position::Fina);
        assert_eq!(letter_position(1, 3), Position::Medi);
        assert_eq!(
            UnitKey::new(&[WrittenUnit::A]),
            UnitKey::new(&[WrittenUnit::A])
        );
        assert_ne!(
            UnitKey::new(&[WrittenUnit::A]),
            UnitKey::new(&[WrittenUnit::A, WrittenUnit::A])
        );
    }
}
