//! GB/T 25914-2023 EAC compliance suite for Hudum, cross-implementation against mongfontbuilder's
//! `eac-hud.tsv` (port of `python/tests/test_eac_hud.py`). The 5 UTN-vs-EAC divergences are skipped
//! exactly as mongfontbuilder marks them `xfail`; their UTN-correct shaping is pinned in
//! `tests/shaper.rs`.
//!
//! The standard keeps all nine duplicate written units, whereas the public `shape` unifies each
//! with the sequence it renders as — a different answer for 375 of these rows — so this suite
//! compares against `shape_raw`, the engine's own sequence, and not the unified public shape.

mod common;

use common::{
    load_tsv, normalize_expected, row_hex, shape_aliases_raw, unit_names, UTN_XFAIL_CASES,
};
use mongol_norm::{Locale, Shaper};

#[test]
fn eac_hud_all() {
    let shaper = Shaper::new(Locale::Mng);
    let rows = load_tsv("data/eac-hud.tsv");
    assert_eq!(rows.len(), 3512, "eac-hud.tsv row count changed");
    let mut failures = Vec::new();
    let mut utn_xfailed = 0;
    for (index, aliases, expected) in &rows {
        if UTN_XFAIL_CASES.contains(&index.as_str()) {
            utn_xfailed += 1;
            continue;
        }
        // ZWJ is zero-width (it renders no glyph), so EAC's expected stream never names it —
        // drop our `Zwj` units before comparing, exactly like `python/tests/test_eac_hud.py`.
        let mut actual = unit_names(&shape_aliases_raw(&shaper, aliases));
        actual.retain(|unit| unit != "Zwj");
        let expected = normalize_expected(expected);
        if actual != expected {
            failures.push(format!(
                "{index:14}  input={aliases:?}\n                hex     {}\n                got     {actual:?}\n                expect  {expected:?}",
                row_hex(aliases)
            ));
        }
    }
    assert_eq!(
        utn_xfailed,
        UTN_XFAIL_CASES.len(),
        "every xfail id must exist in the fixture"
    );
    let checked = rows.len() - utn_xfailed;
    let passed = checked - failures.len();
    eprintln!(
        "\n{passed} / {checked} passed ({:.1}%); {} failed; {utn_xfailed} UTN-xfail (skipped)",
        100.0 * passed as f64 / checked as f64,
        failures.len()
    );
    if !failures.is_empty() {
        for failure in failures.iter().take(30) {
            eprintln!("{failure}");
        }
        if failures.len() > 30 {
            eprintln!("... ({} more)", failures.len() - 30);
        }
        panic!("{} of {checked} eac-hud cases failed", failures.len());
    }
}
