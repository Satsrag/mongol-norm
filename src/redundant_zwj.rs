//! Redundant ZWJ (U+200D): joiner tokens that change no glyph and are therefore dropped from
//! the public shape (issue #33).
//!
//! UTN #57 v4 treats ZWJ purely as a cursive joining control: it forces the neighbouring letters
//! into their joined (`init` / `medi` / `fina`) forms. Where the neighbours already join, or
//! where the joined form is the same ink as the unjoined one, the ZWJ is invisible. Keeping it as
//! a `Zwj` unit would break the crate's one promise — same ink ⟹ same `shape` — because the token
//! also stops duplicate unification from crossing it: ᠯᠢᠡ᠋ is `L G`, ᠯᠢ+ZWJ+ᠡ᠋ was `L I Zwj Aa`.
//!
//! The filter runs on the written-unit sequence — after the rules resolved every letter and
//! **before** duplicate unification ([`crate::duplicates::collapse`]) — so it has the same view
//! of the word whether the units came from `shape` or were handed in through
//! `normalize_written_units`. FVS marks are already bound to their letters at this point, so
//! removing the token can never re-attach a selector to a different letter.
//!
//! # Rules (left to right, repeated until nothing changes)
//!
//! Everything here is about written units: a "letter unit" is any unit other than the structural
//! `Mvs`, `Nirugu`, `Zwj` and `Zwnj` ([`WrittenUnit::is_structural`]), and `U` names one such
//! unit. A segment edge is the start or end of the word, an `Mvs` or a `Zwnj` — the three places
//! the position assignment breaks the joining chain.
//!
//! | ZWJ context | result |
//! |---|---|
//! | `Zwj Zwj` | one copy survives (then judged like a single one) |
//! | next to a `Nirugu`, on either side | dropped — the nirugu already joins |
//! | between two letter units (segment interior) | dropped |
//! | segment-initial (start of word or directly after an `Mvs`/`Zwnj`), before a letter unit `U` | dropped iff `U:init` is accepted as the same shape as `U:medi` |
//! | segment-initial, before a lone letter unit `U` | dropped iff `U:isol` is accepted as the same shape as `U:fina` |
//! | segment-final (end of word or directly before an `Mvs`/`Zwnj`), after a letter unit `U` | dropped iff `U:fina` is accepted as the same shape as `U:medi` |
//! | segment-final, after a lone letter unit `U` | dropped iff `U:isol` is accepted as the same shape as `U:init` |
//! | no letter unit on either side (`Zwj Mvs…`, `…Mvs Zwj`, `Mvs Zwj Mvs`, `Zwj Zwnj…`) | dropped — it has nothing to join |
//! | a lone `Zwj` | kept (it is the whole shape) |
//!
//! The four edge tables are the maintainer's reviewed equivalence classes for Hudum, selected on
//! issue #33 from an ink comparison of every unit on the reference font (mongfontbuilder
//! `hudum.otf`): each listed pair was encoded with `normalize_written_units`, rendered with
//! HarfBuzz, and compared outline-against-outline. They are **Hudum (MNG) only**; the other
//! locales get the structural rules alone. "Lone" means the unit is the only letter of its
//! segment: nothing, or an `Mvs`, on the far side.
//!
//! An `Mvs` splits segments exactly as the position assignment splits them, and the reference
//! font agrees on both sides: the letter before an MVS takes its `fina` (or `isol`) form and the
//! letter after it its `init` (or `isol`) form, so a ZWJ next to an MVS asks the same question as
//! one at the word edge and uses the same lists. Every after-MVS pair differs from its
//! word-initial twin only in how the MVS itself is drawn: with a joiner after it the font shows
//! its dashed "MVS" placeholder box, without one a plain gap. That box marks an MVS the font
//! has no rule for, not text ink, so it does not count (`ᠠ᠎‍ᠶᠢᠨ` = `ᠠ᠎ᠶ᠌ᠢᠨ`, while `ᠲᠠᠯ᠎‍ᠠ`
//! keeps its `Zwj`: `A:fina` is not the suffix `a`). Examples before an MVS: `ᠠᠣ‍᠎ᠠ` = `ᠠᠣ᠋᠎ᠠ`,
//! `ᠪ‍᠎ᠠ` = `ᠪ᠎ᠠ`. A ZWJ whose only neighbours are MVS or the word boundary joins nothing: on the
//! reference font every such pair renders the same glyphs at the same positions, MVS included.
//!
//! A `Zwnj` is a segment edge on both sides for the same reason an `Mvs` is: the letter before it
//! takes its `fina` (or `isol`) form and the letter after it its `init` (or `isol`) form, so a
//! ZWJ beside it asks the word-edge question. The ZWNJ filter itself lives in
//! `redundant_zwnj.rs`, which reuses the four edge tables through [`zwnj_join_keeps_shape`].
//!
//! The pass is a fixed point of left-to-right single removals, so `Zwj B Zwj` first loses the
//! leading joiner (`B:medi` ≡ `B:init`), then — now a lone `B:init` — the trailing one
//! (`B:init` ≡ `B:isol`). Applying the filter to its own output changes nothing, which is what
//! `normalize`'s verification (`shape(candidate) == target`) relies on.

