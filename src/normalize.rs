//! Canonical normalization: `normalize` maps every encoding of a shape to one Unicode string.
//!
//! The encoding itself is the online encoder of [`crate::encoder`] (policy `mng-canonical/3`):
//! within the tables' domain it is a pure function of the shape, round-trips
//! (`shape(normalize(x)) == shape(x)`), and is prefix-stable by construction. This module holds
//! the tables' runtime indexes and the public entry points; an uncovered shape is reported
//! (strict) or echoed back unchanged.
//!
//! The tables are generated offline by `examples/gen_normalize_table` (JSON in
//! `python/mongol_norm/data/MNG.normalize.json`, compiled by `python/scripts/gen_rust_tables.py`).

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

use crate::encoder::{self, DEAD, NONE};
use crate::generated::enums::WrittenUnit;
use crate::shaper::Shaper;
use crate::tables::{FinalValidity, NormalizeData, Position};
use crate::unicode::is_mongolian_word_char;
use crate::Error;

/// Longest written-unit sequence one letter renders.
const MAX_KEY_LEN: usize = 3;

/// A fixed-capacity `(written units)` key — lookups never allocate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct UnitKey {
    len: u8,
    units: [WrittenUnit; MAX_KEY_LEN],
}

impl UnitKey {
    fn new(units: &[WrittenUnit]) -> Option<UnitKey> {
        if units.is_empty() || units.len() > MAX_KEY_LEN {
            return None;
        }
        // The padding value is arbitrary but deterministic per slice, and `len` disambiguates
        // shorter keys from longer ones, so `Hash` and `Eq` stay consistent.
        let mut padded = [units[0]; MAX_KEY_LEN];
        padded[..units.len()].copy_from_slice(units);
        Some(UnitKey {
            len: units.len() as u8,
            units: padded,
        })
    }
}

/// A small multiplicative hasher for the encoder's integer keys (the standard SipHash would
/// dominate the lookup cost).
#[derive(Default)]
struct KeyHasher(u64);

impl Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(u64::from(byte));
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x517c_c1b7_2722_0a95);
    }

    fn write_u8(&mut self, value: u8) {
        self.write_u64(u64::from(value));
    }

    fn write_u16(&mut self, value: u16) {
        self.write_u64(u64::from(value));
    }

    fn write_u32(&mut self, value: u32) {
        self.write_u64(u64::from(value));
    }

    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }
}

type KeyMap<K, V> = HashMap<K, V, BuildHasherDefault<KeyHasher>>;

/// A normalize table with nothing in it: every shape with a letter is uncovered.
#[cfg(any(test, feature = "testing"))]
static EMPTY: NormalizeData = NormalizeData {
    canonical_version: "",
    mask_sets: &[],
    candidates: &[],
    finals: &[],
    promises: [&[], &[], &[], &[], &[]],
    particle_nodes: &[(u16::MAX, 0), (u16::MAX, 0)],
    known_units: &[],
    positioned_units: &[],
};

/// The runtime form of `MNG.normalize.json`: the generated data plus lookup indexes.
pub(crate) struct NormalizeTable {
    pub canonical_version: &'static str,
    pub(crate) data: &'static NormalizeData,
    /// `(position, units)` → the candidates rendering them.
    options: KeyMap<(Position, UnitKey), OptionGroup>,
    /// `(position, units)` → which options can end the word.
    finals: KeyMap<(Position, UnitKey), &'static FinalValidity>,
    /// `(node, letter)` → child node of the particle-key trie.
    particle_children: KeyMap<(u16, u32), u16>,
    /// Promise `p` (1..=5) allows letter `cp`: bit `cp - 0x1820` of `promise_letters[p]`.
    promise_letters: [u64; 6],
    /// Promise `p` can be kept before unit `u`: wherever a letter renders `u` alone at medi or
    /// fina, a letter of the class does too.
    promise_units: [u128; 6],
    /// Every unit the locale's letters render, plus the three structural tokens — the vocabulary
    /// of `normalize_written_units` and `parse_written_units`.
    pub known_units: HashSet<WrittenUnit>,
    /// `known_units` as names, in Python's `(-len, name)` order — the compact-segmentation
    /// vocabulary, built once here so `parse_written_units` never sorts per call.
    pub sorted_vocabulary: Vec<&'static str>,
    /// The authoritative HUD `(unit, position)` inventory.
    pub positioned_units: HashSet<(WrittenUnit, Position)>,
}

/// The candidates of one `(position, units)` group.
pub(crate) struct OptionGroup {
    /// Candidate indices in table order (bit `i` of a final-validity mask is `ids[i]`).
    pub ids: Vec<u16>,
    /// Preference orders (indices into `ids`), see [`preference`], indexed by
    /// `feminine + 2 * after_vowel`: `[plain, feminine, after a vowel, both]`.
    pub orders: [Vec<u8>; 4],
}

