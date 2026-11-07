//! Canonical control policy: after expanding single-unit duplicates, merge identical runs, apply
//! the complete ZWJ filter, then scan ZWNJ once. Repeat complete rounds until unchanged, before
//! chain-tail duplicate contraction. This is a pure written-unit transform, shared by both
//! shaping backends and all encoders.

use crate::generated::enums::WrittenUnit;
use crate::redundant_zwj::drop_redundant_zwj;
use crate::redundant_zwnj::drop_redundant_zwnj;
use WrittenUnit::{Mvs, Zwj, Zwnj};

pub(crate) fn canonical_controls(input: &[WrittenUnit]) -> Vec<WrittenUnit> {
    let mut units = input.to_vec();
    loop {
        let before = units.len();
        units.dedup_by(|a, b| a == b && matches!(a, Zwj | Zwnj | Mvs));
        units = drop_redundant_zwj(&units);
        units = drop_redundant_zwnj(&units);
        // Every operation only removes units. Equal lengths therefore mean equal sequences;
        // each changing round strictly decreases the finite number of control units.
        if units.len() == before {
            return units;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use WrittenUnit::{Aa, Nirugu, A, B, I, N, O};

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
