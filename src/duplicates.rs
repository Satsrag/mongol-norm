//! Duplicate encodings: written units that render as exactly the same ink as a sequence of other
//! units.
//!
//! UTN #57 / GB/T 25914-2023 keep all nine as distinct written units — its EAC conformance
//! vectors spell ᠠᠷᠭᠠᠯ as `A A R Hx A L` — but in the reference font (Noto Sans Mongolian 3.002,
//! the UTN #57 reference build) each pair is the same ink. Left in, they break this crate's one
//! promise: ᠠᠷᠠᠳ and ᠠᠷᠠᠤᠠ are the same visible word yet shaped to `A A R A Dd` and `A A R A O A`,
//! so `same_shape` said `false` and `normalize` gave two canonical texts for one word.
//!
//! Shaping and normalization share the same rule definitions here, while keeping their call sites
//! strictly separated:
//!
//! ```text
//! Shaping (src/shaper.rs):        raw ─► expand_pre (E1–E5) ─► canonical_controls ─► expand_post (E6–E9)
//! Normalization (src/normalize.rs): shape ─► contract (C1–C4) ─► encode & verify (unchanged MNG.normalize.json)
//! ```
//!
//! So the public [`Shaper::shape`](crate::Shaper::shape) — and everything built on it:
//! `same_shape`, `canonical_units`, `canonical_positioned_units`, `trace`, `FontShaper::shape` —
//! sees the expansion-only sequence and never merges two letters. The engine's own output, which
//! is what the standard's vectors describe, remains available as
//! [`Shaper::shape_raw`](crate::Shaper::shape_raw).
//!
//! # Shaping: two expansion passes around control canonicalization
//!
//! All nine duplicate rules unify in `shape` in the same direction (1 → 2 expansion), split into
//! two passes around [`crate::controls::canonical_controls`]:
//!
//! ## `expand_pre` (E1–E5, before control canonicalization)
//!
//! | Rule | Duplicate | Expands to | Witness |
//! |---|---|---|---|
//! | E1 | `Dd:medi` | `O A` | |
//! | E2 | `Dd:fina` | `O A` | ᠠᠷᠠᠳ / ᠠᠷᠠᠤᠠ |
//! | E3 | `H:medi`  | `A A` | ᠪᠠᠭᠰᠢ |
//! | E4 | `Hx:medi` | `N N` | ᠠᠷᠭᠠᠯ |
//! | E5 | `Cr`      | `O O` | ᡂ᠊ / ᠤ᠋ᠤ᠊ |
//!
//! Running `expand_pre` first ensures `Dd` (`O A`) and `Cr` (`O O`) are already decomposed when
//! redundant ZWJ/ZWNJ rules inspect their neighbors.
//!
//! ## `expand_post` (E6–E9, after control canonicalization; chain-final only, `pad_right == 0`)
//!
//! | Rule | Duplicate | Expands to | Condition | Witness |
//! |---|---|---|---|---|
//! | E6 | `G:fina` | `I Aa` | Preceded by a letter in the same chain or `Nirugu` (not lone `Zwj G`) | ᠯᠭ᠌ / ᠯᠢᠡ᠋ |
//! | E7 | `B2:fina` | `O Aa` | Preceded by a letter in the same chain or `Nirugu` (not lone `Zwj B2`) | ᠊ᠪ᠋ / ᠊ᠤᠠ᠋ |
//! | E8 | `<bowed> Aa:fina` | `<bowed> A Aa` | Preceded immediately in the same chain by `<bowed>` | ᠪᠠ / ᠪᠠᠠ᠋ |
//! | E9 | `A:isol` | `A Aa` | Standalone `A` with no adjacent joiner (`pad_left == 0 && pad_right == 0`) | ᠡ / ᠡᠠ᠋ |
//!
//! Running `expand_post` *after* `canonical_controls` automatically catches control-hidden pairs
//! (like `ᠪ‍ᠠ᠋` / `ᠪ‌ᠠ᠌` → `B Aa` → `B A Aa`), while excluding lone `Zwj G` / `Zwj B2` guarantees
//! `expand_post` never introduces a new redundant control — so no fixpoint loop around controls is
//! needed and all 8 ZWJ/ZWNJ tables stay untouched.
//!
//! # Normalization: chain-tail contraction (`contract`, C1–C4)
//!
//! Called only by `crate::normalize` before table lookup (never by shaping), contracting the four
//! chain-final expansions back by their exact inverse rules so `MNG.normalize.json` and canonical
//! Unicode output stay unchanged (`mng-canonical/5`):
//!
//! | Rule | Tail pair | Contracts to | Condition (chain-final, `pad_right == 0`) |
//! |---|---|---|---|
//! | C1 | `A + Aa` | `A:isol` | Entire chain is `A Aa`, no adjacent joiner (`pad_left == 0`) |
//! | C2 | `<bowed> + A + Aa` | `<bowed> + Aa:fina` | Preceded immediately in the same chain by `<bowed>` |
//! | C3 | `O + Aa` | `B2:fina` | Merged unit lands at `fina` (`pad_left + len >= 3`) |
//! | C4 | `I + Aa` | `G:fina` | Merged unit lands at `fina` (`pad_left + len >= 3`) |
//!
//! # Bowed written units, termination, and bijection
//!
//! The Hudum bowed written units are `B`, `P`, `F`, `G`, `Gx`, `K`, `K2`: see the Hudum
//! written-unit ligated variants at <https://mongfontbuilder.pages.dev/hudum/>, mongfontbuilder
//! `data/ligatures.ts` (required `BAa`/`PAa`/`FAa`/`GAa`/`GxAa`/`KAa`/`K2Aa`), and this crate's
//! `rules::iii5_post_bowed_at`.
//!
//! `expand_pre` emits only `O`, `A`, and `N` (never `Dd`/`H`/`Hx`/`Cr`), and only inserts units,
//! so it never turns `init`/`fina` into `medi`. `expand_post` emits only `I Aa`, `O Aa`, or
//! `A Aa` at the end of a chain: none ends in `G`, `B2`, `<bowed> Aa`, or standalone `A`, and none
//! creates a new `expand_pre` target or changes control redundancy. The complete shaping pipeline
//! `expand_post(canonical_controls(expand_pre(raw)))` is therefore idempotent in one pass, and on
//! control-canonicalized sequences `expand_post` and `contract` form an exact 1-to-1 bijection
//! (`contract(expand_post(s)) == contract(s)` and `expand_post(contract(s)) == expand_post(s)`).

