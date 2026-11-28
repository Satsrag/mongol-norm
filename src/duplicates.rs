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
//! Shaping (src/shaper.rs):        raw ─► expand (E1–E9) ─► canonical_controls
//! Normalization (src/normalize.rs): shape ─► contract (C1–C4) ─► encode & verify (MNG.normalize.json)
//! ```
//!
//! So the public [`Shaper::shape`](crate::Shaper::shape) — and everything built on it:
//! `same_shape`, `canonical_units`, `canonical_positioned_units`, `trace`, `FontShaper::shape` —
//! sees the expansion-only sequence and never merges two letters. The engine's own output, which
//! is what the standard's vectors describe, remains available as
//! [`Shaper::shape_raw`](crate::Shaper::shape_raw).
//!
//! # Shaping: single-pass pre-control expansion (`expand`, E1–E9)
//!
//! All nine duplicate rules unify in `shape` in one forward pass before
//! [`crate::controls::canonical_controls`], strictly in the 1 → 2 expansion direction:
//!
//! | Rule | Duplicate | Expands to | Condition | Witness |
//! |---|---|---|---|---|
//! | E1 | `Dd:medi` | `O A` | Always (`Dd` only occurs at `medi` / `fina`) | ᠣᠳᠪᠣ / ᠠ᠋ᠣᠣᠠᠪᠣ᠋ |
//! | E2 | `Dd:fina` | `O A` | Always | ᠠᠷᠠᠳ / ᠠᠷᠠᠤᠠ |
//! | E3 | `H:medi`  | `A A` | `medi` only; keep `H:init` and `H:fina` | ᠪᠠᠭᠰᠢ / ᠪᠠᠠᠠᠰᠢ |
//! | E4 | `Hx:medi` | `N N` | `medi` only; keep `Hx:init` and `Hx:fina` | ᠠᠷᠭᠠᠯ / ᠠ᠋ᠠᠷᠨ᠋ᠨ᠋ᠠᠯ |
//! | E5 | `Cr`      | `O O` | All positions (`isol` / `init` / `medi` / `fina`) | ᡂ᠊ / ᠤ᠋ᠤ᠊ |
//! | E6 | `G:fina`  | `I Aa` | All `fina` positions (including lone `Zwj G`) | ᠯᠭ᠌ / ᠯᠢᠡ᠋, ‍ᠭ / ᠢ᠋ᠠ᠋ |
//! | E7 | `B2`      | `O Aa` | All positions (including lone `Zwj B2`) | ᠊ᠪ᠋ / ᠊ᠤᠠ᠋, ‍ᠪ᠋ / ᠤ᠋ᠠ᠋ |
//! | E8 | `<bowed> [Zwj*] Aa:fina` | `<bowed> [Zwj*] A Aa` | Chain-final (`pad_right == 0`), immediately preceded by `<bowed>` in the same chain or across `Zwj+` (never crossing `Nirugu`/`Mvs`/`Zwnj`) | ᠪᠠ / ᠪᠠᠠ᠋ / ᠪ‍ᠠ᠋ |
//! | E9 | `A:isol`  | `A Aa` | Standalone `A` with no adjacent joiner (`pad_left == 0 && pad_right == 0`) | ᠡ / ᠡᠠ᠋ |
//!
//! Running all nine rules in a single pass *before* `canonical_controls` automatically lets the
//! existing ZWJ/ZWNJ rules handle expanded forms uniformly (for example `Zwj G → Zwj I Aa → I Aa`
//! and `Zwj B2 → Zwj O Aa → O Aa`), while [`crate::redundant_zwnj`]'s Z-Keep guard retains any
//! `Zwnj` between `<bowed>` and a chain-final `Aa` (even across `Zwj*`) so `canonical_controls`
//! never manufactures a new `<bowed> Aa:fina` ligature — keeping the two-stage pipeline strictly
//! idempotent in one pass with no outer fixpoint loop and all 8 ZWJ/ZWNJ tables untouched.
//!
//! # Normalization: chain-tail contraction (`contract`, C1–C4)
//!
//! Called only by `crate::normalize` before table lookup (never by shaping), contracting the four
//! chain-final expansions back to their compact `MNG.normalize.json` forms (`mng-canonical/6`):
//!
//! | Rule | Tail pair | Contracts to | Condition (chain-final, `pad_right == 0`) |
//! |---|---|---|---|
//! | C1 | `A + Aa` | `A:isol` | Entire chain is `A Aa`, no adjacent joiner (`pad_left == 0`) |
//! | C2 | `<bowed> + A + Aa` | `<bowed> + Aa:fina` | Preceded immediately in the same chain by `<bowed>` |
//! | C3 | `O + Aa` | `B2:fina` (or `Zwj + B2` if standalone) | Always at chain tail (`pad_right == 0`) |
//! | C4 | `I + Aa` | `G:fina` (or `Zwj + G` if standalone) | Always at chain tail (`pad_right == 0`) |
//!
//! For C3 and C4, when `O + Aa` or `I + Aa` stands alone as a 2-unit chain with no left joiner
//! (`chain.len() == 2 && pad_left == 0`, e.g. word start or directly after `Mvs` / `Zwnj`),
//! `contract` prepends `Zwj` so `B2` / `G` lands at `Position::Fina`. This keeps suffix encoding
//! uniform (`‍ᠪ᠋` / `‍ᠭ᠌`, matching `...ᠪ᠋` / `...ᠭ᠌` in longer chains) rather than switching to the
//! uncommon `ᠤ᠋ᠠ᠋` / `ᠢ᠋ᠠ᠋` spelling. Because `expand` unconditionally expands `B2` and `G:fina`
//! and `canonical_controls` drops the leading `Zwj` before `O`/`I`, `canonical_units(&contract(&s)) == s`
//! still holds across every boundary (`None`, `Mvs`, `Zwnj`).
//!
//! # Bowed written units, termination, and round-trip
//!
//! The Hudum bowed written units are `B`, `P`, `F`, `G`, `Gx`, `K`, `K2`: see the Hudum
//! written-unit ligated variants at <https://mongfontbuilder.pages.dev/hudum/>, mongfontbuilder
//! `data/ligatures.ts` (required `BAa`/`PAa`/`FAa`/`GAa`/`GxAa`/`KAa`/`K2Aa`), and this crate's
//! `rules::iii5_post_bowed_at`.
//!
//! `expand` eliminates all `Dd`, `Cr`, `B2`, `H:medi`, `Hx:medi`, `G:fina`, `A:isol`, and
//! `<bowed> [Zwj*] Aa:fina` in one pass, and `canonical_controls` (with the Z-Keep guard) can
//! never recreate any of them. Hence `canonical_units(canonical_units(u)) == canonical_units(u)`
//! holds in one pass, and for every canonical output `s`, `canonical_units(&contract(&s)) == s`.

