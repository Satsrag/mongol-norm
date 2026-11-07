//! Redundant ZWNJ (U+200C): remove breaks whose affected letters keep equivalent positions.
//! Each call scans left to right once; the control policy repeats complete rounds until stable.

use crate::generated::enums::WrittenUnit;
use crate::normalize::{is_joiner, slot_position};
use crate::tables::Position;
use WrittenUnit::{
    Aa, Ch, Gx, Hx, Mvs, Rh, Sh, Zr, Zwnj, B, C, D, F, G, H, I, K, K2, L, M, N, O, P, R, S, T, U,
    W, Y, Z,
};

/// Right neighbor `… Zwnj U X…` → `… U X…` (`U:init` → `U:medi`).
pub(crate) const INIT_TO_MEDI: &[WrittenUnit] = &[
    B, C, Ch, D, F, G, Gx, I, K, K2, O, P, R, Rh, S, Sh, T, W, Y, Z, Zr,
];

/// Left neighbor `…X U Zwnj …` → `…X U …` (`U:fina` → `U:medi`).
pub(crate) const FINA_TO_MEDI: &[WrittenUnit] = &[O, Zr];

/// Right neighbor `… Zwnj U` with `U` alone on the right → `… U` (`U:isol` → `U:fina`).
pub(crate) const ISOL_TO_FINA: &[WrittenUnit] = &[Aa, I, U, Zr];

/// Left neighbor `U Zwnj …` with `U` alone on the left → `U …` (`U:isol` → `U:init`).
pub(crate) const ISOL_TO_INIT: &[WrittenUnit] = &[
    B, C, Ch, D, F, G, Gx, H, Hx, K, K2, L, M, N, P, R, Rh, S, Sh, T, W, Y, Z, Zr,
];

/// Remove redundant `Zwnj` units in one left-to-right scan, without revisiting kept breaks.
pub(crate) fn drop_redundant_zwnj(shape: &[WrittenUnit]) -> Vec<WrittenUnit> {
    let mut units = shape.to_vec();
    let mut index = 0;
    while index < units.len() {
        if units[index] == Zwnj && is_redundant(&units, index) {
            units.remove(index);
        } else {
            index += 1;
        }
    }
    units
}

/// The only letters whose positions can change are the immediate neighbors of this ZWNJ.
/// A structural neighbor stays in place, so any letter beyond it retains its joining context.
fn is_redundant(units: &[WrittenUnit], index: usize) -> bool {
    if units.len() == 1 {
        return false;
    }
    let left = index.checked_sub(1).map(|i| units[i]);
    let right = units.get(index + 1).copied();
    if left == Some(Mvs) || right == Some(Mvs) {
        return true;
    }
    if let Some(unit) = left.filter(|u| !u.is_structural()) {
        let outer = index.checked_sub(2).map(|i| units[i]);
        if !equivalent(unit, position(outer, Some(Zwnj)), position(outer, right)) {
            return false;
        }
    }
    if let Some(unit) = right.filter(|u| !u.is_structural()) {
        let outer = units.get(index + 2).copied();
        if !equivalent(unit, position(Some(Zwnj), outer), position(left, outer)) {
            return false;
        }
    }
    true
}

/// Use the encoder's position convention. Only immediate neighbors supply connections;
/// neither an MVS nor another ZWNJ is skipped to find a letter or a joiner farther away.
fn position(left: Option<WrittenUnit>, right: Option<WrittenUnit>) -> Position {
    let joins = |unit: Option<WrittenUnit>| {
        usize::from(unit.is_some_and(|u| !u.is_structural() || is_joiner(u)))
    };
    let left = joins(left);
    slot_position(left, 1, left + 1 + joins(right))
}

const fn mask(position: Position) -> u8 {
    match position {
        Position::Isol => 1,
        Position::Init => 2,
        Position::Medi => 4,
        Position::Fina => 8,
    }
}

/// Reviewed per-unit positional equivalences for removing a ZWNJ break.
/// Paths may compose only for the same unit.
fn equivalent(unit: WrittenUnit, from: Position, to: Position) -> bool {
    use Position::{Fina, Init, Isol, Medi};
    let edges = [
        (Init, Medi, INIT_TO_MEDI),
        (Fina, Medi, FINA_TO_MEDI),
        (Isol, Fina, ISOL_TO_FINA),
        (Isol, Init, ISOL_TO_INIT),
    ];
    let mut reached = mask(from);
    loop {
        if reached & mask(to) != 0 {
            return true;
        }
        let before = reached;
        for (a, b, table) in edges {
            let pair = mask(a) | mask(b);
            if reached & pair != 0 && table.contains(&unit) {
                reached |= pair;
            }
        }
        if reached == before {
            return false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use WrittenUnit::{Aa, Nirugu, Zwj, A, B, I, J, N, O};

    #[test]
    fn reviewed_single_scan_examples() {
        let cases: &[(&[WrittenUnit], &[WrittenUnit])] = &[
            (&[Zwnj, B, Aa], &[B, Aa]),
            (&[B, Aa, Zwnj], &[B, Aa]),
            (&[A, O, Zwnj, O, A], &[A, O, O, A]),
            (&[A, N, Zwnj, A, A], &[A, N, Zwnj, A, A]),
            (&[B, Zwnj, I], &[B, I]),
            (&[B, Zwnj, A], &[B, Zwnj, A]),
            (&[A, Zwj, Zwnj, I], &[A, Zwj, I]),
            (&[A, Zwj, Zwnj, A], &[A, Zwj, Zwnj, A]),
            (&[B, Zwnj, Zwj, A], &[B, Zwj, A]),
            (&[A, Zwnj, Zwj, A], &[A, Zwnj, Zwj, A]),
            (&[N, Zwnj, Nirugu], &[N, Nirugu]),
            (&[A, Zwnj, Nirugu], &[A, Zwnj, Nirugu]),
            (&[Nirugu, Zwnj, I], &[Nirugu, I]),
            (&[Nirugu, Zwnj, B], &[Nirugu, Zwnj, B]),
            (&[A, J, Zwnj, Mvs, Aa], &[A, J, Mvs, Aa]),
            (&[A, J, Mvs, Zwnj, Aa], &[A, J, Mvs, Aa]),
            (
                &[A, J, Zwnj, Nirugu, Mvs, Aa],
                &[A, J, Zwnj, Nirugu, Mvs, Aa],
            ),
            (&[Zwnj], &[Zwnj]),
            (&[Mvs, Zwnj, Mvs], &[Mvs, Mvs]),
            (&[Nirugu, Zwnj, Nirugu], &[Nirugu, Nirugu]),
            (&[N, Zwnj, B, Zwnj, I], &[N, Zwnj, B, I]),
            (&[Nirugu, Zwnj, Zwj, A], &[Nirugu, Zwj, A]),
            (&[A, Zwj, Zwnj, Nirugu], &[A, Zwj, Nirugu]),
        ];
        for &(input, expected) in cases {
            let actual = drop_redundant_zwnj(input);
            assert_eq!(actual, expected, "{input:?}");
        }
    }

    #[test]
    fn equivalent_paths_are_per_letter_and_do_not_guess_missing_edges() {
        use Position::{Fina, Init, Isol, Medi};
        assert!(equivalent(B, Isol, Medi)); // isol -> init -> medi
        assert!(!equivalent(N, Isol, Medi)); // N lacks init -> medi
        assert!(!equivalent(A, Isol, Init));
        assert!(equivalent(A, Isol, Isol));
        assert!(!equivalent(WrittenUnit::Dd, Init, Medi));
        assert!(!equivalent(WrittenUnit::Dd, Isol, Fina));
    }
}
