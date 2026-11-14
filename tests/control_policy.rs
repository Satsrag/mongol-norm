//! Integration contract for complete control-processing rounds, before duplicate unification.

mod common;

use common::mgl;
use mongol_norm::{
    Error, Locale, Position, PositionedWrittenUnit, Shaper, UnitPosition, WrittenUnit,
};

fn assert_units(shaper: &Shaper, input: &str, expected: &str) -> String {
    let input = shaper.parse_written_units(input).unwrap();
    let expected = shaper.parse_written_units(expected).unwrap();
    let normalized = shaper.normalize_written_units(&input).unwrap();
    assert_eq!(shaper.shape(&normalized).unwrap(), expected, "{input:?}");
    assert_eq!(shaper.normalize(&normalized).unwrap(), normalized);
    assert_eq!(
        shaper.normalize_written_units(&expected).unwrap(),
        normalized
    );
    assert_eq!(shaper.normalize_text(&normalized).unwrap(), normalized);
    normalized
}

#[test]
fn all_ordered_control_pairs_alone_and_between_letters() {
    let shaper = Shaper::new(Locale::Mng);
    // B:isol ↔ init and I:isol ↔ fina are both reviewed equivalences. They let the
    // same pair exercise letter-position checks as well as the control-only rules.
    for (pair, alone, between) in [
        ("Zwj+Zwj", "Zwj", "B+I"),
        ("Zwj+Zwnj", "Zwnj", "B+I"),
        ("Zwj+Mvs", "Mvs", "B+Mvs+I"),
        ("Zwj+Nirugu", "Nirugu", "B+Nirugu+I"),
        ("Zwnj+Zwj", "Zwnj", "B+I"),
        ("Zwnj+Zwnj", "Zwnj", "B+I"),
        ("Zwnj+Mvs", "Mvs", "B+Mvs+I"),
        ("Zwnj+Nirugu", "Nirugu", "B+Nirugu+I"),
        ("Mvs+Zwj", "Mvs", "B+Mvs+I"),
        ("Mvs+Zwnj", "Mvs", "B+Mvs+I"),
        ("Mvs+Mvs", "Mvs", "B+Mvs+I"),
        ("Mvs+Nirugu", "Mvs+Nirugu", "B+Mvs+Nirugu+I"),
        ("Nirugu+Zwj", "Nirugu", "B+Nirugu+I"),
        ("Nirugu+Zwnj", "Nirugu", "B+Nirugu+I"),
        ("Nirugu+Mvs", "Nirugu+Mvs", "B+Nirugu+Mvs+I"),
        ("Nirugu+Nirugu", "Nirugu+Nirugu", "B+Nirugu+Nirugu+I"),
    ] {
        assert_units(&shaper, pair, alone);
        assert_units(&shaper, &format!("B+{pair}+I"), between);
    }
}

#[test]
fn later_round_revisits_earlier_breaks_and_new_control_runs() {
    let shaper = Shaper::new(Locale::Mng);
    for (input, expected) in [
        ("N+Zwnj+B+Zwnj+I", "N+B+I"),
        ("Mvs+Zwnj+Mvs", "Mvs"),
        ("Mvs+Zwj+Zwnj+Mvs+Mvs", "Mvs"),
        ("Zwj+Zwnj+Zwj", "Zwnj"),
        ("Nirugu+Zwnj+Nirugu", "Nirugu+Nirugu"),
        ("A+O+Zwnj+O+A", "A+O+O+A"),
        ("A+N+Zwnj+A+A", "A+N+Zwnj+A+A"),
        ("N+Zwnj+Nirugu", "N+Nirugu"),
        ("A+Zwnj+Nirugu", "A+Zwnj+Nirugu"),
    ] {
        assert_units(&shaper, input, expected);
    }
}

#[test]
fn mvs_merging_preserves_jw_chachlag_targets_and_required_breaks() {
    let shaper = Shaper::new(Locale::Mng);
    for unit in ["J", "W"] {
        for input in [
            format!("A+{unit}+Mvs+Mvs+Aa"),
            format!("A+{unit}+Mvs+Mvs+Zwnj+Aa"),
            format!("A+{unit}+Zwnj+Mvs+Aa"),
        ] {
            let normalized = assert_units(&shaper, &input, &format!("A+{unit}+Mvs+Aa"));
            assert_eq!(normalized.matches('\u{180E}').count(), 1);
            assert!(normalized.ends_with("\u{180E}\u{1820}\u{180C}"));
        }
        let units = format!("A+{unit}+Zwnj+Nirugu+Mvs+Aa");
        let normalized = assert_units(&shaper, &units, &units);
        assert_eq!(normalized.matches('\u{200C}').count(), 1);
        assert_eq!(normalized.matches('\u{180A}').count(), 1);
    }
    assert_units(&shaper, "A+N+Mvs+Mvs+Aa", "A+N+Mvs+Aa");
    assert_eq!(assert_units(&shaper, "Mvs+Mvs+Aa", "Mvs+Aa"), mgl("mvs a"));
}

