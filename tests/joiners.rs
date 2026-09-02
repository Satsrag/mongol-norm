//! Joiner tokens (`Nirugu` / `Zwj`) in shape and normalize — port of `python/tests/test_joiners.py`.

mod common;

use common::unit_names;
use mongol_norm::{Locale, Shaper};

const NIRUGU: &str = "\u{180A}";
const ZWJ: &str = "\u{200D}";
const O: &str = "\u{1823}";
const U: &str = "\u{1824}";
const OE: &str = "\u{1825}";
const D: &str = "\u{1833}";
const J: &str = "\u{1835}";
const FVS1: &str = "\u{180B}";
const FVS3: &str = "\u{180D}";

fn shaper() -> Shaper {
    Shaper::new(Locale::Mng)
}

fn shape(text: &str) -> Vec<String> {
    unit_names(&shaper().shape(text).unwrap())
}

fn round_trips(text: &str) -> String {
    let norm = shaper().normalize(text).unwrap();
    assert_eq!(
        shape(&norm),
        shape(text),
        "round-trip broken for {text:?} -> {norm:?}"
    );
    norm
}

// ── TestJoinerTokensInShape ──
#[test]
fn test_nirugu_is_a_shape_token() {
    assert_eq!(
        shape(&format!("{NIRUGU}{O}{NIRUGU}")),
        ["Nirugu", "O", "Nirugu"]
    );
}

#[test]
fn test_nirugu_run_count_preserved() {
    assert_eq!(
        shape(&format!("{NIRUGU}{NIRUGU}{O}{NIRUGU}")),
        ["Nirugu", "Nirugu", "O", "Nirugu"]
    );
}

#[test]
fn test_zwj_is_a_shape_token() {
    assert_eq!(shape(&format!("{ZWJ}{D}")), ["Zwj", "Dd"]);
}

#[test]
fn test_nirugu_vs_zwj_shapes_differ() {
    // visible stem vs invisible joiner — must NOT be conflated
    assert!(!shaper()
        .same_shape(&format!("{NIRUGU}{D}"), &format!("{ZWJ}{D}"))
        .unwrap());
}

#[test]
fn test_same_letter_form_between_joiners_still_matches() {
    // o and u both render written 'O' at medi — same shape
    assert!(shaper()
        .same_shape(
            &format!("{NIRUGU}{O}{NIRUGU}"),
            &format!("{NIRUGU}{U}{NIRUGU}")
        )
        .unwrap());
}

// ── TestJoinerNormalize ──
#[test]
fn test_nirugu_preserved_letter_canonicalized() {
    // u at medi renders 'O'; canonical letter for (medi, O) is bare o
    assert_eq!(
        round_trips(&format!("{NIRUGU}{U}{NIRUGU}")),
        format!("{NIRUGU}{O}{NIRUGU}")
    );
}

#[test]
fn test_same_shape_same_canonical_across_encodings() {
    // oe+fvs3 at medi also renders 'O' (EAC MOZ10-2) — same canonical
    let shaper = shaper();
    assert_eq!(
        shaper
            .normalize(&format!("{NIRUGU}{OE}{FVS3}{NIRUGU}"))
            .unwrap(),
        shaper.normalize(&format!("{NIRUGU}{O}{NIRUGU}")).unwrap()
    );
}

#[test]
fn test_nirugu_count_preserved() {
    let text = format!("{NIRUGU}{NIRUGU}{O}{NIRUGU}");
    assert_eq!(round_trips(&text), text);
}

#[test]
fn test_zwj_preserved() {
    let text = format!("{ZWJ}{D}");
    assert_eq!(round_trips(&text), text);
}

#[test]
fn test_single_sided_nirugu_round_trips() {
    for text in [
        format!("{NIRUGU}{J}"),       // joined-left J -> fina form
        format!("{NIRUGU}{D}"),       // joined-left Dd
        format!("{U}{FVS1}{NIRUGU}"), // u+fvs1 joined-right (EAC MVS20-1)
    ] {
        round_trips(&text);
    }
}

// ── TestJoinerNormalizeText ──
#[test]
fn test_nirugu_word_uses_the_same_joining_context_as_normalize() {
    let shaper = shaper();
    let word = format!("{NIRUGU}{U}{NIRUGU}");
    assert_eq!(
        shaper.normalize_text(&word).unwrap(),
        shaper.normalize(&word).unwrap()
    );
}

#[test]
fn test_nirugu_word_inside_mixed_text_uses_the_same_joining_context() {
    let shaper = shaper();
    let word = format!("{NIRUGU}{U}{NIRUGU}");
    assert_eq!(
        shaper.normalize_text(&format!("A {word} B")).unwrap(),
        format!("A {} B", shaper.normalize(&word).unwrap())
    );
}

#[test]
fn test_zwj_word_uses_the_same_joining_context_as_normalize() {
    let shaper = shaper();
    let word = format!("{ZWJ}{D}");
    assert_eq!(
        shaper.normalize_text(&word).unwrap(),
        shaper.normalize(&word).unwrap()
    );
}
