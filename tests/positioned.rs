//! Positioned written-unit API — port of `python/tests/test_positioned_written_units_api.py`.

use mongol_norm::{Error, Locale, PositionedWrittenUnit, Shaper, UnitPosition, WrittenUnit};

const ZWJ: char = '\u{200D}';

fn shaper() -> Shaper {
    Shaper::new(Locale::Mng)
}

fn rec(unit: WrittenUnit, position: UnitPosition) -> PositionedWrittenUnit {
    PositionedWrittenUnit::new(unit, position)
}

fn positioned(shaper: &Shaper, records: &[PositionedWrittenUnit]) -> Result<String, Error> {
    shaper.normalize_positioned_written_units(records)
}

/// The same units through the plain written-unit API — the reference the positioned API must
/// reproduce once its implicit ZWJs are spelled out.
fn plain(shaper: &Shaper, units: &[WrittenUnit]) -> String {
    shaper.normalize_written_units(units).unwrap()
}

#[test]
fn test_rejects_explicit_zwj_input() {
    let shaper = shaper();
    let error = positioned(&shaper, &[rec(WrittenUnit::Zwj, UnitPosition::Control)]).unwrap_err();
    assert_eq!(error, Error::ExplicitZwj);
    assert_eq!(error.to_string(), "unsupported positioned control 'Zwj'");
}

#[test]
fn test_rejects_unsupported_f_isol_pair() {
    let shaper = shaper();
    let error = positioned(&shaper, &[rec(WrittenUnit::F, UnitPosition::Isol)]).unwrap_err();
    assert_eq!(
        error,
        Error::UnsupportedPositionedUnit {
            index: 0,
            unit: WrittenUnit::F,
            position: UnitPosition::Isol
        }
    );
    assert_eq!(
        error.to_string(),
        "unsupported positioned written unit 'F:isol'"
    );
}

#[test]
fn test_i_isol_and_init_use_the_plain_i_canonical() {
    let shaper = shaper();
    let expected = plain(&shaper, &[WrittenUnit::I]);
    for position in [UnitPosition::Isol, UnitPosition::Init] {
        assert_eq!(
            positioned(&shaper, &[rec(WrittenUnit::I, position)]).unwrap(),
            expected,
            "{position}"
        );
    }
}

#[test]
fn test_isolated_consonant_borrows_its_initial_written_unit() {
    let shaper = shaper();
    let result = positioned(&shaper, &[rec(WrittenUnit::B, UnitPosition::Init)]).unwrap();
    assert_eq!(result, "\u{182A}");
    assert!(!result.contains(ZWJ));
}

#[test]
fn test_isolated_fa_borrows_the_initial_f_written_unit() {
    let shaper = shaper();
    assert_eq!(shaper.shape("\u{1839}").unwrap(), [WrittenUnit::F]);
    let result = positioned(&shaper, &[rec(WrittenUnit::F, UnitPosition::Init)]).unwrap();
    assert_eq!(result, "\u{1839}");
    assert!(!result.contains(ZWJ));
}

#[test]
fn test_o_init_gets_its_required_trailing_zwj() {
    let shaper = shaper();
    let result = positioned(&shaper, &[rec(WrittenUnit::O, UnitPosition::Init)]).unwrap();
    let oa = positioned(
        &shaper,
        &[
            rec(WrittenUnit::O, UnitPosition::Init),
            rec(WrittenUnit::A, UnitPosition::Fina),
        ],
    )
    .unwrap();
    let result_cps: Vec<u32> = result.chars().map(|c| c as u32).collect();
    let oa_cps: Vec<u32> = oa.chars().map(|c| c as u32).collect();
    assert_eq!(result_cps[..result_cps.len() - 1], oa_cps[..2]);
    assert_eq!(result_cps, [0x1824, 0x180B, 0x200D]);
}

/// The `O:init` trailing ZWJ is for a request that is nothing but `O:init`. With a nirugu in
/// front the joining context already exists, so no ZWJ is invented. Confirmed against Python:
/// `normalize_positioned_written_units([Nirugu:control, O:init])` → U+180A U+1823 U+180B.
#[test]
fn test_nirugu_then_o_init_gets_no_trailing_zwj() {
    let shaper = shaper();
    let records = [
        rec(WrittenUnit::Nirugu, UnitPosition::Control),
        rec(WrittenUnit::O, UnitPosition::Init),
    ];
    let result = positioned(&shaper, &records).unwrap();
    assert_eq!(
        result,
        plain(&shaper, &[WrittenUnit::Nirugu, WrittenUnit::O])
    );
    assert!(!result.contains(ZWJ), "{result:?} must not contain U+200D");
    assert_eq!(
        result.chars().map(|c| c as u32).collect::<Vec<_>>(),
        [0x180A, 0x1823, 0x180B]
    );
}

