//! Positioned written-unit API — port of `python/tests/test_positioned_written_units_api.py`.

use mongol_norm::{
    Error, Locale, Position, PositionedWrittenUnit, Shaper, UnitPosition, WrittenUnit,
};

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
    for position in UnitPosition::ALL {
        let error = positioned(&shaper, &[rec(WrittenUnit::Zwj, position)]).unwrap_err();
        assert_eq!(error, Error::ExplicitZwj);
        assert_eq!(error.to_string(), "unsupported positioned control 'Zwj'");
    }
}

#[test]
fn test_rejects_explicit_zwnj_input() {
    let shaper = shaper();
    for position in UnitPosition::ALL {
        let error = positioned(&shaper, &[rec(WrittenUnit::Zwnj, position)]).unwrap_err();
        assert_eq!(error, Error::ExplicitZwnj);
        assert_eq!(error.to_string(), "unsupported positioned control 'Zwnj'");
    }
}

#[test]
fn test_validation_order_precedence() {
    let shaper = shaper();
    // 1. TooManyRecords beats ExplicitZwj / ExplicitZwnj and per-record errors.
    let mut too_many = vec![rec(WrittenUnit::Mvs, UnitPosition::Isol); 1023];
    too_many.push(rec(WrittenUnit::Zwnj, UnitPosition::Isol));
    too_many.push(rec(WrittenUnit::Zwj, UnitPosition::Control));
    assert_eq!(
        positioned(&shaper, &too_many).unwrap_err(),
        Error::TooManyRecords { max: 1024 }
    );

    // 2. First explicit zero-width control wins (regardless of its position), ahead of
    //    earlier per-record control/inventory errors.
    assert_eq!(
        positioned(
            &shaper,
            &[
                rec(WrittenUnit::Mvs, UnitPosition::Isol),
                rec(WrittenUnit::Zwj, UnitPosition::Isol),
                rec(WrittenUnit::Zwnj, UnitPosition::Control),
            ]
        )
        .unwrap_err(),
        Error::ExplicitZwj
    );
    assert_eq!(
        positioned(
            &shaper,
            &[
                rec(WrittenUnit::Mvs, UnitPosition::Isol),
                rec(WrittenUnit::Zwnj, UnitPosition::Isol),
                rec(WrittenUnit::Zwj, UnitPosition::Control),
            ]
        )
        .unwrap_err(),
        Error::ExplicitZwnj
    );
    assert_eq!(
        positioned(
            &shaper,
            &[
                rec(WrittenUnit::F, UnitPosition::Isol),
                rec(WrittenUnit::Zwnj, UnitPosition::Control),
            ]
        )
        .unwrap_err(),
        Error::ExplicitZwnj
    );

    // 4. Per-record check reports the first offender once no zero-width control is present,
    //    ahead of explicit-MVS right-neighbor checks.
    assert_eq!(
        positioned(
            &shaper,
            &[
                rec(WrittenUnit::Mvs, UnitPosition::Isol),
                rec(WrittenUnit::F, UnitPosition::Isol),
            ]
        )
        .unwrap_err(),
        Error::ControlRequiresControlPosition {
            index: 0,
            unit: WrittenUnit::Mvs,
        }
    );
    assert_eq!(
        positioned(
            &shaper,
            &[
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
                rec(WrittenUnit::F, UnitPosition::Isol),
            ]
        )
        .unwrap_err(),
        Error::UnsupportedPositionedUnit {
            index: 2,
            unit: WrittenUnit::F,
            position: UnitPosition::Isol,
        }
    );

    // 5. Explicit Mvs:control right-neighbor check runs left-to-right, reporting whichever
    //    Mvs:control violation comes first.
    assert_eq!(
        positioned(
            &shaper,
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::A, UnitPosition::Init),
                rec(WrittenUnit::A, UnitPosition::Fina),
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
            ]
        )
        .unwrap_err(),
        Error::RedundantChachlagMvs { index: 3 }
    );
    assert_eq!(
        positioned(
            &shaper,
            &[
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
                rec(WrittenUnit::Mvs, UnitPosition::Control),
            ]
        )
        .unwrap_err(),
        Error::RedundantChachlagMvs { index: 0 }
    );
    assert_eq!(
        positioned(
            &shaper,
            &[
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
            ]
        )
        .unwrap_err(),
        Error::ChainPositionMismatch
    );
    assert_eq!(
        positioned(
            &shaper,
            &[
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::I, UnitPosition::Isol),
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
            ]
        )
        .unwrap_err(),
        Error::RedundantChachlagMvs { index: 2 }
    );
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
fn test_i_isol_uses_the_plain_i_canonical() {
    let shaper = shaper();
    let result = positioned(&shaper, &[rec(WrittenUnit::I, UnitPosition::Isol)]).unwrap();
    assert_eq!(result, plain(&shaper, &[WrittenUnit::I]));
    assert!(!result.contains(ZWJ));
}

