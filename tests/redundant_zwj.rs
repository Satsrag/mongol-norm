//! Redundant ZWJ (issue #33): a ZWJ that changes no glyph is dropped from the public shape, so
//! same ink ⟹ same `shape` ⟹ same `normalize` holds across joiner spellings. Port of
//! `python/tests/test_redundant_zwj.py`.

mod common;

use common::unit_names;
use mongol_norm::{Locale, Shaper, WrittenUnit};

const ZWJ: &str = "\u{200D}";
const NIRUGU: &str = "\u{180A}";
const MVS: &str = "\u{180E}";
const FVS1: &str = "\u{180B}";
const FVS2: &str = "\u{180C}";
const A: &str = "\u{1820}";
const E: &str = "\u{1821}";
const I: &str = "\u{1822}";
const O: &str = "\u{1823}";
const U: &str = "\u{1824}";
const UE: &str = "\u{1826}";
const N: &str = "\u{1828}";
const B: &str = "\u{182A}";
const L: &str = "\u{182F}";
const S: &str = "\u{1830}";
const T: &str = "\u{1832}";
const D: &str = "\u{1833}";
const R: &str = "\u{1837}";
const Y: &str = "\u{1836}";
const P: &str = "\u{182B}";

fn shaper() -> Shaper {
    Shaper::new(Locale::Mng)
}

fn shape(text: &str) -> Vec<String> {
    unit_names(&shaper().shape(text).unwrap())
}

/// Both spellings shape and normalize identically, and the canonical text round-trips.
fn assert_same_word(with_zwj: &str, without: &str) {
    let shaper = shaper();
    assert_eq!(
        shape(with_zwj),
        shape(without),
        "{with_zwj:?} vs {without:?}"
    );
    assert!(shaper.same_shape(with_zwj, without).unwrap());
    let norm = shaper.normalize(with_zwj).unwrap();
    assert_eq!(norm, shaper.normalize(without).unwrap());
    assert_eq!(
        shaper.shape(&norm).unwrap(),
        shaper.shape(with_zwj).unwrap()
    );
    assert_eq!(shaper.normalize(&norm).unwrap(), norm, "idempotent");
}

/// The ZWJ matters: shapes differ, the `Zwj` unit survives and normalize keeps the joiner.
fn assert_zwj_kept(with_zwj: &str, without: &str) {
    let shaper = shaper();
    assert_ne!(
        shape(with_zwj),
        shape(without),
        "{with_zwj:?} vs {without:?}"
    );
    assert!(shape(with_zwj).iter().any(|unit| unit == "Zwj"));
    let norm = shaper.normalize(with_zwj).unwrap();
    assert!(norm.contains(ZWJ), "{with_zwj:?} -> {norm:?}");
    assert_eq!(
        shaper.shape(&norm).unwrap(),
        shaper.shape(with_zwj).unwrap()
    );
}

// ── segment interior ──

#[test]
fn interior_zwj_between_joined_letters_is_dropped() {
    assert_same_word(
        &format!("{L}{I}{ZWJ}{E}{FVS1}"),
        &format!("{L}{I}{E}{FVS1}"),
    );
    assert_eq!(shape(&format!("{L}{I}{ZWJ}{E}{FVS1}")), ["L", "G"]);
    assert_same_word(&format!("{A}{R}{A}{ZWJ}{D}"), &format!("{A}{R}{A}{D}"));
    assert_same_word(
        &format!("{L}{A}{L}{A}{ZWJ}{P}{UE}{D}"),
        &format!("{L}{A}{L}{A}{P}{UE}{D}"),
    );
}

#[test]
fn interior_zwj_does_not_reattach_an_orphan_fvs() {
    // The FVS after the ZWJ belongs to no letter: dropping the token must not turn `a ZWJ FVS1 a`
    // into `a FVS1 a`. The joiner is removed from the resolved units, so the orphan selector
    // stays an orphan and the word shapes like `a ZWJ a`.
    assert_eq!(shape(&format!("{A}{ZWJ}{FVS1}{A}")), ["A", "A", "A"]);
    assert_eq!(
        shape(&format!("{A}{ZWJ}{FVS1}{A}")),
        shape(&format!("{A}{ZWJ}{A}"))
    );
    assert_ne!(
        shape(&format!("{A}{ZWJ}{FVS1}{A}")),
        shape(&format!("{A}{FVS1}{A}"))
    );
}

#[test]
fn zwj_next_to_a_nirugu_is_dropped() {
    assert_same_word(&format!("{A}{L}{ZWJ}{NIRUGU}"), &format!("{A}{L}{NIRUGU}"));
    assert_same_word(&format!("{NIRUGU}{ZWJ}{L}{A}"), &format!("{NIRUGU}{L}{A}"));
    assert_same_word(&format!("{A}{L}{NIRUGU}{ZWJ}"), &format!("{A}{L}{NIRUGU}"));
    assert_same_word(&format!("{ZWJ}{NIRUGU}{U}"), &format!("{NIRUGU}{U}"));
    assert_same_word(
        &format!("{A}{L}{NIRUGU}{ZWJ}{MVS}{A}"),
        &format!("{A}{L}{NIRUGU}{MVS}{A}"),
    );
}

