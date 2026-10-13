#![cfg(feature = "font")]
//! mongfontbuilder's conformance suites through the font backend: every suite of the pinned
//! upstream commit (`fonts/mongfontbuilder.json`), for every writing system the unified font
//! writes — the core suites of the Chinese national standard for Hudum, Todo, Sibe, Manchu
//! and their Ali Gali extensions, and the GB/T 25914-2023 EAC suite for Hudum.
//!
//! The expectations are the standard's unit streams, so they are compared against
//! `shape_raw` (with `Zwj` removed — ZWJ renders no glyph, so the streams never name it).
//! The EAC rows mongfontbuilder itself marks `xfail` for the unified font are skipped here,
//! and checked to still fail, so the list cannot go stale.

mod common;

use common::{load_tsv, normalize_font_expected, row_hex, shape_font_row, unit_names, FontAliases};
use mongol_norm::{FontShaper, Locale};

/// (suite file, alias namespace, locale, rows) — the row counts pin the vendored fixtures.
const SUITES: &[(&str, &str, Locale, usize)] = &[
    ("core-hudum", "MNG", Locale::Mng, 177),
    ("core-hudum-ali-gali", "MNGx", Locale::Mng, 261),
    ("core-todo", "TOD", Locale::Tod, 110),
    ("core-todo-ali-gali", "TODx", Locale::Tod, 113),
    ("core-sibe", "SIB", Locale::Sib, 122),
    ("core-manchu", "MCH", Locale::Mch, 124),
    ("core-manchu-ali-gali", "MCHx", Locale::Mch, 187),
    ("eac-hudum", "MNG", Locale::Mng, 3512),
];

/// The EAC rows the unified font answers differently from the EAC spec, as mongfontbuilder
/// marks them (`tests/fixtures.py`: `EAC_XFAIL` and `EAC_UNIFIED_XFAIL`). The UTN model is
/// taken as correct there: NNBSP keeps its old MVS behaviour (`XIM11-39`, `XIM11-40`); a
/// letter before an MVS is final, so an FVS after it selects the final variant
/// (`XIM11-1012`); and an FVS is valid wherever the character is shared between writing
/// systems, so a Hudum-only reading of its validity does not apply (the rest). Upstream also
/// lists `XIM11-41`, whose only difference is the space glyph the font draws for the NNBSP —
/// `Mvs` either way in the crate's units, so it passes here.
const EAC_XFAIL: &[&str] = &[
    "XIM11-39",
    "XIM11-40",
    "XIM11-46",
    "XIM11-1012",
    "MND11-2",
    "MNM10-2",
    "MNM11-2",
    "MNS11-26",
    "MNZ11-3",
    "MNZ21-5",
    "XIM11-11",
    "XIM11-675",
    "XIM11-678",
    "XIM11-681",
    "XIM11-684",
    "XIM11-687",
    "XIM11-690",
    "XIM11-694",
];

fn shape_row(shaper: &FontShaper, aliases: &FontAliases, row: &str) -> Vec<String> {
    let mut units = unit_names(&shape_font_row(aliases, row, |word| {
        shaper
            .shape_raw(word)
            .unwrap_or_else(|e| panic!("shape_raw({row:?}) failed: {e}"))
    }));
    units.retain(|unit| unit != "Zwj");
    units
}

#[test]
fn every_suite_passes() {
    let mut total = 0;
    for &(suite, namespace, locale, count) in SUITES {
        let rows = load_tsv(&format!("data/mongfontbuilder/{suite}.tsv"));
        assert_eq!(rows.len(), count, "{suite}.tsv row count changed");
        let aliases = FontAliases::load(namespace);
        let shaper = FontShaper::new(locale);
        let mut failures = Vec::new();
        let mut xfailed = 0;
        for (index, row, expected) in &rows {
            if suite == "eac-hudum" && EAC_XFAIL.contains(&index.as_str()) {
                xfailed += 1;
                continue;
            }
            let actual = shape_row(&shaper, &aliases, row);
            let expected = normalize_font_expected(expected);
            if actual != expected {
                failures.push(format!(
                    "{suite} {index:14}  input={row:?}\n    hex     {}\n    got     {actual:?}\n    expect  {expected:?}",
                    aliases
                        .words(row)
                        .iter()
                        .filter(|w| !w.is_empty())
                        .map(|w| common::hex(w))
                        .collect::<Vec<_>>()
                        .join(" | ")
                ));
            }
        }
        if suite == "eac-hudum" {
            assert_eq!(
                xfailed,
                EAC_XFAIL.len(),
                "every xfail id must exist in the fixture"
            );
        }
        if !failures.is_empty() {
            for failure in failures.iter().take(30) {
                eprintln!("{failure}");
            }
            panic!("{} of {} {suite} cases failed", failures.len(), rows.len());
        }
        total += rows.len() - xfailed;
    }
    eprintln!(
        "\nAll {total} suite cases passed ({} EAC rows xfail).",
        EAC_XFAIL.len()
    );
}

/// The xfail list is exactly the rows the font answers differently: a row that started to
/// pass must leave the list.
#[test]
fn the_eac_xfails_still_fail() {
    let rows = load_tsv("data/mongfontbuilder/eac-hudum.tsv");
    let aliases = FontAliases::load("MNG");
    let shaper = FontShaper::new(Locale::Mng);
    for (index, row, expected) in &rows {
        if !EAC_XFAIL.contains(&index.as_str()) {
            continue;
        }
        assert_ne!(
            shape_row(&shaper, &aliases, row),
            normalize_font_expected(expected),
            "{index} passes now ({}); remove it from EAC_XFAIL",
            row_hex(row)
        );
    }
}