/// `A` and `I` are the letters the HUD inventory also carries at `isol`, under the same bare
/// spelling. Written bare, a lone `A:init` / `I:init` would read back as `A:isol` / `I:isol` — a
/// different record and a different glyph — so it takes the trailing ZWJ that keeps it initial,
/// exactly the spelling `normalize_written_units` gives `[unit, Zwj]` (Satsrag/meco-rust#45).
#[test]
fn test_a_and_i_init_take_a_trailing_zwj() {
    let shaper = shaper();
    for (unit, expected) in [
        (WrittenUnit::A, [0x1820, 0x180B, 0x200D]),
        (WrittenUnit::I, [0x1822, 0x180B, 0x200D]),
    ] {
        let result = positioned(&shaper, &[rec(unit, UnitPosition::Init)]).unwrap();
        assert_eq!(
            result.chars().map(|c| c as u32).collect::<Vec<_>>(),
            expected,
            "{unit}"
        );
        assert_eq!(result, plain(&shaper, &[unit, WrittenUnit::Zwj]), "{unit}");
        assert_eq!(
            shaper.shape(&result).unwrap(),
            [unit, WrittenUnit::Zwj],
            "{unit}"
        );
    }
}

/// The isolated form of a consonant is its initial written unit, so the bare letter already
/// reads back as `init` and must not take a ZWJ — `Ch`, and `D` too, although its isolated glyph
/// differs in ink from its joined initial one.
#[test]
fn test_isolated_consonants_stay_bare_at_init() {
    let shaper = shaper();
    for (unit, expected) in [(WrittenUnit::Ch, "\u{1834}"), (WrittenUnit::D, "\u{1833}")] {
        assert_eq!(
            positioned(&shaper, &[rec(unit, UnitPosition::Init)]).unwrap(),
            expected,
            "{unit}"
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

/// With a nirugu on the *right* (`[X:init, Nirugu:control]`), the nirugu supplies the
/// right-hand joining context, so `A:init` / `I:init` / `O:init` need no trailing ZWJ.
#[test]
fn test_init_then_nirugu_gets_no_trailing_zwj() {
    let shaper = shaper();
    for (unit, expected) in [
        (WrittenUnit::A, "\u{1820}\u{180B}\u{180A}"),
        (WrittenUnit::I, "\u{1822}\u{180B}\u{180A}"),
        (WrittenUnit::O, "\u{1824}\u{180B}\u{180A}"),
    ] {
        let records = [
            rec(unit, UnitPosition::Init),
            rec(WrittenUnit::Nirugu, UnitPosition::Control),
        ];
        let result = positioned(&shaper, &records).unwrap();
        assert_eq!(
            result,
            plain(&shaper, &[unit, WrittenUnit::Nirugu]),
            "{unit}"
        );
        assert!(
            !result.contains(ZWJ),
            "{unit}: {result:?} must not contain U+200D"
        );
        assert_eq!(result, expected, "{unit}");
    }
}

/// With a nirugu on the *left* (`[Nirugu:control, X:init]`), `init` cannot join leftward into
/// the nirugu, so an implicit `Zwnj` separates them and `A:init` / `I:init` / `O:init` still
/// receive their lone-`init` trailing `Zwj` before canonical control reduction:
/// - `A:init`: neither control is redundant (`\u{180A}\u{200C}\u{1820}\u{180B}\u{200D}`);
/// - `I:init`: `I:init ≡ I:medi` drops the `Zwnj`, keeping the `Zwj` (`\u{180A}\u{1822}\u{180D}\u{200D}`);
/// - `O:init`: `O:init ≡ O:medi ≡ O:fina` drops both controls (`\u{180A}\u{1823}\u{180B}`).
#[test]
fn test_nirugu_then_a_i_o_init_synthesizes_zwnj_and_lone_init_zwj() {
    let shaper = shaper();
    for (unit, expected) in [
        (WrittenUnit::A, "\u{180A}\u{200C}\u{1820}\u{180B}\u{200D}"),
        (WrittenUnit::I, "\u{180A}\u{1822}\u{180D}\u{200D}"),
        (WrittenUnit::O, "\u{180A}\u{1823}\u{180B}"),
    ] {
        let records = [
            rec(WrittenUnit::Nirugu, UnitPosition::Control),
            rec(unit, UnitPosition::Init),
        ];
        let result = positioned(&shaper, &records).unwrap();
        assert_eq!(
            result,
            plain(
                &shaper,
                &[
                    WrittenUnit::Nirugu,
                    WrittenUnit::Zwnj,
                    unit,
                    WrittenUnit::Zwj,
                ]
            ),
            "{unit}"
        );
        assert_eq!(result, expected, "{unit}");
    }
}

/// Every lone `init` record must read back as `init`. Joined on the right, a one-unit chain is
/// `init`. Bare, it is `isol` — which is still the `init` record wherever the inventory has no
/// separate `isol` for the letter and borrows the initial unit (every consonant). So the trailing
/// ZWJ belongs to exactly the letters whose bare spelling is another record: `A` and `I`, which
/// the inventory also carries at `isol`, and `O`, which has no bare spelling of its own.
#[test]
fn test_every_singleton_init_reads_back_as_init() {
    let shaper = shaper();
    let inventory = shaper.positioned_written_units().unwrap();
    let mut joined = Vec::new();
    let mut accepted = 0;
    for unit in WrittenUnit::ALL {
        let result = match positioned(&shaper, &[rec(unit, UnitPosition::Init)]) {
            Ok(result) => result,
            Err(
                Error::UnsupportedPositionedUnit { .. }
                | Error::ControlRequiresControlPosition { .. }
                | Error::ExplicitZwj
                | Error::ExplicitZwnj,
            ) => continue,
            Err(other) => panic!("{unit}: {other}"),
        };
        accepted += 1;
        let shape = shaper.shape(&result).unwrap();
        if shape == [unit, WrittenUnit::Zwj] {
            joined.push(unit);
        } else {
            let expected: &[WrittenUnit] = if unit == WrittenUnit::Cr {
                &[WrittenUnit::O, WrittenUnit::O]
            } else {
                std::slice::from_ref(&unit)
            };
            assert_eq!(shape, expected, "{unit}: {result:?}");
            assert!(
                !inventory.contains(&(unit, Position::Isol)),
                "{unit}: bare {result:?} reads back as {unit}:isol, not {unit}:init"
            );
        }
    }
    joined.sort_by_key(|unit| unit.as_str());
    assert_eq!(accepted, 28, "the HUD inventory has 28 init units");
    assert_eq!(joined, [WrittenUnit::A, WrittenUnit::I, WrittenUnit::O]);
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
    for (records, units) in [
        (
            vec![
                rec(WrittenUnit::T, UnitPosition::Init),
                rec(WrittenUnit::A, UnitPosition::Medi),
                rec(WrittenUnit::L, UnitPosition::Fina),
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::I, UnitPosition::Isol),
            ],
            vec![
                WrittenUnit::T,
                WrittenUnit::A,
                WrittenUnit::L,
                WrittenUnit::Mvs,
                WrittenUnit::I,
            ],
        ),
        (
            vec![
                rec(WrittenUnit::T, UnitPosition::Init),
                rec(WrittenUnit::A, UnitPosition::Medi),
                rec(WrittenUnit::L, UnitPosition::Fina),
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::D, UnitPosition::Init),
                rec(WrittenUnit::U, UnitPosition::Fina),
            ],
            vec![
                WrittenUnit::T,
                WrittenUnit::A,
                WrittenUnit::L,
                WrittenUnit::Mvs,
                WrittenUnit::D,
                WrittenUnit::U,
            ],
        ),
    ] {
        assert_eq!(
            positioned(&shaper, &records).unwrap(),
            plain(&shaper, &units)
        );
    }
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
    // Isolated FA borrows `F:init`: `F:isol` is not in the HUD inventory.
    let error = positioned(&shaper, &[rec(WrittenUnit::F, UnitPosition::Isol)]).unwrap_err();
    assert_eq!(
        error.to_string(),
        "unsupported positioned written unit 'F:isol'"
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
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::U, UnitPosition::Fina),
            ],
            vec![WrittenUnit::Nirugu, WrittenUnit::Nirugu, WrittenUnit::U],
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
    let mut invalid = vec![rec(WrittenUnit::Mvs, UnitPosition::Control)];
    invalid.extend(vec![rec(WrittenUnit::A, UnitPosition::Medi); 998]);
    invalid.push(rec(WrittenUnit::A, UnitPosition::Fina));
    let error = positioned(&shaper, &invalid).unwrap_err();
    assert_eq!(error, Error::ChainPositionMismatch);
    assert!(error
        .to_string()
        .contains("no canonical MNG encoding in the supplied context"));

    let mut nirugu_chain = vec![rec(WrittenUnit::Nirugu, UnitPosition::Control)];
    nirugu_chain.push(rec(WrittenUnit::A, UnitPosition::Init));
    nirugu_chain.extend(vec![rec(WrittenUnit::A, UnitPosition::Medi); 997]);
    nirugu_chain.push(rec(WrittenUnit::A, UnitPosition::Fina));
    let mut expected_units = vec![WrittenUnit::Nirugu, WrittenUnit::Zwnj];
    expected_units.extend(vec![WrittenUnit::A; 999]);
    assert_eq!(
        positioned(&shaper, &nirugu_chain).unwrap(),
        plain(&shaper, &expected_units)
    );
}