use crate::generated::enums::WrittenUnit;
use crate::normalize::{is_joiner, slot_position};
use crate::tables::Position;

/// Whether `unit` is one of the seven Hudum bowed written units (`B`, `P`, `F`, `G`, `Gx`, `K`,
/// `K2`) after which `Aa:fina` carries a tooth (`<bowed> + Aa:fina` ≡ `<bowed> + A:medi + Aa:fina`).
fn is_bowed(unit: WrittenUnit) -> bool {
    matches!(
        unit,
        WrittenUnit::B
            | WrittenUnit::P
            | WrittenUnit::F
            | WrittenUnit::G
            | WrittenUnit::Gx
            | WrittenUnit::K
            | WrittenUnit::K2
    )
}

/// Position of a unit at `index` within a letter chain of length `len` padded by adjacent joiners.
fn chain_position(index: usize, len: usize, pad_left: usize, pad_right: usize) -> Position {
    slot_position(index + pad_left, 1, len + pad_left + pad_right)
}

/// The replacement for one unit at one position in `expand_pre` (rules E1–E5).
///
/// `Dd` exists only in medial and final position and both are duplicates, and `Cr` is a duplicate
/// in every position, so both expand wherever they occur; `H` and `Hx` are duplicates in medial
/// position only — their initial and final forms are distinct ink and are kept.
fn pre_expansion(unit: WrittenUnit, position: Position) -> Option<&'static [WrittenUnit]> {
    match (unit, position) {
        (WrittenUnit::Dd, _) => Some(&[WrittenUnit::O, WrittenUnit::A]),
        (WrittenUnit::H, Position::Medi) => Some(&[WrittenUnit::A, WrittenUnit::A]),
        (WrittenUnit::Hx, Position::Medi) => Some(&[WrittenUnit::N, WrittenUnit::N]),
        (WrittenUnit::Cr, _) => Some(&[WrittenUnit::O, WrittenUnit::O]),
        _ => None,
    }
}

