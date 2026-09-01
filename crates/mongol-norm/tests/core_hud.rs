//! Data-driven regression test against mongfontbuilder's `core-hud.tsv`
//! (port of `tests/test_core_hud.py`).

mod common;

use common::{load_tsv, normalize_expected, shape_aliases, unit_names};
use mongol_norm::{Locale, Shaper};

#[test]
fn core_hud_all() {
    let shaper = Shaper::new(Locale::Mng);
    let rows = load_tsv("data/core-hud.tsv");
    assert_eq!(rows.len(), 177, "core-hud.tsv row count changed");
    let mut failures = Vec::new();
    for (index, aliases, expected) in &rows {
        let actual = unit_names(&shape_aliases(&shaper, aliases));
        let expected = normalize_expected(expected);
        if actual != expected {
            failures.push(format!(
                "{index:10}  input={aliases:?}\n            got     {actual:?}\n            expect  {expected:?}"
            ));
        }
    }
    if !failures.is_empty() {
        eprintln!("\n{} / {} failed:", failures.len(), rows.len());
        for failure in &failures {
            eprintln!("{failure}");
        }
        panic!(
            "{} of {} core-hud cases failed (see above)",
            failures.len(),
            rows.len()
        );
    }
    eprintln!("\nAll {} core-hud cases passed.", rows.len());
}
