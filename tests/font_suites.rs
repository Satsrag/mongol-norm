#![cfg(feature = "font")]
//! mongfontbuilder's Hudum conformance suites through the font backend, as of the pinned
//! upstream commit (`fonts/mongfontbuilder.json`): the core suite of the Chinese national
//! standard and the GB/T 25914-2023 EAC suite.
//!
//! The expectations are the standard's unit streams, so they are compared against
//! `shape_raw` (with `Zwj` removed — ZWJ renders no glyph, so the streams never name it).
//! The EAC rows mongfontbuilder itself marks `xfail` are skipped here, and checked to still
//! fail, so the list cannot go stale.

mod common;

use common::{load_tsv, normalize_font_expected, row_hex, shape_font_row, unit_names};
use mongol_norm::FontShaper;

/// (suite file, rows) — the row counts pin the vendored fixtures.
const SUITES: &[(&str, usize)] = &[("core-hudum", 185), ("eac-hudum", 3512)];

/// The EAC rows the font answers differently from the EAC spec, as mongfontbuilder marks
/// them (`tests/fixtures.py`, `EAC_XFAIL`); the UTN model is taken as correct there. NNBSP
/// keeps its old MVS behaviour (`XIM11-39`, `XIM11-40`); a letter before an MVS is final, so
/// an FVS after it selects the final variant (`XIM11-1012`); the FVS3 after `n` selects the
/// form the UTN model draws rather than the one the EAC expects (`XIM11-46`). Upstream also
/// lists `XIM11-41`, whose only difference is the space glyph the font draws for the NNBSP —
/// `Mvs` either way in the crate's units, so it passes here.
const EAC_XFAIL: &[&str] = &["XIM11-39", "XIM11-40", "XIM11-46", "XIM11-1012"];

fn shape_row(shaper: &FontShaper, row: &str) -> Vec<String> {
    let mut units = unit_names(&shape_font_row(row, |word| {
        shaper
            .shape_raw(word)
            .unwrap_or_else(|e| panic!("shape_raw({row:?}) failed: {e}"))
    }));
    units.retain(|unit| unit != "Zwj");
    units
}

#[test]
fn every_suite_passes() {
    let shaper = FontShaper::new();
    let mut total = 0;
    for &(suite, count) in SUITES {
        let rows = load_tsv(&format!("data/mongfontbuilder/{suite}.tsv"));
        assert_eq!(rows.len(), count, "{suite}.tsv row count changed");
        let mut failures = Vec::new();
        let mut xfailed = 0;
        for (index, row, expected) in &rows {
            if suite == "eac-hudum" && EAC_XFAIL.contains(&index.as_str()) {
                xfailed += 1;
                continue;
            }
            let actual = shape_row(&shaper, row);
            let expected = normalize_font_expected(expected);
            if actual != expected {
                failures.push(format!(
                    "{suite} {index:14}  input={row:?}\n    hex     {}\n    got     {actual:?}\n    expect  {expected:?}",
                    row_hex(row)
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
    let shaper = FontShaper::new();
    for (index, row, expected) in &load_tsv("data/mongfontbuilder/eac-hudum.tsv") {
        if !EAC_XFAIL.contains(&index.as_str()) {
            continue;
        }
        assert_ne!(
            shape_row(&shaper, row),
            normalize_font_expected(expected),
            "{index} passes now ({}); remove it from EAC_XFAIL",
            row_hex(row)
        );
    }
}
