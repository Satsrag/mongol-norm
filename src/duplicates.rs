//! Duplicate encodings: written units that render as exactly the same ink as a sequence of
//! other units, and are therefore unified in the public shape.
//!
//! UTN #57 / GB/T 25914-2023 keep all nine as distinct written units — its EAC conformance
//! vectors spell ᠠᠷᠭᠠᠯ as `A A R Hx A L` — but in the reference font (Noto Sans Mongolian 3.002,
//! the UTN #57 reference build) each pair is the same ink. Left in, they break this crate's one
//! promise: ᠠᠷᠠᠳ and ᠠᠷᠠᠤᠠ are the same visible word yet shaped to `A A R A Dd` and `A A R A O A`,
//! so `same_shape` said `false` and `normalize` gave two canonical texts for one word.
//!
//! So the public [`Shaper::shape`](crate::Shaper::shape) — and everything built on it:
//! `same_shape`, `normalize`, the written-unit encoders — sees the unified sequence. The engine's
//! own output, which is what the standard's vectors describe, remains available as
//! [`Shaper::shape_raw`](crate::Shaper::shape_raw).
//!
//! # Two context-safe canonical directions
//!
//! Five of the nine unify by **expansion** — the single unit becomes the pair:
//!
//! | duplicate | expands to | witness |
//! |---|---|---|
//! | `Dd:medi` | `O:medi A:medi` | |
//! | `Dd:fina` | `O:medi A:fina` | ᠠᠷᠠᠳ / ᠠᠷᠠᠤᠠ |
//! | `H:medi`  | `A:medi A:medi` | ᠪᠠᠭᠰᠢ |
//! | `Hx:medi` | `N:medi N:medi` | ᠠᠷᠭᠠᠯ |
//! | `Cr:init` | `O:init O:medi` | ᡂ᠊ / ᠤ᠋ᠤ᠊ |
//!
//! Four other forms unify by **contraction**, choosing the shorter canonical form only in
//! the verified context. In particular Aa:fina has a tooth immediately after a bowed written unit, but
//! no tooth elsewhere: inserting/removing A repeatedly does not preserve the ink.
//!
//! | pair | contracts to | witness |
//! |---|---|---|
//! | `A Aa` spanning a whole chain | `A:isol`  | ᠡ / ᠡᠠ᠋ |
//! | `bowed written unit A:medi Aa:fina` | `bowed written unit Aa:fina` | ᠪᠠ / ᠪᠠᠠ᠋ |
//! | `O Aa` ending a chain          | `B2:fina` | ᠊ᠪ᠋ / ᠊ᠤᠠ᠋ |
//! | `I Aa` ending a chain          | `G:fina`  | ᠊ᠭ / ᠊ᠢᠠ᠋ |
//!
//! # One expansion pass and at most one tail contraction
//!
//! Expansion emits only `O`, `A` and `N`, and contraction emits only `A`, `Aa`, `B2` and `G` —
//! none of which is an expansion target — so no rule can resurrect one. Expansion only ever
//! *inserts* units, which cannot move a unit to index 0 nor past the end, so it never turns an
//! `init` or `fina` unit into the `medi` that `H`/`Hx` need; contraction only ever merges a pair
//! into the unit at the pair's own end, so it likewise never creates a `medi`. One expansion pass
//! therefore removes every expansion target for good.
//!
//! The Hudum bowed written units are B, P, F, G, Gx, K, K2: see the Hudum written-unit ligated variants
//! at <https://mongfontbuilder.pages.dev/hudum/>, mongfontbuilder data/ligatures.ts
//! (required BAa/PAa/FAa/GAa/GxAa/KAa/K2Aa), and this crate's rules::iii5_post_bowed_at.
//! Other scripts' ligatures are not evidence for additional Hudum bowed written units.
//!
//! All contractions require a chain-ending pair. A:isol, B2:fina and G:fina do not end in
//! Aa; the context-gated Aa result is immediately preceded by a bowed written unit, not A/O/I. Thus no
//! result admits another contraction, and no expansion target is created. Idempotence follows
//! from those guards, not from repeated deletion: B A A Aa and N A Aa stay unchanged.
//! Dd Aa expands to O A Aa, whose A is not after a bowed written unit, so it must not become B2.