#[test]
fn test_long_control_sequence_stays_iterative() {
    let shaper = shaper();
    let mut nirugus = vec![rec(WrittenUnit::Nirugu, UnitPosition::Control); 1000];
    nirugus.push(rec(WrittenUnit::U, UnitPosition::Fina));
    let mut units = vec![WrittenUnit::Nirugu; 1000];
    units.push(WrittenUnit::U);
    assert_eq!(
        positioned(&shaper, &nirugus).unwrap(),
        plain(&shaper, &units)
    );

    let mut repeated_mvs = vec![rec(WrittenUnit::Mvs, UnitPosition::Control); 1000];
    repeated_mvs.push(rec(WrittenUnit::F, UnitPosition::Init));
    assert_eq!(
        positioned(&shaper, &repeated_mvs).unwrap_err(),
        Error::ChainPositionMismatch
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
    // The implicit joiners are inserted by position, then folded exactly as
    // `normalize_written_units` folds an explicit `Zwj`: `N:medi` keeps both (`N:init` and
    // `N:fina` are different shapes), `O:medi` keeps only the trailing one (`O:init` is the
    // medial form), and a lone `U:fina` is the isolated `U`.
    let shaper = shaper();
    let cases: [(WrittenUnit, UnitPosition, &[WrittenUnit], usize); 3] = [
        (
            WrittenUnit::N,
            UnitPosition::Medi,
            &[WrittenUnit::Zwj, WrittenUnit::N, WrittenUnit::Zwj],
            2,
        ),
        (
            WrittenUnit::O,
            UnitPosition::Medi,
            &[WrittenUnit::Zwj, WrittenUnit::O, WrittenUnit::Zwj],
            1,
        ),
        (
            WrittenUnit::U,
            UnitPosition::Fina,
            &[WrittenUnit::Zwj, WrittenUnit::U],
            0,
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

#[test]
fn test_synthesizes_zwnj_across_sub_chain_boundaries() {
    let shaper = shaper();
    // Issue #68 Example 1: [A:init, A:medi, L:fina] + [M:init, A:fina] -> ᠠ᠋ᠠᠯ‌ᠮᠠ᠌
    let records = [
        rec(WrittenUnit::A, UnitPosition::Init),
        rec(WrittenUnit::A, UnitPosition::Medi),
        rec(WrittenUnit::L, UnitPosition::Fina),
        rec(WrittenUnit::M, UnitPosition::Init),
        rec(WrittenUnit::A, UnitPosition::Fina),
    ];
    assert_eq!(
        positioned(&shaper, &records).unwrap(),
        "\u{1820}\u{180B}\u{1820}\u{182F}\u{200C}\u{182E}\u{1820}\u{180C}"
    );
    assert_eq!(
        positioned(&shaper, &records).unwrap(),
        plain(
            &shaper,
            &[
                WrittenUnit::A,
                WrittenUnit::A,
                WrittenUnit::L,
                WrittenUnit::Zwnj,
                WrittenUnit::M,
                WrittenUnit::A,
            ]
        )
    );

    // Issue #68 Example 2: lone consonant [B:init] + [A:init, A:fina] (from ᠪ‌ᠠ)
    let records = [
        rec(WrittenUnit::B, UnitPosition::Init),
        rec(WrittenUnit::A, UnitPosition::Init),
        rec(WrittenUnit::A, UnitPosition::Fina),
    ];
    assert_eq!(
        positioned(&shaper, &records).unwrap(),
        shaper.normalize("\u{182A}\u{200C}\u{1820}").unwrap()
    );
    assert_eq!(
        positioned(&shaper, &records).unwrap(),
        "\u{182A}\u{200C}\u{1820}\u{180B}\u{1820}\u{180C}"
    );
    assert_eq!(
        positioned(&shaper, &records).unwrap(),
        plain(
            &shaper,
            &[
                WrittenUnit::B,
                WrittenUnit::Zwnj,
                WrittenUnit::A,
                WrittenUnit::A,
            ]
        )
    );
}

#[test]
fn test_sub_chain_zwnj_drops_when_redundant_and_pads_open_edges() {
    let shaper = shaper();
    let cases: &[(&[PositionedWrittenUnit], &[WrittenUnit], &str)] = &[
        // Redundant synthesized ZWNJ is omitted by canonical control reduction.
        (
            &[
                rec(WrittenUnit::A, UnitPosition::Init),
                rec(WrittenUnit::O, UnitPosition::Fina),
                rec(WrittenUnit::O, UnitPosition::Init),
                rec(WrittenUnit::A, UnitPosition::Fina),
            ],
            &[
                WrittenUnit::A,
                WrittenUnit::O,
                WrittenUnit::O,
                WrittenUnit::A,
            ],
            "\u{1820}\u{180B}\u{1823}\u{1823}\u{1820}\u{180C}",
        ),
        (
            &[
                rec(WrittenUnit::N, UnitPosition::Init),
                rec(WrittenUnit::B, UnitPosition::Init),
                rec(WrittenUnit::Aa, UnitPosition::Fina),
            ],
            &[WrittenUnit::N, WrittenUnit::B, WrittenUnit::Aa],
            "\u{1828}\u{182A}\u{1820}\u{180B}",
        ),
        // Lone A:init / O:init sub-chains still get their trailing ZWJ before a sub-chain break.
        (
            &[
                rec(WrittenUnit::A, UnitPosition::Init),
                rec(WrittenUnit::M, UnitPosition::Init),
                rec(WrittenUnit::A, UnitPosition::Fina),
            ],
            &[
                WrittenUnit::A,
                WrittenUnit::Zwj,
                WrittenUnit::Zwnj,
                WrittenUnit::M,
                WrittenUnit::A,
            ],
            "\u{1820}\u{180B}\u{200D}\u{200C}\u{182E}\u{1820}\u{180C}",
        ),
        (
            &[
                rec(WrittenUnit::O, UnitPosition::Init),
                rec(WrittenUnit::M, UnitPosition::Init),
                rec(WrittenUnit::A, UnitPosition::Fina),
            ],
            &[
                WrittenUnit::O,
                WrittenUnit::Zwj,
                WrittenUnit::Zwnj,
                WrittenUnit::M,
                WrittenUnit::A,
            ],
            "\u{1824}\u{180B}\u{200D}\u{200C}\u{182E}\u{1820}\u{180C}",
        ),
        // Open medi edges on sub-chains get ZWJ alongside the synthesized ZWNJ.
        (
            &[
                rec(WrittenUnit::A, UnitPosition::Init),
                rec(WrittenUnit::L, UnitPosition::Medi),
                rec(WrittenUnit::M, UnitPosition::Init),
                rec(WrittenUnit::A, UnitPosition::Fina),
            ],
            &[
                WrittenUnit::A,
                WrittenUnit::L,
                WrittenUnit::Zwj,
                WrittenUnit::Zwnj,
                WrittenUnit::M,
                WrittenUnit::A,
            ],
            "\u{1820}\u{180B}\u{182F}\u{200D}\u{200C}\u{182E}\u{1820}\u{180C}",
        ),
        (
            &[
                rec(WrittenUnit::A, UnitPosition::Init),
                rec(WrittenUnit::L, UnitPosition::Fina),
                rec(WrittenUnit::N, UnitPosition::Medi),
                rec(WrittenUnit::A, UnitPosition::Fina),
            ],
            &[
                WrittenUnit::A,
                WrittenUnit::L,
                WrittenUnit::Zwnj,
                WrittenUnit::Zwj,
                WrittenUnit::N,
                WrittenUnit::A,
            ],
            "\u{1820}\u{180B}\u{182F}\u{200C}\u{200D}\u{1828}\u{180B}\u{1820}\u{180C}",
        ),
        // Adjacent isol sub-chains.
        (
            &[
                rec(WrittenUnit::A, UnitPosition::Isol),
                rec(WrittenUnit::A, UnitPosition::Isol),
            ],
            &[WrittenUnit::A, WrittenUnit::Zwnj, WrittenUnit::A],
            "\u{1820}\u{180B}\u{200C}\u{1820}\u{180B}",
        ),
    ];
    for &(records, units, expected) in cases {
        let actual = positioned(&shaper, records).unwrap();
        assert_eq!(actual, plain(&shaper, units), "{records:?}");
        assert_eq!(actual, expected, "{records:?}");
    }
}

#[test]
fn test_aa_isol_synthesizes_leading_mvs() {
    let shaper = shaper();
    let cases: &[(&[PositionedWrittenUnit], &[WrittenUnit], &str)] = &[
        (
            &[rec(WrittenUnit::Aa, UnitPosition::Isol)],
            &[WrittenUnit::Mvs, WrittenUnit::Aa],
            "\u{180E}\u{1820}",
        ),
        (
            &[
                rec(WrittenUnit::S, UnitPosition::Init),
                rec(WrittenUnit::I, UnitPosition::Medi),
                rec(WrittenUnit::N, UnitPosition::Fina),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
            ],
            &[
                WrittenUnit::S,
                WrittenUnit::I,
                WrittenUnit::N,
                WrittenUnit::Mvs,
                WrittenUnit::Aa,
            ],
            "\u{1830}\u{1822}\u{180D}\u{1828}\u{180B}\u{180E}\u{1820}",
        ),
        (
            &[
                rec(WrittenUnit::Ch, UnitPosition::Init),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
            ],
            &[WrittenUnit::Ch, WrittenUnit::Mvs, WrittenUnit::Aa],
            "\u{1834}\u{180E}\u{1820}",
        ),
        (
            &[
                rec(WrittenUnit::A, UnitPosition::Init),
                rec(WrittenUnit::A, UnitPosition::Medi),
                rec(WrittenUnit::J, UnitPosition::Fina),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
            ],
            &[
                WrittenUnit::A,
                WrittenUnit::A,
                WrittenUnit::J,
                WrittenUnit::Mvs,
                WrittenUnit::Aa,
            ],
            "\u{1820}\u{180B}\u{1820}\u{1835}\u{180E}\u{1820}\u{180C}",
        ),
        (
            &[
                rec(WrittenUnit::A, UnitPosition::Init),
                rec(WrittenUnit::A, UnitPosition::Medi),
                rec(WrittenUnit::W, UnitPosition::Fina),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
            ],
            &[
                WrittenUnit::A,
                WrittenUnit::A,
                WrittenUnit::W,
                WrittenUnit::Mvs,
                WrittenUnit::Aa,
            ],
            "\u{1820}\u{180B}\u{1820}\u{1838}\u{180E}\u{1820}\u{180C}",
        ),
        (
            &[
                rec(WrittenUnit::A, UnitPosition::Init),
                rec(WrittenUnit::L, UnitPosition::Medi),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
            ],
            &[
                WrittenUnit::A,
                WrittenUnit::L,
                WrittenUnit::Zwj,
                WrittenUnit::Mvs,
                WrittenUnit::Aa,
            ],
            "\u{1820}\u{180B}\u{182F}\u{200D}\u{180E}\u{1820}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
            ],
            &[WrittenUnit::Nirugu, WrittenUnit::Mvs, WrittenUnit::Aa],
            "\u{180A}\u{180E}\u{1820}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::Aa, UnitPosition::Fina),
            ],
            &[WrittenUnit::Nirugu, WrittenUnit::Aa],
            "\u{180A}\u{1820}\u{180B}",
        ),
        (
            &[
                rec(WrittenUnit::Aa, UnitPosition::Isol),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
            ],
            &[
                WrittenUnit::Mvs,
                WrittenUnit::Aa,
                WrittenUnit::Mvs,
                WrittenUnit::Aa,
            ],
            "\u{180E}\u{1820}\u{180E}\u{1820}",
        ),
        (
            &[
                rec(WrittenUnit::Aa, UnitPosition::Isol),
                rec(WrittenUnit::B, UnitPosition::Init),
                rec(WrittenUnit::Aa, UnitPosition::Fina),
            ],
            &[
                WrittenUnit::Mvs,
                WrittenUnit::Aa,
                WrittenUnit::Zwnj,
                WrittenUnit::B,
                WrittenUnit::Aa,
            ],
            "\u{180E}\u{1820}\u{200C}\u{182A}\u{1820}\u{180B}",
        ),
        (
            &[
                rec(WrittenUnit::S, UnitPosition::Init),
                rec(WrittenUnit::I, UnitPosition::Medi),
                rec(WrittenUnit::N, UnitPosition::Fina),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::I, UnitPosition::Init),
                rec(WrittenUnit::I, UnitPosition::Medi),
                rec(WrittenUnit::A, UnitPosition::Fina),
            ],
            &[
                WrittenUnit::S,
                WrittenUnit::I,
                WrittenUnit::N,
                WrittenUnit::Mvs,
                WrittenUnit::Aa,
                WrittenUnit::Mvs,
                WrittenUnit::I,
                WrittenUnit::I,
                WrittenUnit::A,
            ],
            "\u{1830}\u{1822}\u{180D}\u{1828}\u{180B}\u{180E}\u{1820}\u{180E}\u{1822}\u{180B}\u{1822}\u{180D}\u{1820}\u{180C}",
        ),
    ];
    for &(records, units, expected) in cases {
        let actual = positioned(&shaper, records).unwrap();
        assert_eq!(actual, plain(&shaper, units), "{records:?}");
        assert_eq!(actual, expected, "{records:?}");
    }

    for unit in [
        WrittenUnit::A,
        WrittenUnit::I,
        WrittenUnit::Ix,
        WrittenUnit::U,
        WrittenUnit::Ux,
    ] {
        let actual = positioned(&shaper, &[rec(unit, UnitPosition::Isol)]).unwrap();
        assert!(!actual.contains('\u{180E}'), "{unit}: {actual:?}");
    }
}

#[test]
fn test_rejects_invalid_and_redundant_explicit_mvs() {
    let shaper = shaper();
    for (records, index) in [
        (
            vec![
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
            ],
            0,
        ),
        (
            vec![
                rec(WrittenUnit::T, UnitPosition::Init),
                rec(WrittenUnit::A, UnitPosition::Medi),
                rec(WrittenUnit::L, UnitPosition::Fina),
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
            ],
            3,
        ),
        (
            vec![
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::Mvs, UnitPosition::Control),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
            ],
            1,
        ),
    ] {
        let error = positioned(&shaper, &records).unwrap_err();
        assert_eq!(error, Error::RedundantChachlagMvs { index }, "{records:?}");
        assert_eq!(
            error.to_string(),
            format!("positioned_units[{index}] has redundant 'Mvs' before 'Aa:isol'")
        );
    }

    for records in [
        vec![rec(WrittenUnit::Mvs, UnitPosition::Control)],
        vec![
            rec(WrittenUnit::B, UnitPosition::Init),
            rec(WrittenUnit::Aa, UnitPosition::Fina),
            rec(WrittenUnit::Mvs, UnitPosition::Control),
        ],
        vec![
            rec(WrittenUnit::Mvs, UnitPosition::Control),
            rec(WrittenUnit::Aa, UnitPosition::Fina),
        ],
        vec![
            rec(WrittenUnit::T, UnitPosition::Init),
            rec(WrittenUnit::A, UnitPosition::Medi),
            rec(WrittenUnit::L, UnitPosition::Fina),
            rec(WrittenUnit::Mvs, UnitPosition::Control),
            rec(WrittenUnit::Aa, UnitPosition::Fina),
        ],
        vec![
            rec(WrittenUnit::Mvs, UnitPosition::Control),
            rec(WrittenUnit::N, UnitPosition::Medi),
        ],
        vec![
            rec(WrittenUnit::Mvs, UnitPosition::Control),
            rec(WrittenUnit::B, UnitPosition::Medi),
            rec(WrittenUnit::O, UnitPosition::Medi),
            rec(WrittenUnit::G, UnitPosition::Fina),
        ],
        vec![
            rec(WrittenUnit::Mvs, UnitPosition::Control),
            rec(WrittenUnit::Nirugu, UnitPosition::Control),
            rec(WrittenUnit::U, UnitPosition::Fina),
        ],
        vec![
            rec(WrittenUnit::Mvs, UnitPosition::Control),
            rec(WrittenUnit::Mvs, UnitPosition::Control),
            rec(WrittenUnit::I, UnitPosition::Isol),
        ],
    ] {
        let error = positioned(&shaper, &records).unwrap_err();
        assert_eq!(error, Error::ChainPositionMismatch, "{records:?}");
    }
}