use crate::generated::enums::WrittenUnit;

use WrittenUnit::{
    Aa, Ch, Cr, Dd, Gx, Hx, Mvs, Nirugu, Rh, Sh, Zr, Zwj, Zwnj, B, C, D, F, G, H, I, K, K2, L, M,
    N, O, P, R, S, T, U, W, Y, Z,
};

/// Segment-initial `Zwj U X…` → `U X…` (`U:medi` ≡ `U:init`); at the start of the word or after an MVS.
const INITIAL_BEFORE_LETTER: &[WrittenUnit] = &[
    B, C, Ch, Cr, D, Dd, F, G, Gx, I, K, K2, O, P, R, Rh, S, Sh, T, W, Y, Z, Zr,
];

/// Segment-initial `Zwj U` with `U` alone in its segment → `U` (`U:fina` ≡ `U:isol`).
const INITIAL_ALONE: &[WrittenUnit] = &[Aa, Cr, Dd, I, U, Zr];

/// Segment-final `…X U Zwj` → `…X U` (`U:medi` ≡ `U:fina`); at the end of the word or before an MVS.
const FINAL_AFTER_LETTER: &[WrittenUnit] = &[Cr, O, Zr];

/// Segment-final `U Zwj` with `U` alone in its segment → `U` (`U:init` ≡ `U:isol`).
const FINAL_ALONE: &[WrittenUnit] = &[
    B, C, Ch, Cr, D, F, G, Gx, H, Hx, K, K2, L, M, N, P, R, Rh, S, Sh, T, W, Y, Z, Zr,
];

/// Remove every redundant `Zwj` from a written-unit sequence (see the module docs).
/// `hudum_edges` enables the reviewed word-edge equivalences, which hold for MNG only.
pub(crate) fn drop_redundant_zwj(shape: &[WrittenUnit], hudum_edges: bool) -> Vec<WrittenUnit> {
    let mut units: Vec<WrittenUnit> = Vec::with_capacity(shape.len());
    for &unit in shape {
        if unit == Zwj && units.last() == Some(&Zwj) {
            continue;
        }
        units.push(unit);
    }
    loop {
        let before = units.len();
        let mut index = 0;
        while index < units.len() {
            if units[index] == Zwj && is_redundant(&units, index, hudum_edges) {
                units.remove(index);
            } else {
                index += 1;
            }
        }
        if units.len() == before {
            return units;
        }
    }
}

fn is_letter(unit: WrittenUnit) -> bool {
    !unit.is_structural()
}

/// Does `unit` end a segment the way the word boundary does — an `Mvs` or a `Zwnj`?
fn is_edge(unit: WrittenUnit) -> bool {
    matches!(unit, Mvs | Zwnj)
}