use crate::generated::enums::WrittenUnit;
use crate::normalize::{is_joiner, slot_position};
use crate::tables::Position;

/// The replacement for one unit at one position, if it is a duplicate encoding that unifies by
/// expansion.
///
/// `Dd` exists only in medial and final position and both are duplicates, so it expands wherever
/// it occurs; `H` and `Hx` are duplicates in medial position only — their initial and final forms
/// are distinct ink and are kept. `Cr` is gated on `init` because that is the position the pair
/// was verified in; a lone `Cr` chain sits at `isol` and is left alone.
fn expansion(unit: WrittenUnit, position: Position) -> Option<&'static [WrittenUnit]> {
    match (unit, position) {
        (WrittenUnit::Dd, _) => Some(&[WrittenUnit::O, WrittenUnit::A]),
        (WrittenUnit::H, Position::Medi) => Some(&[WrittenUnit::A, WrittenUnit::A]),
        (WrittenUnit::Hx, Position::Medi) => Some(&[WrittenUnit::N, WrittenUnit::N]),
        (WrittenUnit::Cr, Position::Init) => Some(&[WrittenUnit::O, WrittenUnit::O]),
        _ => None,
    }
}

/// The single unit an adjacent pair contracts to, if the pair is a duplicate encoding.
///
/// `result` is the position the merged unit would occupy in the shortened chain, which is what
/// decides both *whether* the pair is the verified duplicate and *which* unit it becomes: `A Aa`
/// is `A` when it makes up a whole chain (`A:isol`) and `Aa` only when a bowed written unit immediately
/// precedes the A in the same chain (`Aa:fina`). `B2` and `G` are only duplicates at `fina`,
/// and `G:isol`/`G:init`/`G:medi` are distinct ink.
fn contraction(
    first: WrittenUnit,
    second: WrittenUnit,
    result: Position,
    preceding: Option<WrittenUnit>,
) -> Option<WrittenUnit> {
    if second != WrittenUnit::Aa {
        return None;
    }
    match (first, result) {
        (WrittenUnit::A, Position::Isol) => Some(WrittenUnit::A),
        (WrittenUnit::A, Position::Fina)
            if matches!(
                preceding,
                Some(
                    WrittenUnit::B
                        | WrittenUnit::P
                        | WrittenUnit::F
                        | WrittenUnit::G
                        | WrittenUnit::Gx
                        | WrittenUnit::K
                        | WrittenUnit::K2
                )
            ) =>
        {
            Some(WrittenUnit::Aa)
        }
        (WrittenUnit::O, Position::Fina) => Some(WrittenUnit::B2),
        (WrittenUnit::I, Position::Fina) => Some(WrittenUnit::G),
        _ => None,
    }
}

/// Unify every duplicate encoding in `shape`.
///
/// Positions are the ones `normalize` itself uses: slots within each chain between structural
/// units, with a nirugu or ZWJ neighbour padding the chain the way it pads the rendering, so a
/// unit joined across a ZWJ counts as medial exactly as the shaper rendered it. Joiner padding
/// supplies a position, never a bowed written unit. Idempotent after one expansion pass and one tail check.
pub(crate) fn collapse(shape: &[WrittenUnit]) -> Vec<WrittenUnit> {
    let mut out = Vec::with_capacity(shape.len() + 2);
    let mut start = 0;
    while start < shape.len() {
        if shape[start].is_structural() {
            out.push(shape[start]);
            start += 1;
            continue;
        }
        let mut end = start;
        while end < shape.len() && !shape[end].is_structural() {
            end += 1;
        }
        let pad_left = usize::from(start > 0 && is_joiner(shape[start - 1]));
        let pad_right = usize::from(end < shape.len() && is_joiner(shape[end]));
        out.extend(collapse_chain(&shape[start..end], pad_left, pad_right));
        start = end;
    }
    out
}

