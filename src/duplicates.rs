//! Duplicate encodings: written units that render as exactly the same ink as a sequence of
//! other units, and are therefore folded out of the public shape.
//!
//! UTN #57 / GB/T 25914-2023 keep these as distinct written units — its EAC conformance vectors
//! spell ᠠᠷᠭᠠᠯ as `A A R Hx A L` — but in the reference font the ink is identical either way:
//! `Dd:medi` ≡ `O:medi A:medi`, `Dd:fina` ≡ `O:medi A:fina`, `H:medi` ≡ `A:medi A:medi`,
//! `Hx:medi` ≡ `N:medi N:medi` (pixel-compared in Noto Sans Mongolian 3.002, the UTN #57
//! reference build; each pair differs only at the anti-aliasing level, with identical advance).
//! Left in, they break this crate's one promise: ᠠᠷᠠᠳ and ᠠᠷᠠᠤᠠ are the same visible word yet
//! shaped to `A A R A Dd` and `A A R A O A`, so `same_shape` said `false` and `normalize` gave
//! two canonical texts for one word.
//!
//! So the public [`Shaper::shape`](crate::Shaper::shape) — and everything built on it:
//! `same_shape`, `normalize`, the written-unit encoders — sees the collapsed sequence, in which
//! none of the four ever appears. The engine's own output, which is what the standard's vectors
//! describe, remains available as [`Shaper::shape_raw`](crate::Shaper::shape_raw).
//!
//! Only these four are duplicates. Five more units that ZVVNMOD spells with two glyphs
//! (`Cr:init`, `B2:fina`, `G:fina`, `A:isol`, `Aa:fina`) are *not*: their expansions render at a
//! different width in the reference font, and two of them would expand into themselves.

use crate::generated::enums::WrittenUnit;
use crate::normalize::{is_joiner, slot_position};
use crate::tables::Position;

/// The replacement for one unit at one position, if it is a duplicate encoding.
///
/// `Dd` exists only in medial and final position and both are duplicates, so it collapses
/// wherever it occurs; `H` and `Hx` are duplicates in medial position only — their initial and
/// final forms are distinct ink and are kept.
fn expansion(unit: WrittenUnit, position: Position) -> Option<&'static [WrittenUnit]> {
    match (unit, position) {
        (WrittenUnit::Dd, _) => Some(&[WrittenUnit::O, WrittenUnit::A]),
        (WrittenUnit::H, Position::Medi) => Some(&[WrittenUnit::A, WrittenUnit::A]),
        (WrittenUnit::Hx, Position::Medi) => Some(&[WrittenUnit::N, WrittenUnit::N]),
        _ => None,
    }
}

/// Collapse every duplicate encoding in `shape`.
///
/// Positions are the ones `normalize` itself uses: slots within each chain between structural
/// units, with a nirugu or ZWJ neighbour padding the chain the way it pads the rendering, so a
/// unit joined across a ZWJ counts as medial exactly as the shaper rendered it. Idempotent — the
/// replacement units are never themselves duplicates.
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
        let chain = &shape[start..end];
        let pad_left = usize::from(start > 0 && is_joiner(shape[start - 1]));
        let pad_right = usize::from(end < shape.len() && is_joiner(shape[end]));
        let padded = chain.len() + pad_left + pad_right;
        for (index, &unit) in chain.iter().enumerate() {
            match expansion(unit, slot_position(index + pad_left, 1, padded)) {
                Some(units) => out.extend_from_slice(units),
                None => out.push(unit),
            }
        }
        start = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use WrittenUnit::{Aa, Dd, Hx, Mvs, Nirugu, Zwj, A, B, D, G, H, L, M, N, O, R, S, T};

    #[test]
    fn the_four_duplicates_collapse_by_position() {
        assert_eq!(collapse(&[A, A, R, A, Dd]), [A, A, R, A, O, A]); // Dd:fina  ᠠᠷᠠᠳ
        assert_eq!(collapse(&[A, O, Dd, B, O]), [A, O, O, A, B, O]); // Dd:medi
        assert_eq!(collapse(&[B, A, H, S, A]), [B, A, A, A, S, A]); // H:medi   ᠪᠠᠭᠰᠢ
        assert_eq!(collapse(&[A, A, R, Hx, A, L]), [A, A, R, N, N, A, L]); // Hx:medi  ᠠᠷᠭᠠᠯ
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
    }

    #[test]
    fn idempotent_and_identity_elsewhere() {
        let shape = [A, A, R, Hx, A, L];
        let once = collapse(&shape);
        assert_eq!(collapse(&once), once);
        assert_eq!(collapse(&[S, A, G, A]), [S, A, G, A]);
        assert_eq!(collapse(&[]), []);
        assert_eq!(collapse(&[Mvs]), [Mvs]);
    }
}