/// The single unit an adjacent tail pair contracts to (rules C1–C4).
///
/// `result` is the position the merged unit occupies in the shortened chain: `A Aa` becomes
/// `A:isol` when it is the whole chain (C1) and `Aa:fina` only when a bowed written unit
/// immediately precedes `A` in the same chain (C2). `O Aa` → `B2:fina` (C3) and `I Aa` → `G:fina`
/// (C4) apply only when the merged unit lands at `fina` (`pad_right == 0 && pad_left + len >= 3`).
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
        (WrittenUnit::A, Position::Fina) if preceding.is_some_and(is_bowed) => {
            Some(WrittenUnit::Aa)
        }
        (WrittenUnit::O, Position::Fina) => Some(WrittenUnit::B2),
        (WrittenUnit::I, Position::Fina) => Some(WrittenUnit::G),
        _ => None,
    }
}

/// Expand single-unit duplicates `Dd`, `Cr`, medial `H`, and medial `Hx` (rules E1–E5).
///
/// Runs in shaping before [`crate::controls::canonical_controls`] so that `Dd` (`O A`) and `Cr`
/// (`O O`) are already decomposed when redundant ZWJ/ZWNJ rules inspect their neighbors.
pub(crate) fn expand_pre(shape: &[WrittenUnit]) -> Vec<WrittenUnit> {
    map_chains(shape, expand_pre_chain)
}

/// Expand chain-final duplicates `G:fina`, `B2:fina`, `<bowed> + Aa:fina`, and standalone `A:isol`
/// (rules E6–E9).
///
/// Runs in shaping after [`crate::controls::canonical_controls`], so pairs exposed by removing
/// redundant controls are expanded uniformly, and lone `Zwj G` / `Zwj B2` are left intact so no
/// new redundant control is ever created.
pub(crate) fn expand_post(shape: &[WrittenUnit]) -> Vec<WrittenUnit> {
    map_chains(shape, expand_post_chain)
}

/// Contract the four chain-ending tail pairs (`A Aa`, `<bowed> A Aa`, `O Aa`, `I Aa`) for
/// normalization table lookup (rules C1–C4).
///
/// Called only by `crate::normalize`, never by shaping.
pub(crate) fn contract(shape: &[WrittenUnit]) -> Vec<WrittenUnit> {
    map_chains(shape, contract_chain)
}

/// Split `shape` at structural units and transform each maximal letter chain with `f`.
///
/// `f` receives `(chain, left_tok, pad_left, pad_right)` where `left_tok` is the immediately
/// preceding structural token (if any) and `pad_left` / `pad_right` are `1` when the adjacent
/// structural token is a joiner (`Nirugu` or `Zwj`) and `0` otherwise.
fn map_chains(
    shape: &[WrittenUnit],
    f: fn(&[WrittenUnit], Option<WrittenUnit>, usize, usize) -> Vec<WrittenUnit>,
) -> Vec<WrittenUnit> {
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
        let left_tok = start.checked_sub(1).map(|i| shape[i]);
        let pad_left = usize::from(left_tok.is_some_and(is_joiner));
        let pad_right = usize::from(end < shape.len() && is_joiner(shape[end]));
        out.extend(f(&shape[start..end], left_tok, pad_left, pad_right));
        start = end;
    }
    out
}