/// One chain of letter units, with the joiner padding that shifts its slot positions.
fn collapse_chain(chain: &[WrittenUnit], pad_left: usize, pad_right: usize) -> Vec<WrittenUnit> {
    let position =
        |index: usize, len: usize| slot_position(index + pad_left, 1, len + pad_left + pad_right);

    let mut units = Vec::with_capacity(chain.len() + 2);
    for (index, &unit) in chain.iter().enumerate() {
        match expansion(unit, position(index, chain.len())) {
            Some(expanded) => units.extend_from_slice(expanded),
            None => units.push(unit),
        }
    }

    // Every contraction requires the pair to end the chain. Inspect that pair once;
    // a newly merged Aa must not be fed back to consume another A.
    if let Some(index) = units.len().checked_sub(2) {
        if let Some(merged) = contraction(
            units[index],
            units[index + 1],
            position(index, units.len() - 1),
            index.checked_sub(1).map(|previous| units[previous]),
        ) {
            units[index] = merged;
            units.pop();
        }
    }
    units
}

#[cfg(test)]
mod tests {
    use super::*;
    use WrittenUnit::{
        Aa, Cr, Dd, Hx, Mvs, Nirugu, Zwj, A, B, B2, D, G, H, I, L, M, N, O, R, S, T,
    };

    #[test]
    fn the_five_expanding_duplicates_collapse_by_position() {
        assert_eq!(collapse(&[A, A, R, A, Dd]), [A, A, R, A, O, A]); // Dd:fina  ᠠᠷᠠᠳ
        assert_eq!(collapse(&[A, O, Dd, B, O]), [A, O, O, A, B, O]); // Dd:medi
        assert_eq!(collapse(&[B, A, H, S, A]), [B, A, A, A, S, A]); // H:medi   ᠪᠠᠭᠰᠢ
        assert_eq!(collapse(&[A, A, R, Hx, A, L]), [A, A, R, N, N, A, L]); // Hx:medi  ᠠᠷᠭᠠᠯ
        assert_eq!(collapse(&[Cr, Nirugu]), [O, O, Nirugu]); // Cr:init  ᡂ᠊
    }

    #[test]
    fn the_four_contracting_duplicates_collapse_by_position() {
        assert_eq!(collapse(&[A, Aa]), [A]); // A:isol   ᠡ / ᠡᠠ᠋
        assert_eq!(collapse(&[B, A, Aa]), [B, Aa]); // Aa:fina  ᠪᠠ / ᠪᠠᠠ᠋
        assert_eq!(collapse(&[Nirugu, O, Aa]), [Nirugu, B2]); // B2:fina  ᠊ᠪ᠋ / ᠊ᠤᠠ᠋
        assert_eq!(collapse(&[Nirugu, I, Aa]), [Nirugu, G]); // G:fina   ᠊ᠭ / ᠊ᠢᠠ᠋
    }

    #[test]
    fn contraction_needs_the_position_the_pair_was_verified_in() {
        // `B2` and `G` are duplicates only at `fina`; unpadded, the pair's `O`/`I` is `init` and
        // the merged unit would land at `isol`, which is a different form (and `B2:isol` does not
        // exist at all).
        assert_eq!(collapse(&[O, Aa]), [O, Aa]);
        assert_eq!(collapse(&[I, Aa]), [I, Aa]);
        // A right-hand joiner makes the `Aa` medial, so the chain does not end there.
        assert_eq!(collapse(&[A, Aa, Zwj]), [A, Aa, Zwj]);
        // `Aa` alone is `Aa:isol` (chachlag), not the tail of a pair.
        assert_eq!(collapse(&[Aa]), [Aa]);
        assert_eq!(collapse(&[Mvs, Aa]), [Mvs, Aa]);
        // A lone `Cr` chain is `isol`, not the verified `init`.
        assert_eq!(collapse(&[Cr]), [Cr]);
    }

    #[test]
    fn initial_and_final_h_and_hx_are_distinct_ink_and_stay() {
        assert_eq!(collapse(&[H, O, D, A]), [H, O, D, A]); // H:init
        assert_eq!(collapse(&[T, A, H]), [T, A, H]); // H:fina
        assert_eq!(collapse(&[Hx, A]), [Hx, A]); // Hx:init
        assert_eq!(collapse(&[A, N, A, Hx]), [A, N, A, Hx]); // Hx:fina
    }