#[test]
fn test_o_is_the_only_singleton_init_that_adds_zwj() {
    let shaper = shaper();
    let mut accepted = 0;
    for unit in WrittenUnit::ALL {
        match positioned(&shaper, &[rec(unit, UnitPosition::Init)]) {
            Ok(result) => {
                accepted += 1;
                assert_eq!(
                    result.matches(ZWJ).count(),
                    usize::from(unit == WrittenUnit::O),
                    "{unit}"
                );
            }
            Err(Error::UnsupportedPositionedUnit { .. })
            | Err(Error::ControlRequiresControlPosition { .. })
            | Err(Error::ExplicitZwj) => {}
            Err(other) => panic!("{unit}: {other}"),
        }
    }
    assert_eq!(accepted, 28, "the HUD inventory has 28 init units");
}

#[test]
fn test_empty_sequence_returns_empty_string() {
    let shaper = shaper();
    assert_eq!(positioned(&shaper, &[]).unwrap(), "");
}

#[test]
fn test_complete_compound_needs_no_implicit_zwj() {
    let shaper = shaper();
    let records = [
        rec(WrittenUnit::B, UnitPosition::Init),
        rec(WrittenUnit::O, UnitPosition::Medi),
        rec(WrittenUnit::G, UnitPosition::Fina),
    ];
    assert_eq!(
        positioned(&shaper, &records).unwrap(),
        plain(&shaper, &[WrittenUnit::B, WrittenUnit::O, WrittenUnit::G])
    );
}

#[test]
fn test_medi_started_compound_gets_a_leading_zwj() {
    let shaper = shaper();
    let records = [
        rec(WrittenUnit::B, UnitPosition::Medi),
        rec(WrittenUnit::O, UnitPosition::Medi),
        rec(WrittenUnit::G, UnitPosition::Fina),
    ];
    assert_eq!(
        positioned(&shaper, &records).unwrap(),
        plain(
            &shaper,
            &[
                WrittenUnit::Zwj,
                WrittenUnit::B,
                WrittenUnit::O,
                WrittenUnit::G
            ]
        )
    );
}

#[test]
fn test_mvs_splits_letter_position_chains_without_joining() {
    let shaper = shaper();
    let records = [
        rec(WrittenUnit::T, UnitPosition::Init),
        rec(WrittenUnit::A, UnitPosition::Medi),
        rec(WrittenUnit::L, UnitPosition::Fina),
        rec(WrittenUnit::Mvs, UnitPosition::Control),
        rec(WrittenUnit::Aa, UnitPosition::Isol),
    ];
    let units: Vec<WrittenUnit> = records.iter().map(|r| r.unit).collect();
    assert_eq!(
        positioned(&shaper, &records).unwrap(),
        plain(&shaper, &units)
    );
    assert_eq!(
        positioned(
            &shaper,
            &[
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::Aa, UnitPosition::Fina)
            ]
        )
        .unwrap(),
        plain(
            &shaper,
            &[WrittenUnit::Mvs, WrittenUnit::Zwj, WrittenUnit::Aa]
        )
    );
}

#[test]
fn test_controls_require_control_position_and_letters_reject_it() {
    let shaper = shaper();
    let error = positioned(&shaper, &[rec(WrittenUnit::Mvs, UnitPosition::Isol)]).unwrap_err();
    assert_eq!(
        error,
        Error::ControlRequiresControlPosition {
            index: 0,
            unit: WrittenUnit::Mvs
        }
    );
    assert!(error.to_string().contains("requires position 'control'"));
    let error = positioned(&shaper, &[rec(WrittenUnit::B, UnitPosition::Control)]).unwrap_err();
    assert_eq!(
        error,
        Error::UnsupportedPositionedUnit {
            index: 0,
            unit: WrittenUnit::B,
            position: UnitPosition::Control
        }
    );
    assert!(error.to_string().contains("unsupported positioned"));
}