#[test]
fn repeated_zwj_keeps_one() {
    assert_same_word(&format!("{ZWJ}{ZWJ}{N}{A}"), &format!("{ZWJ}{N}{A}"));
    assert_same_word(&format!("{A}{ZWJ}{ZWJ}"), &format!("{A}{ZWJ}"));
    assert_same_word(
        &format!("{L}{I}{ZWJ}{ZWJ}{E}{FVS1}"),
        &format!("{L}{I}{E}{FVS1}"),
    );
    assert_eq!(shape(&format!("{ZWJ}{ZWJ}")), ["Zwj"]);
}

#[test]
fn zwj_after_mvs_follows_the_word_initial_table() {
    // The letter after an MVS is segment-initial, so the word-initial lists decide; the MVS
    // placeholder box the font draws before a joiner is not text ink.
    assert_same_word(
        &format!("{A}{MVS}{ZWJ}{Y}{I}{N}"),
        &format!("{A}{MVS}{Y}{FVS2}{I}{N}"),
    ); // Y:medi ≡ Y:init
    assert_eq!(
        shape(&format!("{A}{MVS}{ZWJ}{Y}{I}{N}")),
        ["A", "A", "Mvs", "Y", "I", "A"]
    );
    assert_same_word(&format!("{A}{MVS}{ZWJ}{B}{A}"), &format!("{A}{MVS}{B}{A}")); // B:medi ≡ B:init
    assert_zwj_kept(&format!("{A}{MVS}{ZWJ}{N}{A}"), &format!("{A}{MVS}{N}{A}")); // N:medi ≠ N:init
                                                                                  // Lone A after the MVS: A:fina is not the suffix a.
    assert_zwj_kept(
        &format!("{T}{A}{L}{MVS}{ZWJ}{A}"),
        &format!("{T}{A}{L}{MVS}{A}"),
    );
    assert_eq!(
        shape(&format!("{T}{A}{L}{MVS}{ZWJ}{A}")),
        ["T", "A", "L", "Mvs", "Zwj", "A"]
    );
}

#[test]
fn zwj_before_mvs_follows_the_word_final_table() {
    // The letter before an MVS is segment-final, so the word-final lists decide.
    assert_zwj_kept(&format!("{A}{L}{ZWJ}{MVS}{A}"), &format!("{A}{L}{MVS}{A}")); // L:medi ≠ L:fina
    assert_eq!(
        shape(&format!("{A}{L}{ZWJ}{MVS}{A}")),
        ["A", "A", "L", "Zwj", "Mvs", "Aa"]
    );
    assert_same_word(
        &format!("{A}{O}{ZWJ}{MVS}{A}"),
        &format!("{A}{O}{FVS1}{MVS}{A}"),
    ); // O:medi ≡ O:fina
    assert_eq!(
        shape(&format!("{A}{O}{ZWJ}{MVS}{A}")),
        ["A", "A", "O", "Mvs", "Aa"]
    );
    assert_same_word(&format!("{B}{ZWJ}{MVS}{A}"), &format!("{B}{MVS}{A}")); // lone B:init ≡ B:isol
    assert_zwj_kept(&format!("{A}{ZWJ}{MVS}{A}"), &format!("{A}{MVS}{A}")); // lone A:init ≠ A:isol
    assert_same_word(&format!("{ZWJ}{B}{ZWJ}{MVS}{A}"), &format!("{B}{MVS}{A}"));
    // fixed point
}

// ── word edges: the reviewed equivalence table ──

#[test]
fn word_initial_zwj_follows_the_reviewed_table() {
    // B:medi ≡ B:init (the ligature is drawn the same), S:medi ≡ S:init (accepted), N differs.
    assert_same_word(&format!("{ZWJ}{B}{A}"), &format!("{B}{A}"));
    assert_eq!(shape(&format!("{ZWJ}{B}{A}")), ["B", "Aa"]);
    assert_same_word(&format!("{ZWJ}{S}{A}"), &format!("{S}{A}"));
    assert_same_word(&format!("{ZWJ}{D}{A}"), &format!("{D}{A}"));
    assert_same_word(&format!("{ZWJ}{B}{A}{R}"), &format!("{B}{A}{R}"));
    assert_zwj_kept(&format!("{ZWJ}{N}{A}"), &format!("{N}{A}"));
    assert_zwj_kept(&format!("{ZWJ}{L}{A}"), &format!("{L}{A}"));
    assert_zwj_kept(&format!("{ZWJ}{A}{A}"), &format!("{A}{A}"));
}