/// Equal-cost preference among letters. Always: code point order, `g` before `h` (ᠭᠡᠷ, not ᠬᠡᠷ).
/// `feminine` (the last vowel was `e oe ue ee`): `e oe ue` just before their partners `a o u`.
/// `after_vowel` (the letter ends its chain right after a vowel): `n` first for a final `A` — a
/// vowel after a vowel is foreign to the language, a final `n` is common.
pub(crate) fn preference(
    cp: u32,
    units: &[WrittenUnit],
    feminine: bool,
    after_vowel: bool,
) -> (bool, u32) {
    let rank = |cp: u32| if cp == 0x182D { 0x182C * 2 - 1 } else { cp * 2 };
    let n_first = after_vowel && cp == 0x1828 && units == [WrittenUnit::A];
    let rank = match cp {
        0x1821 if feminine => rank(0x1820) - 1,
        0x1825 if feminine => rank(0x1823) - 1,
        0x1826 if feminine => rank(0x1824) - 1,
        _ => rank(cp),
    };
    (!n_first, rank)
}

/// The unit names of `units` in Python's `sorted(known, key=lambda u: (-len(u), u))` order.
fn sorted_vocabulary(units: &HashSet<WrittenUnit>) -> Vec<&'static str> {
    let mut names: Vec<&'static str> = units.iter().map(|unit| unit.as_str()).collect();
    names.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    names
}

impl NormalizeTable {
    pub fn new(data: &'static NormalizeData) -> NormalizeTable {
        NormalizeTable::with_version(data, data.canonical_version)
    }

    fn with_version(
        data: &'static NormalizeData,
        canonical_version: &'static str,
    ) -> NormalizeTable {
        let mut groups: KeyMap<(Position, UnitKey), Vec<u16>> = KeyMap::default();
        for (id, cand) in data.candidates.iter().enumerate() {
            let key = UnitKey::new(cand.units).expect("candidate units fit a key");
            groups
                .entry((cand.position, key))
                .or_default()
                .push(id as u16);
        }
        let options = groups
            .into_iter()
            .map(|(key, ids)| {
                let orders = [(false, false), (true, false), (false, true), (true, true)].map(
                    |(feminine, after_vowel)| {
                        let mut order: Vec<u8> = (0..ids.len() as u8).collect();
                        order.sort_by_key(|&i| {
                            let cand = &data.candidates[ids[i as usize] as usize];
                            let (not_n, rank) =
                                preference(cand.cp, cand.units, feminine, after_vowel);
                            (cand.fvs.is_some(), not_n, rank, cand.fvs)
                        });
                        order
                    },
                );
                (key, OptionGroup { ids, orders })
            })
            .collect();
        let finals = data
            .finals
            .iter()
            .map(|entry| {
                let key = UnitKey::new(entry.units).expect("final units fit a key");
                ((entry.position, key), &entry.valid)
            })
            .collect();
        let particle_children = data
            .particle_nodes
            .iter()
            .enumerate()
            .filter(|(_, (parent, _))| *parent != NONE)
            .map(|(node, (parent, cp))| ((*parent, *cp), node as u16))
            .collect();
        let mut promise_letters = [u64::MAX; 6];
        for (index, letters) in data.promises.iter().enumerate() {
            promise_letters[index + 1] = letters
                .iter()
                .fold(0, |bits, cp| bits | 1u64 << (cp - 0x1820));
        }
        // A promise is kept when the next letter can render the next unit wherever it may stand:
        // medial if more follows, final otherwise (positions the unit never takes do not count).
        let mut promise_units = [u128::MAX; 6];
        for (promise, bits) in promise_units.iter_mut().enumerate().skip(1) {
            *bits = 0;
            for unit in WrittenUnit::ALL {
                let renders = |position: Position, class: bool| {
                    data.candidates.iter().any(|cand| {
                        cand.position == position
                            && cand.units == [unit]
                            && (!class
                                || promise_letters[promise] & (1u64 << (cand.cp - 0x1820)) != 0)
                    })
                };
                let positions = [Position::Medi, Position::Fina];
                let keeps = positions.iter().any(|&p| renders(p, false))
                    && positions
                        .iter()
                        .all(|&p| !renders(p, false) || renders(p, true));
                if keeps {
                    *bits |= 1u128 << (unit as u32);
                }
            }
        }
        let mut known_units: HashSet<WrittenUnit> = data.known_units.iter().copied().collect();
        known_units.extend([WrittenUnit::Mvs, WrittenUnit::Nirugu, WrittenUnit::Zwj]);
        let sorted_vocabulary = sorted_vocabulary(&known_units);
        NormalizeTable {
            canonical_version,
            data,
            options,
            finals,
            particle_children,
            promise_letters,
            promise_units,
            known_units,
            sorted_vocabulary,
            positioned_units: data.positioned_units.iter().copied().collect(),
        }
    }

    /// Python's monkeypatched empty table: no encodings at all, so every shape with a letter
    /// falls back.
    #[cfg(any(test, feature = "testing"))]
    pub fn empty(canonical_version: &'static str) -> NormalizeTable {
        NormalizeTable::with_version(&EMPTY, canonical_version)
    }

    /// The candidates rendering exactly `units` at `position`.
    pub(crate) fn options(
        &self,
        position: Position,
        units: &[WrittenUnit],
    ) -> Option<&OptionGroup> {
        UnitKey::new(units).and_then(|key| self.options.get(&(position, key)))
    }