/// Expand E1–E5 in one chain of letter units.
fn expand_pre_chain(
    chain: &[WrittenUnit],
    _left_tok: Option<WrittenUnit>,
    pad_left: usize,
    pad_right: usize,
) -> Vec<WrittenUnit> {
    let mut units = Vec::with_capacity(chain.len() + 2);
    for (index, &unit) in chain.iter().enumerate() {
        match pre_expansion(
            unit,
            chain_position(index, chain.len(), pad_left, pad_right),
        ) {
            Some(expanded) => units.extend_from_slice(expanded),
            None => units.push(unit),
        }
    }
    units
}

/// Expand E6–E9 at the end of one chain of letter units.
fn expand_post_chain(
    chain: &[WrittenUnit],
    left_tok: Option<WrittenUnit>,
    pad_left: usize,
    pad_right: usize,
) -> Vec<WrittenUnit> {
    let mut units = Vec::with_capacity(chain.len() + 1);
    units.extend_from_slice(chain);
    if pad_right != 0 {
        return units;
    }
    let Some(&last) = units.last() else {
        return units;
    };
    let len = units.len();
    match last {
        // E6: G:fina → I + Aa (preceded by a letter in the same chain or Nirugu; not lone Zwj+G).
        WrittenUnit::G if len >= 2 || left_tok == Some(WrittenUnit::Nirugu) => {
            units[len - 1] = WrittenUnit::I;
            units.push(WrittenUnit::Aa);
        }
        // E7: B2:fina → O + Aa (preceded by a letter in the same chain or Nirugu; not lone Zwj+B2).
        WrittenUnit::B2 if len >= 2 || left_tok == Some(WrittenUnit::Nirugu) => {
            units[len - 1] = WrittenUnit::O;
            units.push(WrittenUnit::Aa);
        }
        // E8: <bowed> + Aa:fina → <bowed> + A + Aa.
        WrittenUnit::Aa if len >= 2 && is_bowed(units[len - 2]) => {
            units[len - 1] = WrittenUnit::A;
            units.push(WrittenUnit::Aa);
        }
        // E9: A:isol → A + Aa (standalone A with no adjacent joiner).
        WrittenUnit::A if len == 1 && pad_left == 0 => {
            units.push(WrittenUnit::Aa);
        }
        _ => {}
    }
    units
}