#[test]
fn test_synthesizes_zwnj_across_nirugu_boundaries() {
    let shaper = shaper();
    let cases: &[(&[PositionedWrittenUnit], &[WrittenUnit], &str)] = &[
        // Nirugu followed by `isol`: ZWNJ prevents silent mutation to `fina`.
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::A, UnitPosition::Isol),
            ],
            &[WrittenUnit::Nirugu, WrittenUnit::Zwnj, WrittenUnit::A],
            "\u{180A}\u{200C}\u{1820}\u{180B}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::Ix, UnitPosition::Isol),
            ],
            &[WrittenUnit::Nirugu, WrittenUnit::Zwnj, WrittenUnit::Ix],
            "\u{180A}\u{200C}\u{1822}\u{180C}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::Ux, UnitPosition::Isol),
            ],
            &[WrittenUnit::Nirugu, WrittenUnit::Zwnj, WrittenUnit::Ux],
            "\u{180A}\u{200C}\u{1824}\u{180C}",
        ),
        // `I:isol` and `U:isol` have `isol ≡ fina`, so canonical control reduction drops ZWNJ.
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::I, UnitPosition::Isol),
            ],
            &[WrittenUnit::Nirugu, WrittenUnit::Zwnj, WrittenUnit::I],
            "\u{180A}\u{1822}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::U, UnitPosition::Isol),
            ],
            &[WrittenUnit::Nirugu, WrittenUnit::Zwnj, WrittenUnit::U],
            "\u{180A}\u{1823}\u{180C}",
        ),
        // `isol` followed by Nirugu: ZWNJ prevents silent mutation to `init` (including chachlag `Aa:isol`).
        (
            &[
                rec(WrittenUnit::A, UnitPosition::Isol),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[WrittenUnit::A, WrittenUnit::Zwnj, WrittenUnit::Nirugu],
            "\u{1820}\u{180B}\u{200C}\u{180A}",
        ),
        (
            &[
                rec(WrittenUnit::I, UnitPosition::Isol),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[WrittenUnit::I, WrittenUnit::Zwnj, WrittenUnit::Nirugu],
            "\u{1822}\u{180B}\u{200C}\u{180A}",
        ),
        (
            &[
                rec(WrittenUnit::Ix, UnitPosition::Isol),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[WrittenUnit::Ix, WrittenUnit::Zwnj, WrittenUnit::Nirugu],
            "\u{1822}\u{180C}\u{200C}\u{180A}",
        ),
        (
            &[
                rec(WrittenUnit::U, UnitPosition::Isol),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[WrittenUnit::U, WrittenUnit::Zwnj, WrittenUnit::Nirugu],
            "\u{1824}\u{180B}\u{200C}\u{180A}",
        ),
        (
            &[
                rec(WrittenUnit::Ux, UnitPosition::Isol),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[WrittenUnit::Ux, WrittenUnit::Zwnj, WrittenUnit::Nirugu],
            "\u{1824}\u{180C}\u{200C}\u{180A}",
        ),
        (
            &[
                rec(WrittenUnit::Aa, UnitPosition::Isol),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[
                WrittenUnit::Mvs,
                WrittenUnit::Aa,
                WrittenUnit::Zwnj,
                WrittenUnit::Nirugu,
            ],
            "\u{180E}\u{1820}\u{200C}\u{180A}",
        ),
        // `fina` followed by Nirugu (single-record and multi-record): ZWNJ prevents mutation to `medi`.
        (
            &[
                rec(WrittenUnit::A, UnitPosition::Fina),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[
                WrittenUnit::Zwj,
                WrittenUnit::A,
                WrittenUnit::Zwnj,
                WrittenUnit::Nirugu,
            ],
            "\u{200D}\u{1820}\u{180C}\u{200C}\u{180A}",
        ),
        (
            &[
                rec(WrittenUnit::B, UnitPosition::Fina),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[
                WrittenUnit::Zwj,
                WrittenUnit::B,
                WrittenUnit::Zwnj,
                WrittenUnit::Nirugu,
            ],
            "\u{200D}\u{182A}\u{200C}\u{180A}",
        ),
        (
            &[
                rec(WrittenUnit::B2, UnitPosition::Fina),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[
                WrittenUnit::Zwj,
                WrittenUnit::B2,
                WrittenUnit::Zwnj,
                WrittenUnit::Nirugu,
            ],
            "\u{200D}\u{182A}\u{180B}\u{200C}\u{180A}",
        ),
        (
            &[
                rec(WrittenUnit::T, UnitPosition::Init),
                rec(WrittenUnit::G, UnitPosition::Fina),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::O, UnitPosition::Fina),
            ],
            &[
                WrittenUnit::T,
                WrittenUnit::G,
                WrittenUnit::Zwnj,
                WrittenUnit::Nirugu,
                WrittenUnit::O,
            ],
            "\u{1832}\u{182D}\u{180C}\u{200C}\u{180A}\u{1823}\u{180B}",
        ),
        // Nirugu followed by `init` (single-record, multi-record, and borrowed `Zr:init` / `Cr:init`).
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::B, UnitPosition::Init),
            ],
            &[WrittenUnit::Nirugu, WrittenUnit::Zwnj, WrittenUnit::B],
            "\u{180A}\u{200C}\u{182A}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::A, UnitPosition::Init),
                rec(WrittenUnit::A, UnitPosition::Fina),
            ],
            &[
                WrittenUnit::Nirugu,
                WrittenUnit::Zwnj,
                WrittenUnit::A,
                WrittenUnit::A,
            ],
            "\u{180A}\u{200C}\u{1820}\u{180B}\u{1820}\u{180C}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::Zr, UnitPosition::Init),
            ],
            &[WrittenUnit::Nirugu, WrittenUnit::Zwnj, WrittenUnit::Zr],
            "\u{180A}\u{1841}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::Zr, UnitPosition::Init),
                rec(WrittenUnit::I, UnitPosition::Fina),
            ],
            &[
                WrittenUnit::Nirugu,
                WrittenUnit::Zwnj,
                WrittenUnit::Zr,
                WrittenUnit::I,
            ],
            "\u{180A}\u{1841}\u{1822}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::Cr, UnitPosition::Init),
            ],
            &[WrittenUnit::Nirugu, WrittenUnit::Zwnj, WrittenUnit::Cr],
            "\u{180A}\u{1823}\u{1823}\u{180B}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::Cr, UnitPosition::Init),
                rec(WrittenUnit::I, UnitPosition::Fina),
            ],
            &[
                WrittenUnit::Nirugu,
                WrittenUnit::Zwnj,
                WrittenUnit::Cr,
                WrittenUnit::I,
            ],
            "\u{180A}\u{1823}\u{1823}\u{1822}",
        ),
        // All four positions of `A` (plus `Zr:init` / `Cr:init` / `Aa:isol`) flanked by Nirugu on both sides.
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::A, UnitPosition::Isol),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[
                WrittenUnit::Nirugu,
                WrittenUnit::Zwnj,
                WrittenUnit::A,
                WrittenUnit::Zwnj,
                WrittenUnit::Nirugu,
            ],
            "\u{180A}\u{200C}\u{1820}\u{180B}\u{200C}\u{180A}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::A, UnitPosition::Init),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[
                WrittenUnit::Nirugu,
                WrittenUnit::Zwnj,
                WrittenUnit::A,
                WrittenUnit::Nirugu,
            ],
            "\u{180A}\u{200C}\u{1820}\u{180B}\u{180A}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::A, UnitPosition::Medi),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[WrittenUnit::Nirugu, WrittenUnit::A, WrittenUnit::Nirugu],
            "\u{180A}\u{1820}\u{180A}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::A, UnitPosition::Fina),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[
                WrittenUnit::Nirugu,
                WrittenUnit::A,
                WrittenUnit::Zwnj,
                WrittenUnit::Nirugu,
            ],
            "\u{180A}\u{1820}\u{180C}\u{200C}\u{180A}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::Aa, UnitPosition::Isol),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[
                WrittenUnit::Nirugu,
                WrittenUnit::Mvs,
                WrittenUnit::Aa,
                WrittenUnit::Zwnj,
                WrittenUnit::Nirugu,
            ],
            "\u{180A}\u{180E}\u{1820}\u{200C}\u{180A}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::Zr, UnitPosition::Init),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[
                WrittenUnit::Nirugu,
                WrittenUnit::Zwnj,
                WrittenUnit::Zr,
                WrittenUnit::Nirugu,
            ],
            "\u{180A}\u{1841}\u{180A}",
        ),
        (
            &[
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
                rec(WrittenUnit::Cr, UnitPosition::Init),
                rec(WrittenUnit::Nirugu, UnitPosition::Control),
            ],
            &[
                WrittenUnit::Nirugu,
                WrittenUnit::Zwnj,
                WrittenUnit::Cr,
                WrittenUnit::Nirugu,
            ],
            "\u{180A}\u{1823}\u{1823}\u{180A}",
        ),
    ];
    for &(records, units, expected) in cases {
        let actual = positioned(&shaper, records).unwrap();
        assert_eq!(actual, plain(&shaper, units), "{records:?}");
        assert_eq!(actual, expected, "{records:?}");
    }
}
