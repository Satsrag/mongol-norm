//! Public written-unit normalization API — port of
//! `tests/test_written_units_api.py::TestNormalizeWrittenUnits`.

mod common;

use common::unit_names;
use mongol_norm::{Error, Locale, Shaper, WrittenUnit};

fn shaper() -> Shaper {
    Shaper::new(Locale::Mng)
}

#[test]
fn test_shape_outputs_pascal_case_controls() {
    let shaper = shaper();
    assert_eq!(unit_names(&shaper.shape("\u{180E}").unwrap()), ["Mvs"]);
    assert_eq!(
        unit_names(&shaper.shape("\u{180A}\u{1823}").unwrap()),
        ["Nirugu", "U"]
    );
    assert_eq!(
        unit_names(&shaper.shape("\u{200D}\u{1833}").unwrap()),
        ["Zwj", "Dd"]
    );
}

#[test]
fn test_does_not_insert_unrequested_zwj() {
    // O has connected-position encodings but no isolated encoding. The API must not invent a
    // surrounding ZWJ to make this singleton encodable.
    let error = shaper()
        .normalize_written_units(&[WrittenUnit::O])
        .unwrap_err();
    assert_eq!(error, Error::NoCanonicalEncoding);
    assert!(error.to_string().contains("no canonical MNG encoding"));
}

#[test]
fn test_empty_sequence_encodes_as_empty_text() {
    assert_eq!(shaper().normalize_written_units(&[]).unwrap(), "");
}

#[test]
fn test_shape_output_is_accepted_directly() {
    let shaper = shaper();
    let nominal = "\u{1832}\u{1820}\u{182F}\u{180E}\u{1820}";
    let units = shaper.shape(nominal).unwrap();
    assert_eq!(
        shaper.normalize_written_units(&units).unwrap(),
        shaper.normalize(nominal).unwrap()
    );
}

#[test]
fn test_existing_velar_feminine_refinement_is_reused() {
    let shaper = shaper();
    let nominal = "\u{182C}\u{180C}\u{1826}"; // h+FVS2 + ue -> G Ue
    let units = shaper.shape(nominal).unwrap();
    let result = shaper.normalize_written_units(&units).unwrap();
    assert_eq!(result, shaper.normalize(nominal).unwrap());
    assert_eq!(shaper.shape(&result).unwrap(), units);
}

#[test]
fn test_pascal_case_controls_reuse_structural_chain_encoding() {
    let shaper = shaper();
    for nominal in [
        "\u{1832}\u{1820}\u{182F}\u{180E}\u{1820}", // tal + MVS + a
        "\u{180A}\u{1823}",                         // nirugu + o
        "\u{200D}\u{1823}",                         // ZWJ + o
    ] {
        let units = shaper.shape(nominal).unwrap();
        let result = shaper.normalize_written_units(&units).unwrap();
        assert_eq!(result, shaper.normalize(nominal).unwrap());
        assert_eq!(shaper.shape(&result).unwrap(), units);
    }
}

#[test]
fn test_rejects_unknown_unit_with_its_index() {
    // `E` is a Todo written unit — unknown to the MNG normalize table.
    let error = shaper()
        .normalize_written_units(&[WrittenUnit::B, WrittenUnit::E])
        .unwrap_err();
    assert_eq!(
        error,
        Error::UnsupportedWrittenUnit {
            index: 1,
            unit: WrittenUnit::E
        }
    );
    assert_eq!(error.to_string(), "written_units[1] is unknown: 'E'");
}

