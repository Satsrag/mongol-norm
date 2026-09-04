//! Data-driven regression test against mongfontbuilder's `core-hud.tsv`
//! (port of `python/tests/test_core_hud.py`).
//!
//! Like `tests/eac_hud.rs`, the expectations are the standard's written units, so they are
//! compared against `shape_raw`: 29 of these 177 rows spell `Dd`, `H` or `Hx`, which the public
//! `shape` folds into `O A` / `A A` / `N N`.

mod common;

use common::{
    aliases_to_words, hex, load_tsv, normalize_expected, row_hex, shape_aliases_raw, unit_names,
};
use mongol_norm::{Locale, Shaper};

#[test]
fn core_hud_all() {
    let shaper = Shaper::new(Locale::Mng);
    let rows = load_tsv("data/core-hud.tsv");
    assert_eq!(rows.len(), 177, "core-hud.tsv row count changed");
    let mut failures = Vec::new();
    for (index, aliases, expected) in &rows {
        // Every word must shape the same way through `trace` as through `shape` — the trace is
        // the phase-trace golden's verifier, so the two entry points must not drift apart.
        for word in aliases_to_words(aliases) {
            if word.is_empty() {
                continue;
            }
            assert_eq!(
                shaper.trace(&word).unwrap().shape,
                shaper.shape(&word).unwrap(),
                "{index}: trace/shape disagree on {}",
                hex(&word)
            );
        }
        let actual = unit_names(&shape_aliases_raw(&shaper, aliases));
        // `normalize_expected` is the eac superset; core-hud's expected column only ever uses
        // the `_` / `-` MVS spellings today, so the extra `Ni`/artifact arms are a no-op here.
        let expected = normalize_expected(expected);
        if actual != expected {
            failures.push(format!(
                "{index:10}  input={aliases:?}\n            hex     {}\n            got     {actual:?}\n            expect  {expected:?}",
                row_hex(aliases)
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