use crate::generated::enums::WrittenUnit;
use crate::normalize::{is_joiner, slot_position};
use crate::tables::Position;

/// Whether `unit` is one of the seven Hudum bowed written units (`B`, `P`, `F`, `G`, `Gx`, `K`,
/// `K2`) after which `Aa:fina` carries a tooth (`<bowed> + Aa:fina` ≡ `<bowed> + A:medi + Aa:fina`).
pub(crate) fn is_bowed(unit: WrittenUnit) -> bool {
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

/// The replacement for one unit at one position in `expand` (rules E1–E7).
///
/// `Dd`, `Cr`, and `B2` expand wherever they occur (`Dd` and `B2` exist only in positions where
/// they are duplicates, and `Cr` is a duplicate in every position); `H` and `Hx` expand in medial
/// position only (their initial and final forms are distinct ink); `G` expands in final position
/// only (unconditionally covering all `G:fina`, including lone `Zwj + G`).
fn unit_expansion(unit: WrittenUnit, position: Position) -> Option<&'static [WrittenUnit]> {
    match (unit, position) {
        (WrittenUnit::Dd, _) => Some(&[WrittenUnit::O, WrittenUnit::A]),
        (WrittenUnit::H, Position::Medi) => Some(&[WrittenUnit::A, WrittenUnit::A]),
        (WrittenUnit::Hx, Position::Medi) => Some(&[WrittenUnit::N, WrittenUnit::N]),
        (WrittenUnit::Cr, _) => Some(&[WrittenUnit::O, WrittenUnit::O]),
        (WrittenUnit::G, Position::Fina) => Some(&[WrittenUnit::I, WrittenUnit::Aa]),
        (WrittenUnit::B2, _) => Some(&[WrittenUnit::O, WrittenUnit::Aa]),
        _ => None,
    }
}

