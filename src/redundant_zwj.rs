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
//! `Mvs`, `Nirugu` and `Zwj` ([`WrittenUnit::is_structural`]), and `U` names one such unit.
//!
//! | ZWJ context | result |
//! |---|---|
//! | `Zwj Zwj` | one copy survives (then judged like a single one) |
//! | next to a `Nirugu`, on either side | dropped — the nirugu already joins |
//! | between two letter units (segment interior) | dropped |
//! | segment-initial (start of word or directly after an `Mvs`), before a letter unit `U` | dropped iff `U:init` is accepted as the same shape as `U:medi` |
//! | segment-initial, before a lone letter unit `U` | dropped iff `U:isol` is accepted as the same shape as `U:fina` |
//! | segment-final (end of word or directly before an `Mvs`), after a letter unit `U` | dropped iff `U:fina` is accepted as the same shape as `U:medi` |
//! | segment-final, after a lone letter unit `U` | dropped iff `U:isol` is accepted as the same shape as `U:init` |
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
//! `ᠪ‍᠎ᠠ` = `ᠪ᠎ᠠ`.
//!
//! The pass is a fixed point of left-to-right single removals, so `Zwj B Zwj` first loses the
//! leading joiner (`B:medi` ≡ `B:init`), then — now a lone `B:init` — the trailing one
//! (`B:init` ≡ `B:isol`). Applying the filter to its own output changes nothing, which is what
//! `normalize`'s verification (`shape(candidate) == target`) relies on.

use crate::generated::enums::WrittenUnit;

use WrittenUnit::{
    Aa, Ch, Cr, Dd, Gx, Hx, Mvs, Nirugu, Rh, Sh, Zr, Zwj, B, C, D, F, G, H, I, K, K2, L, M, N, O,
    P, R, S, T, U, W, Y, Z,
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

/// Is the `Zwj` at `units[index]` redundant? `units` holds no repeated `Zwj`.
fn is_redundant(units: &[WrittenUnit], index: usize, hudum_edges: bool) -> bool {
    let prev = index.checked_sub(1).map(|i| units[i]);
    let next = units.get(index + 1).copied();
    match (prev, next) {
        (Some(Nirugu), _) | (_, Some(Nirugu)) => true,
        (Some(p), Some(n)) if is_letter(p) && is_letter(n) => true,
        (None | Some(Mvs), Some(n)) if is_letter(n) => {
            // An MVS starts the segment like the start of the word: `n` is `init` (or `isol`).
            // The far side of `n`: another letter or a joiner keeps `n` joined onward, so
            // dropping the ZWJ turns `n:medi` into `n:init`; nothing (or an MVS) leaves a lone
            // `n`, `fina` → `isol`.
            let alone = matches!(units.get(index + 2), None | Some(Mvs));
            let same_shape = if alone {
                INITIAL_ALONE
            } else {
                INITIAL_BEFORE_LETTER
            };
            hudum_edges && same_shape.contains(&n)
        }
        (Some(p), None | Some(Mvs)) if is_letter(p) => {
            // An MVS ends the segment like the end of the word: `p` is `fina` (or `isol`) either way.
            let alone = index < 2 || units[index - 2] == Mvs;
            let same_shape = if alone {
                FINAL_ALONE
            } else {
                FINAL_AFTER_LETTER
            };
            hudum_edges && same_shape.contains(&p)
        }
        _ => false,
    }
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
        assert_eq!(drop(&[A, Mvs, Zwj, B, Zwj, Mvs, Aa]), [A, Mvs, B, Mvs, Aa]); // fixed point
        assert_eq!(drop(&[Zwj, Mvs, Aa]), [Zwj, Mvs, Aa]); // no letter on either side
        assert_eq!(drop(&[A, Mvs, Zwj, Mvs, Aa]), [A, Mvs, Zwj, Mvs, Aa]);
        assert_eq!(drop(&[Mvs, Zwj]), [Mvs, Zwj]);
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