#[test]
fn test_rejects_unknown_unit_position_pair() {
    let shaper = shaper();
    // `E` is a Todo unit, never in the HUD inventory.
    let error = positioned(&shaper, &[rec(WrittenUnit::E, UnitPosition::Isol)]).unwrap_err();
    assert_eq!(
        error.to_string(),
        "unsupported positioned written unit 'E:isol'"
    );
}

#[test]
fn test_explicit_nirugu_controls_make_a_medi_position_valid() {
    let shaper = shaper();
    let records = [
        rec(WrittenUnit::Nirugu, UnitPosition::Control),
        rec(WrittenUnit::O, UnitPosition::Medi),
        rec(WrittenUnit::Nirugu, UnitPosition::Control),
    ];
    assert_eq!(
        positioned(&shaper, &records).unwrap(),
        plain(
            &shaper,
            &[WrittenUnit::Nirugu, WrittenUnit::O, WrittenUnit::Nirugu]
        )
    );
}

#[test]
fn test_one_sided_joiners_and_repeated_controls() {
    let shaper = shaper();
    let cases: Vec<(Vec<PositionedWrittenUnit>, Vec<WrittenUnit>)> = vec![
        (
            vec![
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::U, UnitPosition::Fina),
            ],
            vec![WrittenUnit::Nirugu, WrittenUnit::U],
        ),
        (
            vec![
                rec(WrittenUnit::A, UnitPosition::Init),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            vec![WrittenUnit::A, WrittenUnit::Nirugu],
        ),
        (
            vec![
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
            ],
            vec![WrittenUnit::Mvs, WrittenUnit::Mvs, WrittenUnit::Aa],
        ),
    ];
    for (records, units) in &cases {
        assert_eq!(
            positioned(&shaper, records).unwrap(),
            plain(&shaper, units),
            "{units:?}"
        );
    }
}

#[test]
fn test_long_invalid_chain_fails_closed_without_recursion_error() {
    let shaper = shaper();
    let records = vec![rec(WrittenUnit::A, UnitPosition::Isol); 1000];
    let error = positioned(&shaper, &records).unwrap_err();
    assert_eq!(error, Error::ChainPositionMismatch);
    assert!(error
        .to_string()
        .contains("no canonical MNG encoding in the supplied context"));
}

#[test]
fn test_long_control_sequence_stays_iterative() {
    let shaper = shaper();
    let mut records = vec![rec(WrittenUnit::Mvs, UnitPosition::Control); 1000];
    records.push(rec(WrittenUnit::F, UnitPosition::Init));
    assert_eq!(
        positioned(&shaper, &records).unwrap(),
        format!("{}\u{1839}", "\u{180E}".repeat(1000))
    );
}

#[test]
fn test_record_limit_fails_closed() {
    let shaper = shaper();
    let records = vec![rec(WrittenUnit::Mvs, UnitPosition::Control); 1025];
    let error = positioned(&shaper, &records).unwrap_err();
    assert_eq!(error, Error::TooManyRecords { max: 1024 });
    assert!(error.to_string().contains("at most 1024 records"));
}

#[test]
fn test_singleton_medi_and_fina_insert_zwj_by_position() {
    let shaper = shaper();
    let cases: [(WrittenUnit, UnitPosition, &[WrittenUnit], usize); 2] = [
        (
            WrittenUnit::O,
            UnitPosition::Medi,
            &[WrittenUnit::Zwj, WrittenUnit::O, WrittenUnit::Zwj],
            2,
        ),
        (
            WrittenUnit::U,
            UnitPosition::Fina,
            &[WrittenUnit::Zwj, WrittenUnit::U],
            1,
        ),
    ];
    for (unit, position, units, zwj_count) in cases {
        let result = positioned(&shaper, &[rec(unit, position)]).unwrap();
        assert_eq!(result, plain(&shaper, units), "{unit}:{position}");
        assert_eq!(result.matches(ZWJ).count(), zwj_count);
    }
}

#[test]
fn test_encodes_a_valid_positioned_sequence() {
    let shaper = shaper();
    let records = [
        rec(WrittenUnit::B, UnitPosition::Init),
        rec(WrittenUnit::Aa, UnitPosition::Fina),
    ];
    assert_eq!(
        positioned(&shaper, &records).unwrap(),
        plain(&shaper, &[WrittenUnit::B, WrittenUnit::Aa])
    );
}