/// Whether `prefix` ends with one or more `Zwj` units immediately preceded by a bowed written unit
/// (never crossing `Nirugu`, `Mvs`, or `Zwnj`).
fn preceded_by_bowed_through_zwj(prefix: &[WrittenUnit]) -> bool {
    let mut i = prefix.len();
    while i > 0 && prefix[i - 1] == WrittenUnit::Zwj {
        i -= 1;
    }
    i > 0 && is_bowed(prefix[i - 1])
}

/// Expand all nine visual duplicate encodings (rules E1–E9) in a single pass into one flat `Vec`.
///
/// Runs in shaping before [`crate::controls::canonical_controls`], so all expanded units —
/// including `G:fina` (`I Aa`), `B2` (`O Aa`), `<bowed> [Zwj*] Aa:fina` (`<bowed> [Zwj*] A Aa`),
/// and `A:isol` (`A Aa`) — are already in place when redundant ZWJ/ZWNJ rules inspect their
/// neighbors.
pub(crate) fn expand(shape: &[WrittenUnit]) -> Vec<WrittenUnit> {
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
        let left_tok = start.checked_sub(1).map(|i| shape[i]);
        let pad_left = usize::from(left_tok.is_some_and(is_joiner));
        let pad_right = usize::from(end < shape.len() && is_joiner(shape[end]));

        let out_start = out.len();
        for (index, &unit) in chain.iter().enumerate() {
            match unit_expansion(
                unit,
                chain_position(index, chain.len(), pad_left, pad_right),
            ) {
                Some(expanded) => out.extend_from_slice(expanded),
                None => out.push(unit),
            }
        }

        if pad_right == 0 {
            let new_len = out.len() - out_start;
            let last_idx = out.len() - 1;
            match out[last_idx] {
                // E8: <bowed> [Zwj*] Aa:fina → <bowed> [Zwj*] A + Aa.
                WrittenUnit::Aa
                    if (new_len >= 2 && is_bowed(out[last_idx - 1]))
                        || (new_len == 1
                            && left_tok == Some(WrittenUnit::Zwj)
                            && preceded_by_bowed_through_zwj(&out[..out_start])) =>
                {
                    out[last_idx] = WrittenUnit::A;
                    out.push(WrittenUnit::Aa);
                }
                // E9: A:isol → A + Aa (standalone A with no adjacent joiner).
                WrittenUnit::A if pad_left == 0 && new_len == 1 => {
                    out.push(WrittenUnit::Aa);
                }
                _ => {}
            }
        }
        start = end;
    }
    out
}

