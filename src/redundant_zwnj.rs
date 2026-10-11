//! Redundant ZWNJ (U+200C): joining breaks that change no glyph and are therefore dropped from
//! the public shape (issue #34).
//!
//! A ZWNJ breaks the cursive chain: the letter before it is written `fina` (or `isol`), the
//! letter after it `init` (or `isol`). Following the reference font (mongfontbuilder
//! `hudum.otf`, Kushim-Jiang/mongolian#58) the phase-III context rules look *through* it —
//! vowel harmony, the gender markers, chachlag, dotless g and the particle lookup all behave as
//! they do across a nirugu ([`crate::token::zwnj_blocks_context`] is the one switch) — so the
//! break changes ink only through those joining positions. Where it changes none, keeping a
//! `Zwnj` unit in the shape would break the crate's one promise, same ink ⟹ same `shape`: a
//! ZWNJ at the start of a word, between two MVS, or between two letters whose joined and
//! unjoined forms are the same shape is as invisible as a redundant ZWJ.
//!
//! # The criterion
//!
//! A ZWNJ is dropped iff the word's public shape is the same with and without it, and "the
//! same" is the test the ZWJ filter applies at a segment edge: unit for unit the names must
//! match, and the units beside the break must sit in the reviewed Hudum equivalence classes
//! for the position they move to. Everything is decided on the written units — the engine's
//! output after the redundant-ZWJ filter (`redundant_zwj.rs`) and duplicate unification
//! (`duplicates.rs`) — never by re-shaping the text or consulting a font:
//!
//! 1. Take the `Zwnj` out. The two sides are now one segment, so the ZWJ filter runs again (a
//!    ZWJ that only the break made necessary goes with it) and so does duplicate unification
//!    (a pair the break kept apart may now contract).
//! 2. Only the unit before the break and the unit after it change form, and only when the
//!    other side is something they would join — a letter unit or a joiner (`Nirugu`, `Zwj`).
//!    The unit before goes `fina` → `medi`, or `isol` → `init` when it is alone in its segment;
//!    the unit after goes `init` → `medi`, or `isol` → `fina` when alone. These are the very
//!    pairs the four edge tables of `redundant_zwj.rs` were reviewed for, so each side is one
//!    lookup in the list a ZWJ at that edge would use
//!    ([`crate::redundant_zwj::zwnj_join_keeps_shape`]). Outside Hudum no changed position is
//!    the same shape.
//! 3. If every changed unit is in its list, the break is redundant and the shape of step 1
//!    stands; otherwise the `Zwnj` stays, and it stays a segment edge for the ZWJ rules and for
//!    duplicate unification (`L I Zwnj Aa` does not contract to `L G`).
//!
//! Why the units and not the text: a break can be needed by one *spelling* and not by the
//! ink. ᠯᠠᠯᠠ‍‌ᠫᠦᠳ (the first word of issue #34) is `L A L A Zwj Zwnj P O I O A` — the `ue` takes
//! its marked `O I` only because the break makes the `p` initial, and the same text without
//! the ZWNJ renders `P O O A`. But `P:init` and `P:medi` are the same shape, so the ink is
//! exactly that of `L A L A P O I O A`, which `normalize` spells with pinned letters and no
//! break at all; had the decision been made by re-shaping the text, the first word would have
//! kept its `Zwnj` and had no canonical spelling (every pinned spelling drops the break).
//!
//! The filter is a fixed point of left-to-right single removals, like the ZWJ pass: after each
//! drop steps 1–2 run again on the result, so dropping one break can make the next one
//! redundant. Repeated ZWNJs collapse to one — or to none, when the survivor is redundant too.
//! A lone ZWNJ — nothing but ZWNJs (and FVS) in the word — keeps one `Zwnj`, exactly as a lone
//! ZWJ keeps its `Zwj`: it is the whole shape. Applying the filter to its own output changes
//! nothing.
//!
//! The filter touches nothing without a `Zwnj`, so no shape without a ZWNJ can move.
//! `shape_raw`, `shape_detailed` and the trace tokens keep every ZWNJ of the input.

use crate::duplicates::collapse;
use crate::generated::enums::WrittenUnit;
use crate::redundant_zwj::{drop_redundant_zwj, zwnj_join_keeps_shape};

/// The public shape of `units`, which are already free of redundant ZWJs and have their
/// duplicates unified: every redundant `Zwnj` removed, as described in the module docs.
/// `hudum_edges` enables the reviewed edge equivalences (MNG only).
pub(crate) fn drop_redundant_zwnj(units: &[WrittenUnit], hudum_edges: bool) -> Vec<WrittenUnit> {
    let mut units = units.to_vec();
    while let Some(index) = (0..units.len()).find(|&index| {
        units[index] == WrittenUnit::Zwnj && is_redundant(&units, index, hudum_edges)
    }) {
        units.remove(index);
        units = collapse(&drop_redundant_zwj(&units, hudum_edges));
    }
    units
}

