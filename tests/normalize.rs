//! normalize() / normalize_text() / NNBSP tests — port of `python/tests/test_shaper.py`
//! (`TestNormalize`, `TestNormalizeText`, the normalize half of `TestNNBSP`).

mod common;

use common::mgl;
use mongol_norm::{Locale, Shaper};

const NNBSP: &str = "\u{202F}";
const MVS: &str = "\u{180E}";

fn shaper() -> Shaper {
    Shaper::new(Locale::Mng)
}

fn normalize(text: &str) -> String {
    shaper().normalize(text).unwrap()
}

fn normalize_text(text: &str) -> String {
    shaper().normalize_text(text).unwrap()
}

/// Canonical "sain" under the FVS-pinned per-unit encoder: `s a i+fvs3 i+fvs3 a+fvs2`.
fn canonical_sain() -> String {
    mgl("s a i fvs3 i fvs3 a fvs2")
}

fn sain_variants() -> [String; 5] {
    [
        mgl("s a i n"),
        mgl("s e i n"),
        mgl("s n fvs2 i i n"),
        mgl("s a y fvs1 i n"),
        mgl("s a y fvs1 y fvs1 n"),
    ]
}

// ── TestNormalize ──────────────────────────────────────────────────────────

#[test]
fn test_sain_base() {
    assert_eq!(normalize(&mgl("s a i n")), canonical_sain());
}

#[test]
fn test_sain_e_variant() {
    assert_eq!(normalize(&mgl("s e i n")), canonical_sain());
}

#[test]
fn test_sain_ya_fvs1() {
    assert_eq!(normalize(&mgl("s a y fvs1 i n")), canonical_sain());
}

#[test]
fn test_sain_ya_fvs1_ya_fvs1() {
    assert_eq!(normalize(&mgl("s a y fvs1 y fvs1 n")), canonical_sain());
}

#[test]
fn test_normalize_idempotent() {
    for word in [
        mgl("s a i n"),
        mgl("s e i n"),
        mgl("n a i fvs3 m a"),
        mgl("o r o n"),
    ] {
        let n1 = normalize(&word);
        let n2 = normalize(&n1);
        assert_eq!(n1, n2, "not idempotent: {word:?} → {n1:?} → {n2:?}");
    }
}

#[test]
fn test_normalized_same_shape_as_original() {
    let shaper = shaper();
    for word in &sain_variants()[1..] {
        assert!(shaper
            .same_shape(word, &shaper.normalize(word).unwrap())
            .unwrap());
    }
}

// ── TestNormalizeText ──────────────────────────────────────────────────────

#[test]
fn test_single_word_matches_normalize() {
    for word in [
        mgl("s a i n"),
        mgl("s e i n"),
        mgl("s a y fvs1 i n"),
        mgl("n a i fvs3 m a"),
        mgl("o r o n"),
    ] {
        assert_eq!(normalize_text(&word), normalize(&word));
    }
}

#[test]
fn test_two_words_space_separated() {
    let (a, b) = (mgl("s e i n"), mgl("n a i fvs3 m a"));
    assert_eq!(
        normalize_text(&format!("{a} {b}")),
        format!("{} {}", normalize(&a), normalize(&b))
    );
}

#[test]
fn test_space_preserved() {
    let text = format!("{}  {}", mgl("s a i n"), mgl("n a i fvs3 m a")); // double space
    assert!(normalize_text(&text).contains("  "));
}

#[test]
fn test_mixed_script() {
    let text = format!("Hello {} world", mgl("s e i n"));
    assert_eq!(
        normalize_text(&text),
        format!("Hello {} world", canonical_sain())
    );
}

#[test]
fn test_punctuation_preserved() {
    let result = normalize_text(&format!("{}, {}!", mgl("s e i n"), mgl("n a i fvs3 m a")));
    assert!(result.contains(',') && result.contains('!') && result.contains(' '));
}

#[test]
fn test_empty_string() {
    assert_eq!(normalize_text(""), "");
}

#[test]
fn test_no_mongolian() {
    assert_eq!(normalize_text("Hello, world! 123"), "Hello, world! 123");
}

#[test]
fn test_normalize_text_idempotent() {
    let text = format!("{} {}", mgl("s e i n"), mgl("n a i fvs3 m a"));
    let n1 = normalize_text(&text);
    assert_eq!(normalize_text(&n1), n1);
}

#[test]
fn test_numbers_preserved() {
    let (a, b) = (mgl("s e i n"), mgl("n a i fvs3 m a"));
    let result = normalize_text(&format!("{a} 123 {b}"));
    assert!(result.contains("123"));
    assert_eq!(result, format!("{} 123 {}", normalize(&a), normalize(&b)));
}

#[test]
fn test_symbols_preserved() {
    let result = normalize_text(&format!("#{} @world", mgl("s e i n")));
    assert!(result.starts_with('#'));
    assert!(result.contains("@world"));
}

#[test]
fn test_multiword_each_word_independent() {
    let words = [mgl("s e i n"), mgl("o r o n"), mgl("n a i fvs3 m a")];
    let expected = words
        .iter()
        .map(|w| normalize(w))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(normalize_text(&words.join(" ")), expected);
}

// ── TestNNBSP (normalize half) ─────────────────────────────────────────────

#[test]
fn test_nnbsp_converted_to_mvs_in_normalize() {
    let result = normalize(&format!("{}{NNBSP}{}", mgl("s a i n"), mgl("a")));
    assert!(
        result.contains(MVS),
        "NNBSP must be normalized to MVS in output"
    );
    assert!(
        !result.contains(NNBSP),
        "NNBSP must not survive normalization"
    );
}

#[test]
fn test_mvs_stays_mvs_in_normalize() {
    let result = normalize(&format!("{}{MVS}{}", mgl("s a i n"), mgl("a")));
    assert!(result.contains(MVS));
    assert!(!result.contains(NNBSP));
}

#[test]
fn test_nnbsp_in_mongolian_run() {
    let result = normalize_text(&format!("{}{NNBSP}{}", mgl("s a i n"), mgl("a")));
    assert!(result.contains(MVS));
    assert!(!result.contains(NNBSP));
}

#[test]
fn test_nnbsp_normalize_text_matches_normalize() {
    let text = format!("{}{NNBSP}{}", mgl("s a i n"), mgl("a"));
    assert_eq!(normalize_text(&text), normalize(&text));
}

#[test]
fn test_nnbsp_mixed_with_spaces() {
    let text = format!(
        "{}{NNBSP}{} {}",
        mgl("s a i n"),
        mgl("a"),
        mgl("n a i fvs3 m a")
    );
    let result = normalize_text(&text);
    assert!(result.contains(MVS));
    assert!(!result.contains(NNBSP));
    assert!(result.contains(' '));
}

#[test]
fn test_nnbsp_normalize_idempotent() {
    let text = format!("{}{NNBSP}{}", mgl("s a i n"), mgl("a"));
    let n1 = normalize(&text);
    assert_eq!(normalize(&n1), n1);
}

#[test]
fn test_nnbsp_normalize_text_idempotent() {
    let text = format!(
        "{}{NNBSP}{} {}",
        mgl("s a i n"),
        mgl("a"),
        mgl("n a i fvs3 m a")
    );
    let n1 = normalize_text(&text);
    assert_eq!(normalize_text(&n1), n1);
}