/// Is the `Zwj` at `units[index]` redundant? `units` holds no repeated `Zwj`.
fn is_redundant(units: &[WrittenUnit], index: usize, hudum_edges: bool) -> bool {
    let prev = index.checked_sub(1).map(|i| units[i]);
    let next = units.get(index + 1).copied();
    match (prev, next) {
        (Some(Nirugu), _) | (_, Some(Nirugu)) => true,
        (Some(p), Some(n)) if is_letter(p) && is_letter(n) => true,
        (None | Some(Mvs | Zwnj), Some(n)) if is_letter(n) => {
            // An MVS or ZWNJ starts the segment like the start of the word: `n` is `init` (or
            // `isol`). The far side of `n`: another letter or a joiner keeps `n` joined onward,
            // so dropping the ZWJ turns `n:medi` into `n:init`; nothing (or an edge) leaves a
            // lone `n`, `fina` → `isol`.
            let alone = units.get(index + 2).map_or(true, |&unit| is_edge(unit));
            let same_shape = if alone {
                INITIAL_ALONE
            } else {
                INITIAL_BEFORE_LETTER
            };
            hudum_edges && same_shape.contains(&n)
        }
        (Some(p), None | Some(Mvs | Zwnj)) if is_letter(p) => {
            // An MVS or ZWNJ ends the segment like the end of the word: `p` is `fina` (or
            // `isol`) either way.
            let alone = index < 2 || is_edge(units[index - 2]);
            let same_shape = if alone {
                FINAL_ALONE
            } else {
                FINAL_AFTER_LETTER
            };
            hudum_edges && same_shape.contains(&p)
        }
        // A lone `Zwj` is the whole shape.
        (None, None) => false,
        // Only edges or the word boundary around it: the joiner has nothing to join.
        _ => true,
    }
}