/// Contract a chain-ending tail pair in one chain of letter units (rules C1–C4).
fn contract_chain(
    chain: &[WrittenUnit],
    _left_tok: Option<WrittenUnit>,
    pad_left: usize,
    pad_right: usize,
) -> Vec<WrittenUnit> {
    let mut units = chain.to_vec();
    // Every contraction requires the pair to end the chain. Inspect that pair once;
    // a newly merged Aa must not be fed back to consume another A.
    if let Some(index) = units.len().checked_sub(2) {
        if let Some(merged) = contraction(
            units[index],
            units[index + 1],
            chain_position(index, units.len() - 1, pad_left, pad_right),
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
    use crate::controls::canonical_controls;
    use crate::shaper::canonical_units;
    use crate::{Locale, Shaper};
    use WrittenUnit::{
        Aa, Ch, Cr, Dd, Gx, Hx, Mvs, Nirugu, Zwj, Zwnj, A, B, B2, D, F, G, H, I, K, K2, L, M, N, O,
        P, R, S, T,
    };

    const FULL_ALPHABET: [WrittenUnit; 21] = [
        A, Aa, O, I, N, B, P, F, Gx, K, K2, B2, G, Dd, H, Hx, Cr, Nirugu, Zwj, Zwnj, Mvs,
    ];

    fn for_each_sequence(
        alphabet: &[WrittenUnit],
        length: u32,
        mut visit: impl FnMut(&[WrittenUnit]),
    ) {
        let mut seq = Vec::with_capacity(length as usize);
        for index in 0..alphabet.len().pow(length) {
            seq.clear();
            let mut rest = index;
            for _ in 0..length {
                seq.push(alphabet[rest % alphabet.len()]);
                rest /= alphabet.len();
            }
            visit(&seq);
        }
    }

    #[test]
    fn expand_pre_covers_e1_through_e5_by_position() {
        assert_eq!(expand_pre(&[A, A, R, A, Dd]), [A, A, R, A, O, A]); // E2 Dd:fina ᠠᠷᠠᠳ
        assert_eq!(expand_pre(&[A, O, Dd, B, O]), [A, O, O, A, B, O]); // E1 Dd:medi
        assert_eq!(expand_pre(&[B, A, H, S, A]), [B, A, A, A, S, A]); // E3 H:medi  ᠪᠠᠭᠰᠢ
        assert_eq!(expand_pre(&[A, A, R, Hx, A, L]), [A, A, R, N, N, A, L]); // E4 Hx:medi ᠠᠷᠭᠠᠯ
        assert_eq!(expand_pre(&[Cr]), [O, O]); // E5 Cr:isol
        assert_eq!(expand_pre(&[Cr, Nirugu]), [O, O, Nirugu]); // E5 Cr:init ᡂ᠊
        assert_eq!(expand_pre(&[Nirugu, Cr, Nirugu]), [Nirugu, O, O, Nirugu]); // E5 Cr:medi
        assert_eq!(expand_pre(&[Nirugu, Cr]), [Nirugu, O, O]); // E5 Cr:fina
    }

    #[test]
    fn initial_and_final_h_and_hx_are_distinct_ink_and_stay() {
        assert_eq!(canonical_units(&[H, O, D, A]), [H, O, D, A]); // H:init
        assert_eq!(canonical_units(&[T, A, H]), [T, A, H]); // H:fina
        assert_eq!(canonical_units(&[Hx, A]), [Hx, A]); // Hx:init
        assert_eq!(canonical_units(&[A, N, A, Hx]), [A, N, A, Hx]); // Hx:fina
    }

    #[test]
    fn expand_post_covers_e6_through_e9_and_edge_guards() {
        // E6: G:fina → I + Aa
        assert_eq!(expand_post(&[L, G]), [L, I, Aa]);
        assert_eq!(expand_post(&[Nirugu, G]), [Nirugu, I, Aa]);
        assert_eq!(expand_post(&[Zwj, N, G]), [Zwj, N, I, Aa]);
        assert_eq!(expand_post(&[Zwj, G]), [Zwj, G]); // lone Zwj+G kept
        assert_eq!(expand_post(&[G]), [G]); // G:isol kept
        assert_eq!(expand_post(&[L, G, Zwj]), [L, G, Zwj]); // pad_right != 0 kept

        // E7: B2:fina → O + Aa
        assert_eq!(expand_post(&[A, B2]), [A, O, Aa]);
        assert_eq!(expand_post(&[Nirugu, B2]), [Nirugu, O, Aa]);
        assert_eq!(expand_post(&[Zwj, N, B2]), [Zwj, N, O, Aa]);
        assert_eq!(expand_post(&[Zwj, B2]), [Zwj, B2]); // lone Zwj+B2 kept
        assert_eq!(expand_post(&[B2]), [B2]); // B2:isol kept
        assert_eq!(expand_post(&[A, B2, Zwj]), [A, B2, Zwj]); // pad_right != 0 kept

        // E8: <bowed> + Aa:fina → <bowed> + A + Aa
        for bowed_unit in [B, P, F, G, Gx, K, K2] {
            assert_eq!(expand_post(&[bowed_unit, Aa]), [bowed_unit, A, Aa]);
            assert_eq!(expand_post(&[N, bowed_unit, Aa]), [N, bowed_unit, A, Aa]);
            assert_eq!(expand_post(&[bowed_unit, A, Aa]), [bowed_unit, A, Aa]);
        }
        for non_bowed_unit in [A, N, O, I, B2, D, R] {
            assert_eq!(expand_post(&[non_bowed_unit, Aa]), [non_bowed_unit, Aa]);
        }
        assert_eq!(expand_post(&[B, Zwj, Aa]), [B, Zwj, Aa]);
        assert_eq!(expand_post(&[Nirugu, Aa]), [Nirugu, Aa]);
        assert_eq!(expand_post(&[B, Aa, Zwj]), [B, Aa, Zwj]);
        assert_eq!(expand_post(&[Aa]), [Aa]);
        assert_eq!(expand_post(&[Mvs, Aa]), [Mvs, Aa]);

        // E9: A:isol → A + Aa
        assert_eq!(expand_post(&[A]), [A, Aa]);
        assert_eq!(expand_post(&[Mvs, A]), [Mvs, A, Aa]);
        assert_eq!(expand_post(&[B, Zwnj, A]), [B, Zwnj, A, Aa]);
        assert_eq!(expand_post(&[A, Aa]), [A, Aa]);
        assert_eq!(expand_post(&[A, Zwj]), [A, Zwj]);
        assert_eq!(expand_post(&[Zwj, A]), [Zwj, A]);
        assert_eq!(expand_post(&[Nirugu, A]), [Nirugu, A]);
        assert_eq!(expand_post(&[A, Nirugu]), [A, Nirugu]);
    }

    #[test]
    fn c1_through_c4_contract_by_position_and_context() {
        assert_eq!(contract(&[A, Aa]), [A]); // C1 A:isol
        assert_eq!(contract(&[B, A, Aa]), [B, Aa]); // C2 <bowed>+Aa:fina
        assert_eq!(contract(&[Nirugu, O, Aa]), [Nirugu, B2]); // C3 B2:fina
        assert_eq!(contract(&[Nirugu, I, Aa]), [Nirugu, G]); // C4 G:fina

        // Unpadded O+Aa / I+Aa would land at isol, not fina.
        assert_eq!(contract(&[O, Aa]), [O, Aa]);
        assert_eq!(contract(&[I, Aa]), [I, Aa]);
        // Right-hand joiner keeps Aa medial.
        assert_eq!(contract(&[A, Aa, Zwj]), [A, Aa, Zwj]);
        assert_eq!(contract(&[B, A, Aa, Zwj]), [B, A, Aa, Zwj]);
        // Bare Aa is isol (chachlag), not a tail pair.
        assert_eq!(contract(&[Aa]), [Aa]);
        assert_eq!(contract(&[Mvs, Aa]), [Mvs, Aa]);

        // C2 requires an immediately preceding bowed unit in the same chain.
        for bowed_unit in [B, P, F, G, Gx, K, K2] {
            assert_eq!(contract(&[bowed_unit, A, Aa]), [bowed_unit, Aa]);
            assert_eq!(contract(&[N, bowed_unit, A, Aa]), [N, bowed_unit, Aa]);
            assert_eq!(contract(&[bowed_unit, A, A, Aa]), [bowed_unit, A, A, Aa]);
        }
        for non_bowed_unit in [A, N, O, I, B2, D, R] {
            assert_eq!(contract(&[non_bowed_unit, A, Aa]), [non_bowed_unit, A, Aa]);
        }
        assert_eq!(contract(&[B, Zwj, A, Aa]), [B, Zwj, A, Aa]);
        assert_eq!(contract(&[Nirugu, A, Aa]), [Nirugu, A, Aa]);
    }

    #[test]
    fn structural_units_split_chains_and_joiners_pad_them() {
        // Word-final H before MVS is final in its own chain, not medial.
        assert_eq!(canonical_units(&[T, A, H, Mvs, Aa]), [T, A, H, Mvs, Aa]);
        // ZWJ after H makes it medial, so E3 expands it.
        assert_eq!(canonical_units(&[T, A, H, Zwj]), [T, A, A, A, Zwj]);
        assert_eq!(canonical_units(&[Nirugu, H, A]), [Nirugu, A, A, A]);
        // Dd expands in every position it exists in.
        assert_eq!(canonical_units(&[M, O, Dd, Mvs, Aa]), [M, O, O, A, Mvs, Aa]);
        // Each chain expands on its own.
        assert_eq!(canonical_units(&[B, Aa, Mvs, A]), [B, A, Aa, Mvs, A, Aa]);
        // Expansion does not manufacture bowed context for E8.
        assert_eq!(canonical_units(&[B, H, Aa]), [B, A, A, Aa]);
        assert_eq!(canonical_units(&[B, O, Dd, Aa]), [B, O, O, A, Aa]);
        assert_eq!(canonical_units(&[S, A, G, A]), [S, A, G, A]);
        assert_eq!(canonical_units(&[]), []);
        assert_eq!(canonical_units(&[Mvs]), [Mvs]);
    }

    /// Control invariance and idempotence by exhaustion over every sequence of up to four units
    /// across all 21 units the rules touch (structural controls including `Zwnj` included —
    /// 204,205 inputs).
    #[test]
    fn expand_post_preserves_canonical_controls_and_canonical_units_is_idempotent() {
        for length in 0..=4u32 {
            for_each_sequence(&FULL_ALPHABET, length, |input| {
                // 1. Control invariance: for any control-canonicalized sequence `s`,
                //    `canonical_controls(&expand_post(&s)) == expand_post(&s)`.
                let controlled = canonical_controls(input);
                let post = expand_post(&controlled);
                assert_eq!(
                    canonical_controls(&post),
                    post,
                    "expand_post broke control invariance: {input:?} -> {controlled:?} -> {post:?}"
                );

                // 2. Idempotence of the full shaping pipeline:
                //    `canonical_units(&canonical_units(&u)) == canonical_units(&u)`.
                let once = canonical_units(input);
                assert_eq!(
                    canonical_units(&once),
                    once,
                    "canonical_units not idempotent: {input:?} -> {once:?}"
                );
                assert!(
                    !once.contains(&Dd) && !once.contains(&Cr),
                    "Dd or Cr survived canonical_units: {input:?} -> {once:?}"
                );
            });
        }
    }

    /// Exact bijection between `expand_post` and `contract` on every control-canonicalized
    /// sequence over the full 21-unit alphabet up to length 4 (204,205 inputs) and over the
    /// core tail-interaction alphabet up to length 5 (161,051 inputs).
    #[test]
    fn expand_post_and_contract_form_exact_bijection() {
        let assert_bijection = |raw: &[WrittenUnit]| {
            let s = canonical_controls(raw);
            assert_eq!(
                contract(&expand_post(&s)),
                contract(&s),
                "contract(expand_post(s)) != contract(s) for s={s:?}"
            );
            assert_eq!(
                expand_post(&contract(&s)),
                expand_post(&s),
                "expand_post(contract(s)) != expand_post(s) for s={s:?}"
            );
        };

        for length in 0..=4u32 {
            for_each_sequence(&FULL_ALPHABET, length, assert_bijection);
        }

        const TAIL_ALPHABET: [WrittenUnit; 11] = [A, Aa, O, I, B, B2, G, Nirugu, Zwj, Zwnj, Mvs];
        for_each_sequence(&TAIL_ALPHABET, 5, assert_bijection);
    }

    /// Regression coverage for every word example in issue #76.
    #[test]
    fn issue_76_examples_shape_expanded_and_normalize_unchanged() {
        let shaper = Shaper::new(Locale::Mng);
        let cases: &[(&[&str], &[WrittenUnit], &str)] = &[
            // ᠠᠷᠠᠳ
            (
                &["\u{1820}\u{1837}\u{1820}\u{1833}"],
                &[A, A, R, A, O, A],
                "\u{1820}\u{180B}\u{1820}\u{1837}\u{1820}\u{1823}\u{1820}\u{180C}",
            ),
            // ᠡ / ᠡᠠ᠋
            (
                &["\u{1821}", "\u{1821}\u{1820}\u{180B}"],
                &[A, Aa],
                "\u{1820}\u{180B}",
            ),
            // ᠪᠠ / ᠪᠠᠠ᠋ / ᠪ‍ᠠ᠋ / ᠪ‌ᠠ᠌
            (
                &[
                    "\u{182A}\u{1820}",
                    "\u{182A}\u{1820}\u{1820}\u{180B}",
                    "\u{182A}\u{200D}\u{1820}\u{180B}",
                    "\u{182A}\u{200C}\u{1820}\u{180C}",
                ],
                &[B, A, Aa],
                "\u{182A}\u{1820}\u{180B}",
            ),
            // ᠠ᠋ᠪ᠋ / ᠣᠠ᠋
            (
                &[
                    "\u{1820}\u{180B}\u{182A}\u{180B}",
                    "\u{1823}\u{1820}\u{180B}",
                ],
                &[A, O, Aa],
                "\u{1820}\u{180B}\u{182A}\u{180B}",
            ),
            // ᠊ᠪ᠋ / ᠊ᠤᠠ᠋
            (
                &[
                    "\u{180A}\u{182A}\u{180B}",
                    "\u{180A}\u{1824}\u{1820}\u{180B}",
                ],
                &[Nirugu, O, Aa],
                "\u{180A}\u{182A}\u{180B}",
            ),
            // ᠯᠭ᠌ / ᠯᠢᠡ᠋ / ᠯᠢ‍ᠡ᠋
            (
                &[
                    "\u{182F}\u{182D}\u{180C}",
                    "\u{182F}\u{1822}\u{1821}\u{180B}",
                    "\u{182F}\u{1822}\u{200D}\u{1821}\u{180B}",
                ],
                &[L, I, Aa],
                "\u{182F}\u{182D}\u{180C}",
            ),
            // ᠪᠢᠴᠢᠭ
            (
                &["\u{182A}\u{1822}\u{1834}\u{1822}\u{182D}"],
                &[B, I, Ch, I, I, Aa],
                "\u{182A}\u{1822}\u{180D}\u{1834}\u{1822}\u{180D}\u{182D}\u{180C}",
            ),
            // ‍ᠭ (lone Zwj+G kept)
            (&["\u{200D}\u{182D}"], &[Zwj, G], "\u{200D}\u{182D}\u{180C}"),
            // ‍ᠪ᠋ (lone Zwj+B2 kept)
            (
                &["\u{200D}\u{182A}\u{180B}"],
                &[Zwj, B2],
                "\u{200D}\u{182A}\u{180B}",
            ),
        ];

        for &(inputs, expected_shape, expected_norm) in cases {
            for &word in inputs {
                assert_eq!(
                    shaper.shape(word).unwrap(),
                    expected_shape,
                    "shape({word:?})"
                );
                let norm = shaper.normalize(word).unwrap();
                assert_eq!(norm, expected_norm, "normalize({word:?})");
                assert_eq!(
                    shaper.shape(&norm).unwrap(),
                    expected_shape,
                    "round-trip({word:?})"
                );
                assert!(shaper.same_shape(word, &norm).unwrap());
                assert_eq!(
                    shaper.normalize_written_units(expected_shape).unwrap(),
                    expected_norm,
                    "normalize_written_units({expected_shape:?})"
                );
            }
            for pair in inputs.windows(2) {
                assert!(
                    shaper.same_shape(pair[0], pair[1]).unwrap(),
                    "same_shape({:?}, {:?})",
                    pair[0],
                    pair[1]
                );
            }
        }
    }
}