    #[test]
    fn structural_units_split_chains_and_joiners_pad_them() {
        // A word-final H before an MVS suffix is final in its own chain, not medial in the word.
        assert_eq!(collapse(&[T, A, H, Mvs, Aa]), [T, A, H, Mvs, Aa]);
        // A ZWJ after H joins it onward, so it is medial and a duplicate.
        assert_eq!(collapse(&[T, A, H, Zwj]), [T, A, A, A, Zwj]);
        assert_eq!(collapse(&[Nirugu, H, A]), [Nirugu, A, A, A]);
        // Dd is a duplicate in every position it exists in.
        assert_eq!(collapse(&[M, O, Dd, Mvs, Aa]), [M, O, O, A, Mvs, Aa]);
        // Each chain contracts on its own.
        assert_eq!(collapse(&[B, A, Aa, Mvs, A, Aa]), [B, Aa, Mvs, A]);
    }

    #[test]
    fn final_a_aa_requires_an_immediately_preceding_bowed_unit() {
        use WrittenUnit::{Gx, F, K, K2, P};
        for bowed_unit in [B, P, F, G, Gx, K, K2] {
            assert_eq!(collapse(&[bowed_unit, A, Aa]), [bowed_unit, Aa]);
            assert_eq!(collapse(&[N, bowed_unit, A, Aa]), [N, bowed_unit, Aa]);
            assert_eq!(collapse(&[bowed_unit, A, A, Aa]), [bowed_unit, A, A, Aa]);
        }
        for non_bowed_unit in [A, N, O, I, B2, D, R] {
            assert_eq!(collapse(&[non_bowed_unit, A, Aa]), [non_bowed_unit, A, Aa]);
        }
        assert_eq!(collapse(&[B, Zwj, A, Aa]), [B, Zwj, A, Aa]);
        assert_eq!(collapse(&[Nirugu, A, Aa]), [Nirugu, A, Aa]);
        assert_eq!(collapse(&[B, A, Aa, Zwj]), [B, A, Aa, Zwj]);
        // Expansion must not manufacture permission to swallow a tooth.
        assert_eq!(collapse(&[B, H, Aa]), [B, A, A, Aa]);
        assert_eq!(collapse(&[B, O, Dd, Aa]), [B, O, O, A, Aa]);
    }

    /// Termination and idempotence by exhaustion: every sequence of up to four units over the
    /// whole alphabet the rules touch, structural tokens included — 168 421 inputs. Expansion and
    /// contraction cannot fight each other into an oscillation or a runaway.
    #[test]
    fn collapse_is_idempotent_over_every_short_sequence() {
        use WrittenUnit::{Gx, F, K, K2, P};
        const ALPHABET: [WrittenUnit; 20] = [
            A, Aa, O, I, N, B, P, F, Gx, K, K2, B2, G, Dd, H, Hx, Cr, Nirugu, Zwj, Mvs,
        ];
        for length in 0..=4u32 {
            for index in 0..ALPHABET.len().pow(length) {
                let mut rest = index;
                let shape: Vec<WrittenUnit> = (0..length)
                    .map(|_| {
                        let unit = ALPHABET[rest % ALPHABET.len()];
                        rest /= ALPHABET.len();
                        unit
                    })
                    .collect();
                let once = collapse(&shape);
                assert_eq!(
                    collapse(&once),
                    once,
                    "not idempotent: {shape:?} -> {once:?}"
                );
                // Every unit expands to at most two, and contraction only shrinks.
                assert!(once.len() <= 2 * shape.len(), "runaway growth: {shape:?}");
                // `Dd` is a duplicate in both the positions it has, so it never survives.
                assert!(
                    !once.contains(&Dd),
                    "Dd survived the collapse: {shape:?} -> {once:?}"
                );
            }
        }
    }

    #[test]
    fn idempotent_and_identity_elsewhere() {
        for shape in [
            vec![A, A, R, Hx, A, L],
            vec![A, A, R, A, Dd],
            vec![Cr, Nirugu],
            vec![A, Aa],
            vec![B, A, A, Aa],
            vec![Nirugu, O, Aa],
            vec![Nirugu, I, Aa],
            vec![B, O, Dd, Aa],
            vec![T, A, H, Zwj],
        ] {
            let once = collapse(&shape);
            assert_eq!(collapse(&once), once, "not idempotent: {shape:?}");
        }
        assert_eq!(collapse(&[S, A, G, A]), [S, A, G, A]);
        assert_eq!(collapse(&[]), []);
        assert_eq!(collapse(&[Mvs]), [Mvs]);
    }
}