#[test]
fn word_initial_zwj_before_a_lone_letter_follows_the_reviewed_table() {
    // I:fina ≡ I:isol and U:fina ≡ U:isol; a lone `Dd:fina` is `O A` either way.
    assert_same_word(&format!("{ZWJ}{I}"), &format!("{I}{FVS1}"));
    assert_same_word(&format!("{ZWJ}{U}"), &format!("{U}{FVS1}"));
    assert_eq!(shape(&format!("{ZWJ}{D}")), ["O", "A"]);
    // B:fina (a tail) is not the isolated B.
    assert_zwj_kept(&format!("{ZWJ}{B}"), B);
    assert_zwj_kept(&format!("{ZWJ}{A}"), A);
}

#[test]
fn word_final_zwj_follows_the_reviewed_table() {
    // O:medi ≡ O:fina; everything else grows a tail at the end of a word.
    assert_same_word(&format!("{A}{O}{ZWJ}"), &format!("{A}{O}{FVS1}"));
    assert_zwj_kept(&format!("{A}{L}{ZWJ}"), &format!("{A}{L}"));
    assert_zwj_kept(&format!("{A}{ZWJ}"), A);
    assert_eq!(shape(&format!("{A}{ZWJ}")), ["A", "A", "Zwj"]);
}

#[test]
fn word_final_zwj_after_a_lone_consonant_is_dropped() {
    // A consonant's isolated form is its initial unit, so `b ZWJ` is bare `b`.
    for letter in [B, L, N, S, T] {
        assert_same_word(&format!("{letter}{ZWJ}"), letter);
        assert!(!shape(&format!("{letter}{ZWJ}")).contains(&"Zwj".to_owned()));
    }
    assert_eq!(shaper().normalize(&format!("{B}{ZWJ}")).unwrap(), B);
}

#[test]
fn both_edges_resolve_to_a_fixed_point() {
    // Leading first (B:medi ≡ B:init), then — now a lone B:init — the trailing one.
    assert_same_word(&format!("{ZWJ}{B}{ZWJ}"), B);
    assert_eq!(shape(&format!("{ZWJ}{B}{ZWJ}")), ["B"]);
    assert_eq!(shape(&format!("{ZWJ}{A}{ZWJ}")), ["Zwj", "A", "Zwj"]);
}

// ── the written-unit APIs share the contract ──

#[test]
fn normalize_written_units_folds_redundant_zwj_like_shape() {
    use WrittenUnit as W;
    let shaper = shaper();
    for (spelled, folded) in [
        (vec![W::L, W::I, W::Zwj, W::Aa], vec![W::L, W::G]),
        (vec![W::Zwj, W::B, W::Aa], vec![W::B, W::Aa]),
        (vec![W::B, W::Zwj], vec![W::B]),
        (vec![W::Zwj, W::B, W::Zwj], vec![W::B]),
        (
            vec![W::Nirugu, W::Zwj, W::L, W::A],
            vec![W::Nirugu, W::L, W::A],
        ),
        (vec![W::Zwj, W::Zwj, W::N, W::A], vec![W::Zwj, W::N, W::A]),
    ] {
        let text = shaper.normalize_written_units(&spelled).unwrap();
        assert_eq!(text, shaper.normalize_written_units(&folded).unwrap());
        assert_eq!(shaper.shape(&text).unwrap(), folded, "{spelled:?}");
    }
    assert_eq!(
        shaper
            .normalize_written_units(&[W::L, W::I, W::Zwj, W::Aa])
            .unwrap(),
        shaper.normalize(&format!("{L}{I}{ZWJ}{E}{FVS1}")).unwrap()
    );
}

#[test]
fn shape_raw_and_diagnostics_keep_every_zwj() {
    use WrittenUnit as W;
    let shaper = shaper();
    assert_eq!(
        shaper.shape_raw(&format!("{L}{I}{ZWJ}{E}{FVS1}")).unwrap(),
        [W::L, W::I, W::Zwj, W::Aa]
    );
    assert_eq!(
        shaper.shape_raw(&format!("{ZWJ}{D}")).unwrap(),
        [W::Zwj, W::Dd]
    );
    let detailed = shaper.shape_detailed(&format!("{ZWJ}{D}")).unwrap();
    assert_eq!(detailed.len(), 2);
    assert_eq!(detailed[0].cp, '\u{200D}');
    let trace = shaper.trace(&format!("{ZWJ}{D}")).unwrap();
    assert_eq!(trace.positions.len(), 2);
    assert_eq!(trace.shape, shaper.shape(&format!("{ZWJ}{D}")).unwrap());
}

#[test]
fn edge_equivalences_are_hudum_only() {
    // The structural rules hold everywhere; the reviewed edge table is MNG data.
    // (`n` is a letter in every locale; Todo has no `b`.)
    for locale in [Locale::Tod, Locale::Sib, Locale::Mch] {
        let shaper = Shaper::new(locale);
        let with = unit_names(&shaper.shape(&format!("{N}{ZWJ}")).unwrap());
        assert!(with.contains(&"Zwj".to_owned()), "{locale:?}: {with:?}");
        let interior = unit_names(&shaper.shape(&format!("{N}{A}{ZWJ}{N}{A}")).unwrap());
        assert!(
            !interior.contains(&"Zwj".to_owned()),
            "{locale:?}: {interior:?}"
        );
    }
}