#[test]
fn raw_units_and_positions_precede_control_canonicalization() {
    use WrittenUnit::{Aa, Mvs, Zwnj, B};
    let shaper = Shaper::new(Locale::Mng);
    let text = mgl("zwnj b zwnj zwnj mvs mvs a");
    assert_eq!(
        shaper.shape_raw(&text).unwrap(),
        [Zwnj, B, Zwnj, Zwnj, Mvs, Mvs, Aa]
    );
    assert_eq!(shaper.shape(&text).unwrap(), [B, Mvs, Aa]);
    assert_eq!(shaper.trace(&text).unwrap().shape, [B, Mvs, Aa]);
    let details = shaper.shape_detailed(&text).unwrap();
    assert_eq!(details.len(), 7);
    assert_eq!(details[1].position, Position::Isol);
    assert_eq!(
        details.iter().filter(|unit| unit.cp == '\u{200C}').count(),
        3
    );
}

#[test]
fn positioned_records_are_validated_before_redundant_controls_disappear() {
    let shaper = Shaper::new(Locale::Mng);
    let rec = PositionedWrittenUnit::new;
    // Explicit zero-width controls are rejected up front; repeated MVS controls or an
    // implicit sub-chain boundary will be merged/dropped later, so per-record validation
    // must still report the original record index.
    assert_eq!(
        shaper.normalize_positioned_written_units(&[
            rec(WrittenUnit::Zwnj, UnitPosition::Control),
            rec(WrittenUnit::F, UnitPosition::Isol),
        ]),
        Err(Error::ExplicitZwnj)
    );
    assert_eq!(
        shaper.normalize_positioned_written_units(&[
            rec(WrittenUnit::Mvs, UnitPosition::Control),
            rec(WrittenUnit::Mvs, UnitPosition::Control),
            rec(WrittenUnit::F, UnitPosition::Isol),
        ]),
        Err(Error::UnsupportedPositionedUnit {
            index: 2,
            unit: WrittenUnit::F,
            position: UnitPosition::Isol,
        })
    );
    assert_eq!(
        shaper.normalize_positioned_written_units(&[
            rec(WrittenUnit::N, UnitPosition::Init),
            rec(WrittenUnit::F, UnitPosition::Isol),
        ]),
        Err(Error::UnsupportedPositionedUnit {
            index: 1,
            unit: WrittenUnit::F,
            position: UnitPosition::Isol,
        })
    );
    let records = [
        rec(WrittenUnit::Mvs, UnitPosition::Control),
        rec(WrittenUnit::Mvs, UnitPosition::Control),
        rec(WrittenUnit::N, UnitPosition::Init),
        rec(WrittenUnit::B, UnitPosition::Init),
        rec(WrittenUnit::I, UnitPosition::Fina),
    ];
    let normalized = shaper.normalize_positioned_written_units(&records).unwrap();
    assert_eq!(
        normalized,
        assert_units(&shaper, "Mvs+Mvs+N+Zwnj+B+I", "Mvs+N+B+I")
    );
}

#[test]
fn duplicate_units_next_to_controls_keep_public_encoding_idempotent() {
    let shaper = Shaper::new(Locale::Mng);
    let mut successes = 0;
    for unit in ["Dd", "H", "Hx", "Cr", "I", "Aa"] {
        for left in ["Zwj", "Zwnj", "Mvs", "Nirugu"] {
            for right in ["Zwj", "Zwnj", "Mvs", "Nirugu"] {
                for input in [
                    format!("{left}+{unit}+{right}"),
                    format!("B+{left}+{unit}+{right}+I"),
                ] {
                    let units = shaper.parse_written_units(&input).unwrap();
                    match shaper.normalize_written_units(&units) {
                        Ok(text) => {
                            successes += 1;
                            let canonical = shaper.shape(&text).unwrap();
                            assert_eq!(shaper.normalize(&text).unwrap(), text, "{input}");
                            assert_eq!(
                                shaper.normalize_written_units(&canonical).unwrap(),
                                text,
                                "{input}"
                            );
                        }
                        Err(Error::NoCanonicalEncoding) => {}
                        Err(error) => panic!("{input}: {error}"),
                    }
                }
            }
        }
    }
    assert!(successes > 150, "only {successes} encodable probes");
}

#[cfg(feature = "font")]
#[test]
fn both_shapers_share_control_processing() {
    let shaper = Shaper::new(Locale::Mng);
    let font = mongol_norm::FontShaper::new();
    for aliases in [
        "zwnj b zwnj zwnj mvs mvs a",
        "n zwnj b zwnj i fvs1",
        "a fvs1 j mvs mvs a fvs2",
        "a fvs1 w zwnj nirugu mvs a fvs2",
        "nirugu zwnj nirugu",
        "zwj zwnj zwj",
    ] {
        let text = mgl(aliases);
        assert_eq!(
            font.shape(&text).unwrap(),
            shaper.shape(&text).unwrap(),
            "{aliases}"
        );
    }
}
