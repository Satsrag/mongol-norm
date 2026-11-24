//! Chain-tail contraction (normalization side, `pub(crate)` only).
//!
//! Shaping (`crate::expand`) unifies all nine duplicate written units strictly by expansion
//! (`expand_pre` E1–E5 and `expand_post` E6–E9), so `Shaper::shape` never merges two letters.
//! Before table lookup in `crate::normalize`, the four chain-final expansions are contracted back
//! by the exact inverse rules C1–C4 so `MNG.normalize.json` and canonical Unicode output stay
//! unchanged (`mng-canonical/5`):
//!
//! | Rule | Tail pair | Contracts to | Condition (chain-final, `pad_right == 0`) |
//! |---|---|---|---|
//! | C1 | `A + Aa` | `A:isol` | Entire chain is `A Aa`, no adjacent joiner (`pad_left == 0`) |
//! | C2 | `<bowed> + A + Aa` | `<bowed> + Aa:fina` | Preceded immediately in the same chain by `<bowed>` |
//! | C3 | `O + Aa` | `B2:fina` | Merged unit lands at `fina` (`pad_left + len >= 3`) |
//! | C4 | `I + Aa` | `G:fina` | Merged unit lands at `fina` (`pad_left + len >= 3`) |
//!
//! On control-canonicalized sequences, `contract` and `crate::expand::expand_post` form an exact
//! 1-to-1 bijection (`contract(expand_post(s)) == contract(s)` and
//! `expand_post(contract(s)) == expand_post(s)`).

use crate::expand::{is_bowed, map_chains};
use crate::generated::enums::WrittenUnit;
use crate::normalize::slot_position;
use crate::tables::Position;

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

/// Contract the four chain-ending tail pairs (`A Aa`, `<bowed> A Aa`, `O Aa`, `I Aa`) for
/// normalization table lookup (rules C1–C4).
pub(crate) fn contract(shape: &[WrittenUnit]) -> Vec<WrittenUnit> {
    map_chains(shape, contract_chain)
}

/// Contract a chain-ending tail pair in one chain of letter units.
fn contract_chain(
    chain: &[WrittenUnit],
    _left_tok: Option<WrittenUnit>,
    pad_left: usize,
    pad_right: usize,
) -> Vec<WrittenUnit> {
    let position =
        |index: usize, len: usize| slot_position(index + pad_left, 1, len + pad_left + pad_right);

    let mut units = chain.to_vec();
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
    use crate::controls::canonical_controls;
    use crate::expand::expand_post;
    use WrittenUnit::{
        Aa, Cr, Dd, Gx, Hx, Mvs, Nirugu, Zwj, Zwnj, A, B, B2, D, F, G, H, I, K, K2, N, O, P, R,
    };

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

    /// Exact bijection between `expand_post` and `contract` on every control-canonicalized
    /// sequence over the full 21-unit alphabet up to length 4 (204,205 inputs) and over the
    /// core tail-interaction alphabet up to length 5 (161,051 inputs).
    #[test]
    fn expand_post_and_contract_form_exact_bijection() {
        const FULL_ALPHABET: [WrittenUnit; 21] = [
            A, Aa, O, I, N, B, P, F, Gx, K, K2, B2, G, Dd, H, Hx, Cr, Nirugu, Zwj, Zwnj, Mvs,
        ];
        for length in 0..=4u32 {
            for index in 0..FULL_ALPHABET.len().pow(length) {
                let mut rest = index;
                let raw: Vec<WrittenUnit> = (0..length)
                    .map(|_| {
                        let unit = FULL_ALPHABET[rest % FULL_ALPHABET.len()];
                        rest /= FULL_ALPHABET.len();
                        unit
                    })
                    .collect();
                let s = canonical_controls(&raw);
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
            }
        }

        const TAIL_ALPHABET: [WrittenUnit; 11] = [A, Aa, O, I, B, B2, G, Nirugu, Zwj, Zwnj, Mvs];
        for index in 0..TAIL_ALPHABET.len().pow(5) {
            let mut rest = index;
            let raw: Vec<WrittenUnit> = (0..5)
                .map(|_| {
                    let unit = TAIL_ALPHABET[rest % TAIL_ALPHABET.len()];
                    rest /= TAIL_ALPHABET.len();
                    unit
                })
                .collect();
            let s = canonical_controls(&raw);
            assert_eq!(
                contract(&expand_post(&s)),
                contract(&s),
                "len-5 contract(expand_post(s)) != contract(s) for s={s:?}"
            );
            assert_eq!(
                expand_post(&contract(&s)),
                expand_post(&s),
                "len-5 expand_post(contract(s)) != expand_post(s) for s={s:?}"
            );
        }
    }
}
