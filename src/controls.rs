//! Canonical control policy: merge identical runs, apply the existing complete ZWJ filter,
//! then scan ZWNJ once. Repeat complete rounds until unchanged, before duplicate unification.
//! This is a pure written-unit transform, shared by both shaping backends and all encoders.

use crate::generated::enums::WrittenUnit;
use crate::normalize::{is_joiner, slot_position};
use crate::redundant_zwj::{
    drop_redundant_zwj, FINAL_AFTER_LETTER, FINAL_ALONE, INITIAL_ALONE, INITIAL_BEFORE_LETTER,
};
use crate::tables::Position;
use WrittenUnit::{Mvs, Zwj, Zwnj};

pub(crate) fn canonical_controls(input: &[WrittenUnit]) -> Vec<WrittenUnit> {
    let mut units = input.to_vec();
    loop {
        let before = units.len();
        units.dedup_by(|a, b| a == b && matches!(a, Zwj | Zwnj | Mvs));
        units = drop_redundant_zwj(&units);
        drop_zwnj_once(&mut units);
        // Every operation only removes units. Equal lengths therefore mean equal sequences;
        // each changing round strictly decreases the finite number of control units.
        if units.len() == before {
            return units;
        }
    }
}

/// The only letters whose positions can change are the immediate neighbors of this ZWNJ.
/// A structural neighbor stays in place, so any letter beyond it retains its joining context.
fn redundant_zwnj(units: &[WrittenUnit], index: usize) -> bool {
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

fn drop_zwnj_once(units: &mut Vec<WrittenUnit>) {
    let mut index = 0;
    while index < units.len() {
        if units[index] == Zwnj && redundant_zwnj(units, index) {
            units.remove(index);
        } else {
            index += 1;
        }
    }
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

/// The same four reviewed per-unit equivalences as ZWJ, used in either direction.
/// Paths may compose only for the same unit. The reviewed comparison snapshot contains no
/// additional approved edges; missing encodings do not revoke an existing ZWJ decision.
fn equivalent(unit: WrittenUnit, from: Position, to: Position) -> bool {
    use Position::{Fina, Init, Isol, Medi};
    let edges = [
        (Init, Medi, INITIAL_BEFORE_LETTER),
        (Fina, Medi, FINAL_AFTER_LETTER),
        (Isol, Fina, INITIAL_ALONE),
        (Isol, Init, FINAL_ALONE),
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
    use WrittenUnit::{Aa, Nirugu, A, B, I, J, N, O};

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
            let mut actual = input.to_vec();
            drop_zwnj_once(&mut actual);
            assert_eq!(actual, expected, "{input:?}");
        }
    }

    #[test]
    fn complete_rounds_revisit_kept_breaks_and_new_runs() {
        assert_eq!(canonical_controls(&[N, Zwnj, B, Zwnj, I]), [N, B, I]);
        assert_eq!(canonical_controls(&[Mvs, Zwnj, Mvs]), [Mvs]);
        assert_eq!(canonical_controls(&[Zwj, Zwnj, Zwj]), [Zwnj]);
        assert_eq!(
            canonical_controls(&[Nirugu, Zwnj, Nirugu]),
            [Nirugu, Nirugu]
        );
        assert_eq!(canonical_controls(&[]), []);
    }

    #[test]
    fn equivalent_paths_are_per_letter_and_do_not_guess_missing_edges() {
        use Position::{Init, Isol, Medi};
        assert!(equivalent(B, Isol, Medi)); // isol -> init -> medi
        assert!(!equivalent(N, Isol, Medi)); // N lacks init -> medi
        assert!(!equivalent(A, Isol, Init));
        assert!(equivalent(A, Isol, Isol));
    }

    #[test]
    fn complete_canonicalization_is_idempotent_around_duplicate_units() {
        use crate::shaper::canonical_units;
        use WrittenUnit::{Cr, Dd, Hx, H};
        let alphabet = [Zwj, Zwnj, Mvs, Nirugu, Dd, H, Hx, Cr, I, Aa, B, N, A, O];
        // Enumerate short unit inputs independently of whether a Unicode encoding
        // exists, including duplicates whose expansion can affect adjacent controls.
        for length in 0..=4 {
            for mut index in 0..alphabet.len().pow(length) {
                let input: Vec<_> = (0..length)
                    .map(|_| {
                        let unit = alphabet[index % alphabet.len()];
                        index /= alphabet.len();
                        unit
                    })
                    .collect();
                let canonical = canonical_units(&input);
                assert_eq!(canonical_units(&canonical), canonical, "{input:?}");
            }
        }
    }
}