/// Would taking the `Zwnj` at `units[index]` out — so that its two sides join — leave every
/// letter unit at the same shape? Only the units right beside the break change form, and only
/// when the other side is something they would join: a letter unit or a joiner (`Nirugu`,
/// `Zwj`). The unit before the break goes `fina` → `medi`, or `isol` → `init` when it is alone
/// in its segment; the unit after it goes `init` → `medi`, or `isol` → `fina` when alone —
/// the very pairs the four edge tables were reviewed for, so this is one question per side with
/// the same lists `is_redundant` uses for a ZWJ at that edge (and `false` outside Hudum).
///
/// The caller has already established that the unit *names* are the same with and without the
/// break; this settles whether the changed joining positions are accepted as the same ink.
pub(crate) fn zwnj_join_keeps_shape(
    units: &[WrittenUnit],
    index: usize,
    hudum_edges: bool,
) -> bool {
    debug_assert_eq!(units[index], Zwnj);
    let prev = index.checked_sub(1).map(|i| units[i]);
    let next = units.get(index + 1).copied();
    let joins = |unit: Option<WrittenUnit>| unit.is_some_and(|unit| !is_edge(unit));
    if let Some(p) = prev.filter(|&p| is_letter(p)) {
        if joins(next) {
            let alone = index < 2 || is_edge(units[index - 2]);
            let same_shape = if alone {
                FINAL_ALONE
            } else {
                FINAL_AFTER_LETTER
            };
            if !(hudum_edges && same_shape.contains(&p)) {
                return false;
            }
        }
    }
    if let Some(n) = next.filter(|&n| is_letter(n)) {
        if joins(prev) {
            let alone = units.get(index + 2).map_or(true, |&unit| is_edge(unit));
            let same_shape = if alone {
                INITIAL_ALONE
            } else {
                INITIAL_BEFORE_LETTER
            };
            if !(hudum_edges && same_shape.contains(&n)) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use WrittenUnit::A;

    fn drop(shape: &[WrittenUnit]) -> Vec<WrittenUnit> {
        drop_redundant_zwj(shape, true)
    }

    #[test]
    fn interior_zwj_between_letters_is_dropped() {
        assert_eq!(drop(&[L, I, Zwj, Aa]), [L, I, Aa]);
        assert_eq!(drop(&[A, A, R, A, Zwj, O, A]), [A, A, R, A, O, A]);
        assert_eq!(
            drop(&[L, A, L, A, Zwj, P, O, O, A]),
            [L, A, L, A, P, O, O, A]
        );
    }

    #[test]
    fn zwj_next_to_a_nirugu_is_dropped_anywhere() {
        assert_eq!(drop(&[A, L, Zwj, Nirugu]), [A, L, Nirugu]);
        assert_eq!(drop(&[Nirugu, Zwj, L, A]), [Nirugu, L, A]);
        assert_eq!(drop(&[A, L, Nirugu, Zwj]), [A, L, Nirugu]);
        assert_eq!(drop(&[Zwj, Nirugu, L, A]), [Nirugu, L, A]);
        assert_eq!(drop(&[A, L, Nirugu, Zwj, Mvs, Aa]), [A, L, Nirugu, Mvs, Aa]);
    }

    #[test]
    fn repeated_zwj_keeps_one() {
        assert_eq!(drop(&[Zwj, Zwj, N, A]), [Zwj, N, A]);
        assert_eq!(drop(&[A, A, Zwj, Zwj]), [A, A, Zwj]);
        assert_eq!(drop(&[Zwj, Zwj, Zwj]), [Zwj]);
        assert_eq!(drop(&[L, I, Zwj, Zwj, Aa]), [L, I, Aa]);
    }

    #[test]
    fn zwj_after_mvs_is_segment_initial() {
        assert_eq!(drop(&[A, Mvs, Zwj, B, Aa]), [A, Mvs, B, Aa]); // B:medi ≡ B:init
        assert_eq!(drop(&[A, Mvs, Zwj, Y, I, A]), [A, Mvs, Y, I, A]); // Y:medi ≡ Y:init
        assert_eq!(drop(&[A, Mvs, Zwj, N, A]), [A, Mvs, Zwj, N, A]); // N:medi ≠ N:init
        assert_eq!(drop(&[A, Mvs, Zwj, Aa]), [A, Mvs, Aa]); // lone: Aa:fina ≡ Aa:isol
        assert_eq!(drop(&[A, Mvs, Zwj, A]), [A, Mvs, Zwj, A]); // lone: A:fina ≠ the suffix a
        assert_eq!(drop(&[A, Mvs, Zwj, B, Zwj, Mvs, Aa]), [A, Mvs, B, Mvs, Aa]);
        // fixed point
    }

    #[test]
    fn zwj_with_no_letter_on_either_side_is_dropped() {
        assert_eq!(drop(&[Zwj, Mvs, Aa]), [Mvs, Aa]);
        assert_eq!(drop(&[A, Mvs, Zwj]), [A, Mvs]);
        assert_eq!(drop(&[A, Mvs, Zwj, Mvs, Aa]), [A, Mvs, Mvs, Aa]);
        assert_eq!(drop(&[Mvs, Zwj]), [Mvs]);
        assert_eq!(drop(&[Zwj, Mvs]), [Mvs]);
        // Structural, so it holds without the Hudum edge tables too.
        assert_eq!(drop_redundant_zwj(&[Zwj, Mvs, Aa], false), [Mvs, Aa]);
    }

    #[test]
    fn zwj_before_mvs_is_segment_final() {
        assert_eq!(drop(&[A, L, Zwj, Mvs, Aa]), [A, L, Zwj, Mvs, Aa]); // L:medi ≠ L:fina
        assert_eq!(drop(&[A, O, Zwj, Mvs, Aa]), [A, O, Mvs, Aa]); // O:medi ≡ O:fina
        assert_eq!(drop(&[B, Zwj, Mvs, Aa]), [B, Mvs, Aa]); // lone: B:init ≡ B:isol
        assert_eq!(drop(&[A, Zwj, Mvs, Aa]), [A, Zwj, Mvs, Aa]); // lone: A:init ≠ A:isol
        assert_eq!(drop(&[Mvs, B, Zwj, Mvs, Aa]), [Mvs, B, Mvs, Aa]); // lone between two MVS
        assert_eq!(drop(&[Zwj, B, Zwj, Mvs, Aa]), [B, Mvs, Aa]); // fixed point, as at a word end
    }

    #[test]
    fn lone_zwj_is_the_whole_shape() {
        assert_eq!(drop(&[Zwj]), [Zwj]);
        assert_eq!(drop(&[]), []);
    }

    #[test]
    fn word_initial_zwj_follows_the_reviewed_table() {
        assert_eq!(drop(&[Zwj, B, Aa]), [B, Aa]); // B:medi ≡ B:init
        assert_eq!(drop(&[Zwj, S, A]), [S, A]); // accepted near-equivalent
        assert_eq!(drop(&[Zwj, N, A]), [Zwj, N, A]); // N:medi ≠ N:init
        assert_eq!(drop(&[Zwj, A, A]), [Zwj, A, A]);
        assert_eq!(drop(&[Zwj, I]), [I]); // lone: I:fina ≡ I:isol
        assert_eq!(drop(&[Zwj, B]), [Zwj, B]); // lone: B:fina ≠ B:isol
        assert_eq!(drop(&[Zwj, B, Mvs, Aa]), [Zwj, B, Mvs, Aa]); // lone before MVS
        assert_eq!(drop(&[Zwj, U, Mvs, Aa]), [U, Mvs, Aa]);
        assert_eq!(drop(&[Zwj, B, Nirugu]), [B, Nirugu]); // joined onward by the nirugu
    }

    #[test]
    fn word_final_zwj_follows_the_reviewed_table() {
        assert_eq!(drop(&[A, A, Zwj]), [A, A, Zwj]); // A:medi ≠ A:fina
        assert_eq!(drop(&[A, O, Zwj]), [A, O]); // O:medi ≡ O:fina
        assert_eq!(drop(&[B, Zwj]), [B]); // lone: B:init ≡ B:isol
        assert_eq!(drop(&[A, Zwj]), [A, Zwj]); // lone: A:init ≠ A:isol
        assert_eq!(drop(&[Mvs, B, Zwj]), [Mvs, B]); // lone after MVS
        assert_eq!(drop(&[Nirugu, B, Zwj]), [Nirugu, B, Zwj]); // joined-left B is medi
    }

    #[test]
    fn both_edges_resolve_to_a_fixed_point() {
        // Leading joiner first (B:medi ≡ B:init), then the trailing one (B:init ≡ B:isol).
        assert_eq!(drop(&[Zwj, B, Zwj]), [B]);
        // A keeps both.
        assert_eq!(drop(&[Zwj, A, Zwj]), [Zwj, A, Zwj]);
        // O: leading drops (O:medi ≡ O:init); then `O Zwj` is a lone O:init, not equivalent.
        assert_eq!(drop(&[Zwj, O, Zwj]), [O, Zwj]);
    }

    #[test]
    fn zwnj_is_a_segment_edge_for_the_zwj_rules() {
        // Before a ZWNJ: the word-final question.
        assert_eq!(drop(&[B, Zwj, Zwnj, A, A]), [B, Zwnj, A, A]); // lone: B:init ≡ B:isol
        assert_eq!(drop(&[A, Zwj, Zwnj, B]), [A, Zwj, Zwnj, B]); // lone: A:init ≠ A:isol
        assert_eq!(drop(&[A, O, Zwj, Zwnj, B]), [A, O, Zwnj, B]); // O:medi ≡ O:fina
        assert_eq!(drop(&[A, L, Zwj, Zwnj, B]), [A, L, Zwj, Zwnj, B]); // L:medi ≠ L:fina
                                                                       // After a ZWNJ: the word-initial question.
        assert_eq!(drop(&[A, Zwnj, Zwj, B, Aa]), [A, Zwnj, B, Aa]); // B:medi ≡ B:init
        assert_eq!(drop(&[A, Zwnj, Zwj, N, A]), [A, Zwnj, Zwj, N, A]); // N:medi ≠ N:init
        assert_eq!(drop(&[A, Zwnj, Zwj, Aa]), [A, Zwnj, Aa]); // lone: Aa:fina ≡ Aa:isol
        assert_eq!(drop(&[A, Zwnj, Zwj, A]), [A, Zwnj, Zwj, A]); // lone: A:fina ≠ the suffix a
                                                                 // A ZWNJ counts as the far edge when deciding "alone".
        assert_eq!(drop(&[Zwnj, B, Zwj, Zwnj]), [Zwnj, B, Zwnj]); // lone B between two breaks
        assert_eq!(drop(&[Zwj, B, Zwnj, A]), [Zwj, B, Zwnj, A]); // lone: B:fina ≠ B:isol
                                                                 // No letter beside it.
        assert_eq!(drop(&[Zwj, Zwnj, N, A]), [Zwnj, N, A]);
        assert_eq!(drop(&[N, A, Zwnj, Zwj]), [N, A, Zwnj]);
        assert_eq!(drop(&[Zwnj, Zwj, Zwnj]), [Zwnj, Zwnj]);
        assert_eq!(drop(&[Mvs, Zwj, Zwnj]), [Mvs, Zwnj]);
    }

    #[test]
    fn zwnj_join_follows_the_edge_tables() {
        // Both sides change: `B:isol` → `B:init` and `Aa:isol` → `Aa:fina`, both reviewed.
        assert!(zwnj_join_keeps_shape(&[B, Zwnj, Aa], 1, true));
        // `A:isol` → `A:fina` is not.
        assert!(!zwnj_join_keeps_shape(&[B, Zwnj, A], 1, true));
        // Letters on the far sides: `B:fina` → `B:medi` is not reviewed as the same shape…
        assert!(!zwnj_join_keeps_shape(&[A, B, Zwnj, B, Aa], 2, true));
        // …but `O:fina` → `O:medi` and `B:init` → `B:medi` are.
        assert!(zwnj_join_keeps_shape(&[A, O, Zwnj, B, Aa], 2, true));
        // A joiner on the far side joins the letter too: `N:isol` → `N:init` (reviewed).
        assert!(zwnj_join_keeps_shape(&[N, Zwnj, Nirugu], 1, true));
        assert!(zwnj_join_keeps_shape(&[N, Zwnj, Zwj, A], 1, true));
        // A kept ZWJ beside the break already joins its letter: nothing changes on that side.
        assert!(zwnj_join_keeps_shape(&[A, Zwj, Zwnj, B, Aa], 2, true));
        // An edge or the word boundary on the other side: nothing changes at all.
        assert!(zwnj_join_keeps_shape(&[Zwnj, N, A], 0, true));
        assert!(zwnj_join_keeps_shape(&[N, A, Zwnj], 2, true));
        assert!(zwnj_join_keeps_shape(&[A, A, Zwnj, Mvs, Aa], 2, true));
        assert!(zwnj_join_keeps_shape(&[Mvs, Zwnj, Aa], 1, true));
        assert!(zwnj_join_keeps_shape(&[A, A, Zwnj, Zwnj, B], 2, true));
        // The tables are Hudum data; elsewhere a changed position is never the same shape.
        assert!(!zwnj_join_keeps_shape(&[B, Zwnj, Aa], 1, false));
        assert!(zwnj_join_keeps_shape(&[Zwnj, N, A], 0, false));
    }

    #[test]
    fn idempotent_and_edge_tables_are_hudum_only() {
        for shape in [
            vec![Zwj, B, Zwj],
            vec![Zwj, N, A, Zwj],
            vec![L, I, Zwj, Aa],
            vec![A, L, Zwj, Mvs, Aa],
            vec![Zwj, Zwj, Nirugu, Zwj],
        ] {
            let once = drop(&shape);
            assert_eq!(drop(&once), once, "{shape:?}");
            let structural = drop_redundant_zwj(&shape, false);
            assert_eq!(drop_redundant_zwj(&structural, false), structural);
        }
        assert_eq!(drop_redundant_zwj(&[Zwj, B, Zwj], false), [Zwj, B, Zwj]);
        assert_eq!(drop_redundant_zwj(&[L, I, Zwj, Aa], false), [L, I, Aa]);
    }
}