/// Contract the four chain-ending tail pairs (`A Aa`, `<bowed> A Aa`, `O Aa`, `I Aa`) for
/// normalization table lookup (rules C1–C4).
///
/// Called only by `crate::normalize`, never by shaping. For C3 (`O Aa`) and C4 (`I Aa`) with
/// `pad_right == 0`, a standalone 2-unit chain (`pad_left == 0 && chain.len() == 2`) contracts to
/// `[Zwj, B2]` / `[Zwj, G]` so the merged unit lands at `Position::Fina` and encodes uniformly as
/// `‍ᠪ᠋` / `‍ᠭ᠌`.
pub(crate) fn contract(shape: &[WrittenUnit]) -> Vec<WrittenUnit> {
    let mut out = Vec::with_capacity(shape.len());
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
        let out_start = out.len();
        out.extend_from_slice(chain);
        // Every contraction requires the pair to end the chain with no right joiner. Inspect that
        // pair once; a newly merged Aa must not be fed back to consume another A.
        if pad_right == 0 {
            if let Some(index) = chain.len().checked_sub(2) {
                if chain[index + 1] == WrittenUnit::Aa {
                    let result_pos = chain_position(index, chain.len() - 1, pad_left, 0);
                    let preceding = index.checked_sub(1).map(|previous| chain[previous]);
                    match (chain[index], result_pos) {
                        // C1: standalone A + Aa → A:isol.
                        (WrittenUnit::A, Position::Isol) => {
                            out[out_start + index] = WrittenUnit::A;
                            out.pop();
                        }
                        // C2: <bowed> + A + Aa → <bowed> + Aa:fina.
                        (WrittenUnit::A, Position::Fina) if preceding.is_some_and(is_bowed) => {
                            out[out_start + index] = WrittenUnit::Aa;
                            out.pop();
                        }
                        // C3: O + Aa → B2:fina (prepend Zwj when standalone so B2 lands at fina).
                        (WrittenUnit::O, Position::Fina) => {
                            out[out_start + index] = WrittenUnit::B2;
                            out.pop();
                        }
                        (WrittenUnit::O, Position::Isol) => {
                            out[out_start] = WrittenUnit::Zwj;
                            out[out_start + 1] = WrittenUnit::B2;
                        }
                        // C4: I + Aa → G:fina (prepend Zwj when standalone so G lands at fina).
                        (WrittenUnit::I, Position::Fina) => {
                            out[out_start + index] = WrittenUnit::G;
                            out.pop();
                        }
                        (WrittenUnit::I, Position::Isol) => {
                            out[out_start] = WrittenUnit::Zwj;
                            out[out_start + 1] = WrittenUnit::G;
                        }
                        _ => {}
                    }
                }
            }
        }
        start = end;
    }
    out
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

    /// Assert that `units` has no residual E1–E9 expansion target anywhere.
    fn assert_expansion_complete(units: &[WrittenUnit], context: &[WrittenUnit]) {
        assert!(
            !units.contains(&Dd) && !units.contains(&Cr) && !units.contains(&B2),
            "Dd, Cr, or B2 survived canonical_units: {context:?} -> {units:?}"
        );
        let mut start = 0;
        while start < units.len() {
            if units[start].is_structural() {
                start += 1;
                continue;
            }
            let mut end = start;
            while end < units.len() && !units[end].is_structural() {
                end += 1;
            }
            let chain = &units[start..end];
            let left_tok = start.checked_sub(1).map(|i| units[i]);
            let pad_left = usize::from(left_tok.is_some_and(is_joiner));
            let pad_right = usize::from(end < units.len() && is_joiner(units[end]));
            for (index, &unit) in chain.iter().enumerate() {
                let pos = chain_position(index, chain.len(), pad_left, pad_right);
                assert!(
                    !(unit == H && pos == Position::Medi),
                    "H:medi survived: {context:?} -> {units:?}"
                );
                assert!(
                    !(unit == Hx && pos == Position::Medi),
                    "Hx:medi survived: {context:?} -> {units:?}"
                );
                assert!(
                    !(unit == G && pos == Position::Fina),
                    "G:fina survived: {context:?} -> {units:?}"
                );
                assert!(
                    !(unit == A && pos == Position::Isol),
                    "A:isol survived: {context:?} -> {units:?}"
                );
            }
            if pad_right == 0 && chain.last() == Some(&Aa) {
                let same_chain_bowed = chain.len() >= 2 && is_bowed(chain[chain.len() - 2]);
                let across_zwj_bowed = chain.len() == 1
                    && left_tok == Some(Zwj)
                    && preceded_by_bowed_through_zwj(&units[..start]);
                assert!(
                    !same_chain_bowed && !across_zwj_bowed,
                    "<bowed>[Zwj*]Aa:fina survived: {context:?} -> {units:?}"
                );
            }
            start = end;
        }
    }

    #[test]
    fn expand_covers_e1_through_e9_by_position_and_context() {
        // E1–E5
        assert_eq!(expand(&[A, A, R, A, Dd]), [A, A, R, A, O, A]); // E2 Dd:fina ᠠᠷᠠᠳ
        assert_eq!(expand(&[A, O, Dd, B, O]), [A, O, O, A, B, O]); // E1 Dd:medi
        assert_eq!(expand(&[B, A, H, S, A]), [B, A, A, A, S, A]); // E3 H:medi  ᠪᠠᠭᠰᠢ
        assert_eq!(expand(&[A, A, R, Hx, A, L]), [A, A, R, N, N, A, L]); // E4 Hx:medi ᠠᠷᠭᠠᠯ
        assert_eq!(expand(&[Cr]), [O, O]); // E5 Cr:isol
        assert_eq!(expand(&[Cr, Nirugu]), [O, O, Nirugu]); // E5 Cr:init ᡂ᠊
        assert_eq!(expand(&[Nirugu, Cr, Nirugu]), [Nirugu, O, O, Nirugu]); // E5 Cr:medi
        assert_eq!(expand(&[Nirugu, Cr]), [Nirugu, O, O]); // E5 Cr:fina

        // E6: G:fina → I + Aa (unconditionally for all Fina, including lone Zwj+G)
        assert_eq!(expand(&[L, G]), [L, I, Aa]);
        assert_eq!(expand(&[Nirugu, G]), [Nirugu, I, Aa]);
        assert_eq!(expand(&[Zwj, N, G]), [Zwj, N, I, Aa]);
        assert_eq!(expand(&[Zwj, G]), [Zwj, I, Aa]); // lone Zwj+G expands
        assert_eq!(expand(&[G]), [G]); // G:isol kept
        assert_eq!(expand(&[L, G, Zwj]), [L, G, Zwj]); // G:medi kept

        // E7: B2 → O + Aa (unconditionally across all positions, including lone Zwj+B2)
        assert_eq!(expand(&[A, B2]), [A, O, Aa]);
        assert_eq!(expand(&[Nirugu, B2]), [Nirugu, O, Aa]);
        assert_eq!(expand(&[Zwj, N, B2]), [Zwj, N, O, Aa]);
        assert_eq!(expand(&[Zwj, B2]), [Zwj, O, Aa]); // lone Zwj+B2 expands
        assert_eq!(expand(&[B2]), [O, Aa]); // B2:isol expands
        assert_eq!(expand(&[A, B2, Zwj]), [A, O, Aa, Zwj]); // B2:medi expands

        // E8: <bowed> [Zwj*] Aa:fina → <bowed> [Zwj*] A + Aa
        for bowed_unit in [B, P, F, G, Gx, K, K2] {
            assert_eq!(expand(&[bowed_unit, Aa]), [bowed_unit, A, Aa]);
            assert_eq!(expand(&[N, bowed_unit, Aa]), [N, bowed_unit, A, Aa]);
            assert_eq!(expand(&[bowed_unit, A, Aa]), [bowed_unit, A, Aa]);
            assert_eq!(expand(&[bowed_unit, Zwj, Aa]), [bowed_unit, Zwj, A, Aa]);
            assert_eq!(
                expand(&[bowed_unit, Zwj, Zwj, Aa]),
                [bowed_unit, Zwj, Zwj, A, Aa]
            );
            // Never crosses Nirugu, Mvs, or Zwnj.
            assert_eq!(expand(&[bowed_unit, Nirugu, Aa]), [bowed_unit, Nirugu, Aa]);
            assert_eq!(
                expand(&[bowed_unit, Nirugu, Zwj, Aa]),
                [bowed_unit, Nirugu, Zwj, Aa]
            );
            assert_eq!(expand(&[bowed_unit, Mvs, Aa]), [bowed_unit, Mvs, Aa]);
            assert_eq!(
                expand(&[bowed_unit, Mvs, Zwj, Aa]),
                [bowed_unit, Mvs, Zwj, Aa]
            );
            assert_eq!(expand(&[bowed_unit, Zwnj, Aa]), [bowed_unit, Zwnj, Aa]);
            assert_eq!(
                expand(&[bowed_unit, Zwnj, Zwj, Aa]),
                [bowed_unit, Zwnj, Zwj, Aa]
            );
        }
        for non_bowed_unit in [A, N, O, I, D, R] {
            assert_eq!(expand(&[non_bowed_unit, Aa]), [non_bowed_unit, Aa]);
            assert_eq!(
                expand(&[non_bowed_unit, Zwj, Aa]),
                [non_bowed_unit, Zwj, Aa]
            );
        }
        assert_eq!(expand(&[Nirugu, Aa]), [Nirugu, Aa]);
        assert_eq!(expand(&[Zwj, Aa]), [Zwj, Aa]);
        assert_eq!(expand(&[B, Aa, Zwj]), [B, Aa, Zwj]);
        assert_eq!(expand(&[Aa]), [Aa]);
        assert_eq!(expand(&[Mvs, Aa]), [Mvs, Aa]);

        // E9: A:isol → A + Aa
        assert_eq!(expand(&[A]), [A, Aa]);
        assert_eq!(expand(&[Mvs, A]), [Mvs, A, Aa]);
        assert_eq!(expand(&[B, Zwnj, A]), [B, Zwnj, A, Aa]);
        assert_eq!(expand(&[A, Aa]), [A, Aa]);
        assert_eq!(expand(&[A, Zwj]), [A, Zwj]);
        assert_eq!(expand(&[Zwj, A]), [Zwj, A]);
        assert_eq!(expand(&[Nirugu, A]), [Nirugu, A]);
        assert_eq!(expand(&[A, Nirugu]), [A, Nirugu]);
    }

    #[test]
    fn initial_and_final_h_and_hx_are_distinct_ink_and_stay() {
        assert_eq!(canonical_units(&[H, O, D, A]), [H, O, D, A]); // H:init
        assert_eq!(canonical_units(&[T, A, H]), [T, A, H]); // H:fina
        assert_eq!(canonical_units(&[Hx, A]), [Hx, A]); // Hx:init
        assert_eq!(canonical_units(&[A, N, A, Hx]), [A, N, A, Hx]); // Hx:fina
    }

    #[test]
    fn c1_through_c4_contract_by_position_and_context() {
        assert_eq!(contract(&[A, Aa]), [A]); // C1 A:isol
        assert_eq!(contract(&[B, A, Aa]), [B, Aa]); // C2 <bowed>+Aa:fina
        assert_eq!(contract(&[Nirugu, O, Aa]), [Nirugu, B2]); // C3 B2:fina
        assert_eq!(contract(&[Nirugu, I, Aa]), [Nirugu, G]); // C4 G:fina

        // Standalone O+Aa / I+Aa prepend Zwj so the contracted B2/G lands at fina (suffix stability).
        assert_eq!(contract(&[O, Aa]), [Zwj, B2]);
        assert_eq!(contract(&[I, Aa]), [Zwj, G]);
        assert_eq!(contract(&[Mvs, O, Aa]), [Mvs, Zwj, B2]);
        assert_eq!(contract(&[Mvs, I, Aa]), [Mvs, Zwj, G]);
        assert_eq!(contract(&[A, H, Zwnj, I, Aa]), [A, H, Zwnj, Zwj, G]);
        assert_eq!(contract(&[I, Aa, Zwnj, B]), [Zwj, G, Zwnj, B]);
        assert_eq!(contract(&[O, Aa, Zwnj, Nirugu]), [Zwj, B2, Zwnj, Nirugu]);
        // Right-hand joiner keeps Aa medial (no contraction).
        assert_eq!(contract(&[A, Aa, Zwj]), [A, Aa, Zwj]);
        assert_eq!(contract(&[B, A, Aa, Zwj]), [B, A, Aa, Zwj]);
        assert_eq!(contract(&[O, Aa, Zwj]), [O, Aa, Zwj]);
        assert_eq!(contract(&[I, Aa, Zwj]), [I, Aa, Zwj]);
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
        assert_eq!(canonical_units(&[B, H, Zwj, Aa]), [B, A, A, Aa]);
        assert_eq!(canonical_units(&[B, Dd, Zwj, Aa]), [B, O, A, Aa]);
        assert_eq!(canonical_units(&[S, A, G, A]), [S, A, G, A]);
        assert_eq!(canonical_units(&[]), []);
        assert_eq!(canonical_units(&[Mvs]), [Mvs]);
    }

    #[test]
    fn single_pass_edge_cases_zwj_g_b2_and_bowed_zwnj_aa() {
        // Lone Zwj+G / Zwj+B2 expand via E6/E7 and drop the now-redundant leading Zwj.
        assert_eq!(canonical_units(&[Zwj, G]), [I, Aa]);
        assert_eq!(canonical_units(&[Zwj, B2]), [O, Aa]);
        // Bowed + Zwj* + Aa:fina expands via E8 and drops interior Zwj.
        assert_eq!(canonical_units(&[B, Zwj, Aa]), [B, A, Aa]);
        assert_eq!(canonical_units(&[B, Zwj, Zwj, Aa]), [B, A, Aa]);
        // Bowed + Zwnj + Aa: Z-Keep guard retains Zwnj so no tooth is manufactured.
        assert_eq!(canonical_units(&[B, Zwnj, Aa]), [B, Zwnj, Aa]);
        assert_eq!(
            canonical_units(&[A, B, Zwj, Zwnj, Aa]),
            [A, B, Zwj, Zwnj, Aa]
        );
        assert_eq!(canonical_units(&[B, Zwnj, Zwj, Aa]), [B, Zwnj, Aa]);
        assert_eq!(canonical_units(&[B, Nirugu, Zwj, Aa]), [B, Nirugu, Aa]);

        for bowed_unit in [B, P, F, G, Gx, K, K2] {
            assert_eq!(canonical_units(&[bowed_unit, Zwj, Aa]), [bowed_unit, A, Aa]);
            assert_eq!(
                canonical_units(&[bowed_unit, Zwnj, Aa]),
                [bowed_unit, Zwnj, Aa]
            );
            assert_eq!(
                canonical_units(&[A, bowed_unit, Zwj, Zwnj, Aa]),
                [A, bowed_unit, Zwj, Zwnj, Aa]
            );
            assert_eq!(
                canonical_units(&[bowed_unit, Zwnj, Zwj, Aa]),
                [bowed_unit, Zwnj, Aa]
            );
        }
    }

    /// Exhaustive single-pass invariant regression over every sequence of up to length 4 across
    /// the 21-unit alphabet (204,205 inputs) and length 5 across the 11-unit tail-interaction
    /// alphabet (161,051 inputs):
    /// 1. Strict idempotence: `canonical_units(&canonical_units(&u)) == canonical_units(&u)`.
    /// 2. Control completeness: `canonical_controls(&s) == s`.
    /// 3. Expansion completeness: no `Dd`, `Cr`, `B2`, `H:medi`, `Hx:medi`, `G:fina`, `A:isol`,
    ///    or `<bowed>[Zwj*]Aa:fina` survives (`expand(&s) == s`).
    /// 4. Contraction round-trip: `canonical_units(&contract(&s)) == s`.
    #[test]
    fn single_pass_invariants_hold_exhaustively() {
        let check = |input: &[WrittenUnit]| {
            let s = canonical_units(input);
            // 1. Strict single-pass idempotence.
            assert_eq!(
                canonical_units(&s),
                s,
                "canonical_units not idempotent: {input:?} -> {s:?}"
            );
            // 2. Control completeness.
            assert_eq!(
                canonical_controls(&s),
                s,
                "canonical_controls not stable on canonical output: {input:?} -> {s:?}"
            );
            // 3. Expansion completeness.
            assert_eq!(
                expand(&s),
                s,
                "expand not stable on canonical output: {input:?} -> {s:?}"
            );
            assert_expansion_complete(&s, input);
            // 4. Contraction round-trip.
            let contracted = contract(&s);
            assert_eq!(
                canonical_units(&contracted),
                s,
                "canonical_units(contract(s)) != s: {input:?} -> s={s:?} -> contracted={contracted:?}"
            );
        };

        for length in 0..=4u32 {
            for_each_sequence(&FULL_ALPHABET, length, check);
        }

        const TAIL_ALPHABET: [WrittenUnit; 11] = [A, Aa, O, I, B, B2, G, Nirugu, Zwj, Zwnj, Mvs];
        for_each_sequence(&TAIL_ALPHABET, 5, check);
    }

    /// Regression coverage for issue #76 and single-pass `expand` + Z-Keep word examples.
    #[test]
    fn issue_76_and_single_pass_examples_shape_and_normalize() {
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
            // ᠪᠠ / ᠪᠠᠠ᠋ / ᠪ‍ᠠ᠋ (1 tooth: ligature B+Aa ≡ B+A+Aa)
            (
                &[
                    "\u{182A}\u{1820}",
                    "\u{182A}\u{1820}\u{1820}\u{180B}",
                    "\u{182A}\u{200D}\u{1820}\u{180B}",
                ],
                &[B, A, Aa],
                "\u{182A}\u{1820}\u{180B}",
            ),
            // ᠪ‌ᠠ᠌ / ᠪ‌‍ᠠ (0 teeth: ZWNJ blocks the tooth-forming ligature and is kept)
            (
                &[
                    "\u{182A}\u{200C}\u{1820}\u{180C}",
                    "\u{182A}\u{200C}\u{200D}\u{1820}",
                ],
                &[B, Zwnj, Aa],
                "\u{182A}\u{200C}\u{1820}\u{180C}",
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
            // ‍ᠭ / ‍ᠭ᠌ / ‍ᠢᠠ᠋ / ᠢ᠋ᠠ᠋ → shape [I, Aa], norm "‍ᠭ᠌" (C4 standalone → [Zwj, G])
            (
                &[
                    "\u{200D}\u{182D}",
                    "\u{200D}\u{182D}\u{180C}",
                    "\u{200D}\u{1822}\u{1820}\u{180B}",
                    "\u{1822}\u{180B}\u{1820}\u{180B}",
                ],
                &[I, Aa],
                "\u{200D}\u{182D}\u{180C}",
            ),
            // ‍ᠪ᠋ / ‍ᠤᠠ᠋ / ᠤ᠋ᠠ᠋ → shape [O, Aa], norm "‍ᠪ᠋" (C3 standalone → [Zwj, B2])
            (
                &[
                    "\u{200D}\u{182A}\u{180B}",
                    "\u{200D}\u{1824}\u{1820}\u{180B}",
                    "\u{1824}\u{180B}\u{1820}\u{180B}",
                ],
                &[O, Aa],
                "\u{200D}\u{182A}\u{180B}",
            ),
            // Bare ᠭ᠌ stays [G] → "ᠭ᠌"; bare ᠪ᠋ stays [B] → "ᠪ".
            (&["\u{182D}\u{180C}"], &[G], "\u{182D}\u{180C}"),
            (&["\u{182A}\u{180B}", "\u{182A}"], &[B], "\u{182A}"),
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

    /// `FontShaper` and `Shaper` agree unit-for-unit on `ᠭ`, `‍ᠭ`, `ᠪ᠋`, `‍ᠪ᠋`, and all 7 bowed
    /// consonants separated from `Aa` by `ZWNJ` (with and without `ZWJ`).
    #[cfg(feature = "font")]
    #[test]
    fn font_shaper_matches_rule_engine_on_expanded_and_z_keep_cases() {
        let engine = Shaper::new(Locale::Mng);
        let font = crate::FontShaper::new();

        // Bare and ZWJ-prefixed G / B2.
        for (word, expected) in [
            ("\u{182D}", &[Hx][..]),                    // ᠭ (isolated g → Hx)
            ("\u{182D}\u{180C}", &[G][..]),             // ᠭ᠌ (isolated G)
            ("\u{200D}\u{182D}", &[I, Aa][..]),         // ‍ᠭ (lone Zwj+G → I+Aa)
            ("\u{182A}\u{180B}", &[B][..]),             // ᠪ᠋ (isolated b+FVS1)
            ("\u{200D}\u{182A}\u{180B}", &[O, Aa][..]), // ‍ᠪ᠋ (lone Zwj+B2 → O+Aa)
        ] {
            assert_eq!(engine.shape(word).unwrap(), expected, "engine({word:?})");
            assert_eq!(font.shape(word).unwrap(), expected, "font({word:?})");
        }

        // All 7 Hudum bowed consonants with ZWNJ + Aa (isol a+FVS2 = U+1820 U+180C),
        // plus ZWJ+ZWNJ after initial A and ZWNJ+ZWJ before post-bowed a.
        let bowed_spellings: &[(WrittenUnit, &str)] = &[
            (B, "\u{182A}"),
            (P, "\u{182B}"),
            (F, "\u{1839}"),
            (G, "\u{182D}\u{180C}"),
            (Gx, "\u{182C}\u{180F}"),
            (K, "\u{183B}"),
            (K2, "\u{183A}"),
        ];
        for &(unit, bowed) in bowed_spellings {
            let zwnj_aa = format!("{bowed}\u{200C}\u{1820}\u{180C}");
            assert_eq!(
                engine.shape(&zwnj_aa).unwrap(),
                [unit, Zwnj, Aa],
                "engine({unit:?} + ZWNJ + Aa)"
            );
            assert_eq!(
                font.shape(&zwnj_aa).unwrap(),
                engine.shape(&zwnj_aa).unwrap(),
                "font vs engine({unit:?} + ZWNJ + Aa)"
            );

            let a_bowed_zwj_zwnj_aa =
                format!("\u{1820}\u{180B}{bowed}\u{200D}\u{200C}\u{1820}\u{180C}");
            assert_eq!(
                engine.shape(&a_bowed_zwj_zwnj_aa).unwrap(),
                [A, unit, Zwj, Zwnj, Aa],
                "engine(A + {unit:?} + ZWJ + ZWNJ + Aa)"
            );
            assert_eq!(
                font.shape(&a_bowed_zwj_zwnj_aa).unwrap(),
                engine.shape(&a_bowed_zwj_zwnj_aa).unwrap(),
                "font vs engine(A + {unit:?} + ZWJ + ZWNJ + Aa)"
            );

            let bowed_zwj_aa = format!("{bowed}\u{200D}\u{1820}\u{180B}");
            assert_eq!(
                engine.shape(&bowed_zwj_aa).unwrap(),
                [unit, A, Aa],
                "engine({unit:?} + ZWJ + Aa)"
            );
            assert_eq!(
                font.shape(&bowed_zwj_aa).unwrap(),
                engine.shape(&bowed_zwj_aa).unwrap(),
                "font vs engine({unit:?} + ZWJ + Aa)"
            );
        }

        // B + ZWNJ + ZWJ + a: post_bowed resolves a:fina → Aa, Z-Keep retains ZWNJ.
        let b_zwnj_zwj_a = "\u{182A}\u{200C}\u{200D}\u{1820}";
        assert_eq!(engine.shape(b_zwnj_zwj_a).unwrap(), [B, Zwnj, Aa]);
        assert_eq!(font.shape(b_zwnj_zwj_a).unwrap(), [B, Zwnj, Aa]);
    }
}