/// Is the `Zwnj` at `units[index]` redundant?
fn is_redundant(units: &[WrittenUnit], index: usize, hudum_edges: bool) -> bool {
    // A lone ZWNJ is the whole shape, like a lone ZWJ.
    units.len() > 1 && zwnj_join_keeps_shape(units, index, hudum_edges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use WrittenUnit::{Aa, Mvs, Nirugu, Zwj, Zwnj, A, B, G, I, L, N, O, P, S, T};

    fn drop(units: &[WrittenUnit]) -> Vec<WrittenUnit> {
        drop_redundant_zwnj(units, true)
    }

    #[test]
    fn edges_and_controls() {
        assert_eq!(drop(&[Zwnj, B, Aa]), [B, Aa]);
        assert_eq!(drop(&[B, Aa, Zwnj]), [B, Aa]);
        assert_eq!(drop(&[Zwnj, N, A, Zwnj]), [N, A]);
        assert_eq!(drop(&[A, A, Zwnj, Mvs, Aa]), [A, A, Mvs, Aa]);
        assert_eq!(drop(&[Mvs, Zwnj, Aa]), [Mvs, Aa]);
        assert_eq!(drop(&[Zwnj, Mvs]), [Mvs]);
        assert_eq!(drop(&[Mvs, Zwnj]), [Mvs]);
        assert_eq!(drop(&[Zwnj, Nirugu, G]), [Nirugu, G]);
    }

    #[test]
    fn repeated_zwnj_collapse_and_a_lone_one_stays() {
        assert_eq!(drop(&[Zwnj]), [Zwnj]);
        assert_eq!(drop(&[Zwnj, Zwnj]), [Zwnj]);
        assert_eq!(drop(&[Zwnj, Zwnj, Zwnj]), [Zwnj]);
        assert_eq!(drop(&[A, A, Zwnj, Zwnj, B]), [A, A, Zwnj, B]);
        assert_eq!(drop(&[B, Zwnj, Zwnj, B, Aa]), [B, B, Aa]);
        assert_eq!(drop(&[]), []);
    }

    #[test]
    fn between_letters_the_edge_tables_decide() {
        // Both sides reviewed: `B:isol` ≡ `B:init`, `B:init` ≡ `B:medi`.
        assert_eq!(drop(&[B, Zwnj, B, Aa]), [B, B, Aa]);
        assert_eq!(drop(&[B, Zwnj, Aa]), [B, Aa]);
        assert_eq!(drop(&[N, Zwnj, Nirugu]), [N, Nirugu]);
        // One side not: `A:isol` ≠ `A:fina`, `A:isol` ≠ `A:init`, `L:fina` ≠ `L:medi`.
        assert_eq!(drop(&[B, Zwnj, A, A]), [B, Zwnj, A, A]);
        assert_eq!(drop(&[A, A, Zwnj, B]), [A, A, Zwnj, B]);
        assert_eq!(drop(&[T, A, L, Zwnj, A, A]), [T, A, L, Zwnj, A, A]);
        // A kept break is a wall for duplicate unification…
        assert_eq!(drop(&[L, I, Zwnj, Aa]), [L, I, Zwnj, Aa]);
        // …and a dropped one lets the merged chain unify: `O Aa` ending a chain is `B2`.
        assert_eq!(drop(&[S, O, Zwnj, Aa]), [S, WrittenUnit::B2]);
    }

    #[test]
    fn the_zwj_filter_runs_again_on_the_joined_segment() {
        // A kept `Zwj` beside the break already joined its letter; after the break goes it sits
        // between two letters and is redundant too.
        assert_eq!(drop(&[A, A, Zwj, Zwnj, B, Aa]), [A, A, B, Aa]);
        assert_eq!(
            drop(&[L, A, L, A, Zwj, Zwnj, P, O, I, O, A]),
            [L, A, L, A, P, O, I, O, A]
        );
        assert_eq!(drop(&[N, Zwnj, Zwj, A]), [N, A]);
        // A `Zwj` the break made necessary stays when the break stays.
        assert_eq!(drop(&[A, A, Zwj, Zwnj, B]), [A, A, Zwj, Zwnj, B]);
    }

    #[test]
    fn fixed_point_and_idempotence() {
        // The second break is redundant only once the first is gone (`B:init` ≡ `B:isol` needs
        // the `B` to be alone in its segment).
        assert_eq!(drop(&[Zwnj, B, Zwnj, Zwnj]), [B]);
        for units in [
            vec![B, Zwnj, A, A],
            vec![L, I, Zwnj, Aa],
            vec![A, A, Zwj, Zwnj, B],
            vec![Zwnj, Zwnj],
            vec![N, Zwnj, Zwj, A],
            vec![T, A, L, Zwnj, A, A],
        ] {
            let once = drop(&units);
            assert_eq!(drop(&once), once, "{units:?}");
            // Outside Hudum only the structural cases hold.
            let structural = drop_redundant_zwnj(&units, false);
            assert_eq!(drop_redundant_zwnj(&structural, false), structural);
        }
        assert_eq!(drop_redundant_zwnj(&[B, Zwnj, Aa], false), [B, Zwnj, Aa]);
        assert_eq!(drop_redundant_zwnj(&[Zwnj, B, Aa], false), [B, Aa]);
    }
}