    pub(crate) fn final_validity(
        &self,
        position: Position,
        units: &[WrittenUnit],
    ) -> Option<&'static FinalValidity> {
        UnitKey::new(units).and_then(|key| self.finals.get(&(position, key)).copied())
    }

    pub(crate) fn particle_child(&self, node: u16, cp: u32) -> u16 {
        if node == DEAD {
            return DEAD;
        }
        self.particle_children
            .get(&(node, cp))
            .copied()
            .unwrap_or(DEAD)
    }

    /// Does letter `cp` satisfy promise `promise` (`0` = none)?
    pub(crate) fn allows(&self, promise: u8, cp: u32) -> bool {
        (0x1820..0x1860).contains(&cp)
            && self.promise_letters[promise as usize] & (1u64 << (cp - 0x1820)) != 0
    }

    /// Can promise `promise` be kept when the next unit is `unit`?
    pub(crate) fn promise_feasible(&self, unit: WrittenUnit, promise: u8) -> bool {
        self.promise_units[promise as usize] & (1u128 << (unit as u32)) != 0
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

impl Shaper {
    /// The normalize table, or [`Error::NormalizeUnsupported`] for locales without one.
    pub(crate) fn table(&self) -> Result<&NormalizeTable, Error> {
        self.normalize.as_ref().ok_or(Error::NormalizeUnsupported {
            locale: self.locale(),
        })
    }

    /// Version of the canonical Unicode selection policy (`"mng-canonical/3"` for MNG; `None`
    /// for locales without a normalize table). Persist it next to stored normalized keys.
    pub fn canonical_version(&self) -> Option<&'static str> {
        self.normalize.as_ref().map(|table| table.canonical_version)
    }

    /// The canonical text of a shape (duplicate encodings unified), `Ok(None)` when the tables do
    /// not cover it. A shape of structural tokens only is copied through without a table, so it
    /// works on every locale (Python parity).
    pub(crate) fn encode_shape(&self, shape: &[WrittenUnit]) -> Result<Option<String>, Error> {
        if shape.iter().all(|unit| unit.is_structural()) {
            return Ok(Some(
                shape
                    .iter()
                    .map(|unit| structural_char(*unit).expect("structural token"))
                    .collect(),
            ));
        }
        Ok(encoder::encode(self.table()?, shape))
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
        match self.encode_shape(&target)? {
            Some(canonical) => {
                // The tables are generated so that this always holds; the tests run it on every
                // call (the test profile keeps debug assertions).
                debug_assert_eq!(
                    self.shape(&canonical).as_deref(),
                    Ok(&target[..]),
                    "normalize({text:?}) = {canonical:?} does not reshape to the input's shape"
                );
                Ok(canonical)
            }
            None if strict => Err(Error::NormalizationFallback {
                text: text.to_owned(),
                written_units: target,
            }),
            None => Ok(text.to_owned()),
        }
    }

    /// Canonical encoding of one Mongolian word: within the normalize tables' domain,
    /// `shape(x) == shape(y)` ⟹ `normalize(x) == normalize(y)`, `shape(normalize(x)) == shape(x)`,
    /// and the encoding of a word's shape-prefix is a prefix of the word's encoding apart from its
    /// last letter.
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
            Some("mng-canonical/3")
        );
    }

    #[test]
    fn positions_of_slots_and_unit_keys() {
        assert_eq!(slot_position(0, 1, 1), Position::Isol);
        assert_eq!(slot_position(0, 2, 2), Position::Isol);
        assert_eq!(slot_position(0, 1, 3), Position::Init);
        assert_eq!(slot_position(1, 1, 3), Position::Medi);
        assert_eq!(slot_position(1, 2, 3), Position::Fina);
        assert_eq!(
            UnitKey::new(&[WrittenUnit::A]),
            UnitKey::new(&[WrittenUnit::A])
        );
        assert_ne!(
            UnitKey::new(&[WrittenUnit::A]),
            UnitKey::new(&[WrittenUnit::A, WrittenUnit::A])
        );
        assert_eq!(UnitKey::new(&[]), None);
        assert_eq!(UnitKey::new(&[WrittenUnit::A; 4]), None);
    }

    /// The robustness masks are indexed by `unit as u32`: the enum's discriminants must follow
    /// `WrittenUnit::ALL`, the order the generator writes (`mask_units`).
    #[test]
    fn written_unit_discriminants_follow_all() {
        for (index, unit) in WrittenUnit::ALL.iter().enumerate() {
            assert_eq!(*unit as usize, index, "{unit:?}");
        }
        assert!(WrittenUnit::ALL.len() <= 128, "a mask holds 128 units");
    }

    #[test]
    fn the_empty_table_encodes_structural_tokens_only() {
        let shaper = Shaper::with_empty_normalize_table(Locale::Mng);
        assert_eq!(
            shaper.normalize("\u{180E}\u{180A}").unwrap(),
            "\u{180E}\u{180A}"
        );
        assert_eq!(shaper.canonical_version(), Some("mng-canonical/3"));
    }
}