#[test]
fn test_rejects_old_control_spellings() {
    // Python calls `normalize_written_units(["MVS"])` — untypeable against a `WrittenUnit` enum,
    // so this goes through `parse_written_units` instead. Differentially verified against
    // `shaper.py::_parse_written_units`: four spellings are outright unknown, while `NIRUGU` and
    // `ZWJ` happen to have a unique compact segmentation into ordinary single-letter units. The
    // property the Python test guards holds either way — no old spelling names a control unit.
    let shaper = shaper();
    for control in ["MVS", "mvs", "nirugu", "zwj"] {
        let error = shaper.parse_written_units(control).unwrap_err();
        assert_eq!(
            error,
            Error::UnknownWrittenUnit {
                index: 0,
                unit: control.to_owned()
            },
            "{control}"
        );
        assert!(error.to_string().contains("is unknown"));
    }
    assert_eq!(
        shaper.parse_written_units("NIRUGU").unwrap(),
        [
            WrittenUnit::N,
            WrittenUnit::I,
            WrittenUnit::R,
            WrittenUnit::U,
            WrittenUnit::G,
            WrittenUnit::U
        ]
    );
    assert_eq!(
        shaper.parse_written_units("ZWJ").unwrap(),
        [WrittenUnit::Z, WrittenUnit::W, WrittenUnit::J]
    );
    for control in ["MVS", "mvs", "NIRUGU", "nirugu", "ZWJ", "zwj"] {
        let parsed = shaper.parse_written_units(control).unwrap_or_default();
        assert!(
            !parsed.iter().any(|unit| unit.is_structural()),
            "{control} parsed to a control unit: {:?}",
            unit_names(&parsed)
        );
    }
}

#[test]
fn test_plain_units_encode_to_canonical_unicode() {
    let shaper = shaper();
    let result = shaper
        .normalize_written_units(&[WrittenUnit::B, WrittenUnit::Aa])
        .unwrap();
    assert_eq!(result, "\u{182A}\u{1820}\u{180B}");
    assert_eq!(
        shaper.shape(&result).unwrap(),
        [WrittenUnit::B, WrittenUnit::Aa]
    );
}

#[test]
fn test_parse_written_units_forms() {
    let shaper = shaper();
    let parse = |text: &str| shaper.parse_written_units(text);
    assert_eq!(parse("").unwrap(), Vec::<WrittenUnit>::new());
    assert_eq!(parse("B+Aa").unwrap(), [WrittenUnit::B, WrittenUnit::Aa]);
    assert_eq!(parse("B+Aa\n").unwrap(), [WrittenUnit::B, WrittenUnit::Aa]);
    assert_eq!(parse("BZwj").unwrap(), [WrittenUnit::B, WrittenUnit::Zwj]);
    assert_eq!(
        parse("AAaBZwj").unwrap(),
        [
            WrittenUnit::A,
            WrittenUnit::Aa,
            WrittenUnit::B,
            WrittenUnit::Zwj
        ]
    );
    assert_eq!(
        parse("Unknown").unwrap_err(),
        Error::UnknownWrittenUnit {
            index: 0,
            unit: "Unknown".to_owned()
        }
    );
    assert_eq!(
        parse("B+Unknown").unwrap_err(),
        Error::UnknownWrittenUnit {
            index: 1,
            unit: "Unknown".to_owned()
        }
    );
    for bad in ["B++Aa", " B+Aa", "B+Aa ", "\tB+Aa", "A A", "A A+B"] {
        let error = parse(bad).unwrap_err();
        assert!(
            matches!(error, Error::InvalidUnitSpec(_)),
            "{bad:?}: {error:?}"
        );
        assert!(error.to_string().contains("whitespace"), "{bad:?}: {error}");
    }
}

/// A name may be a `WrittenUnit` variant and still be outside this locale's table (`E` is Todo).
/// Python rejects the FIRST such name, so `parse_written_units` must test the table's vocabulary
/// rather than the enum — otherwise the reported index drifts past it to a later offender.
/// Differentially verified against `shaper.py::_parse_written_units` + `normalize_written_units`.
#[test]
fn test_parse_rejects_units_outside_this_locales_table_at_pythons_index() {
    let shaper = shaper();
    for (spec, index, unit) in [
        ("E", 0, "E"),
        ("B+E", 1, "E"),
        ("E+B", 0, "E"),
        ("E+Unknown", 0, "E"),
        ("Unknown+E", 0, "Unknown"),
        ("B+E+Unknown", 1, "E"),
    ] {
        assert_eq!(
            shaper.parse_written_units(spec).unwrap_err(),
            Error::UnknownWrittenUnit {
                index,
                unit: unit.to_owned()
            },
            "{spec}"
        );
    }
}
