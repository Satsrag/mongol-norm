//! Issue #33, stage one: redundant interior ZWJ, with all edge policies deferred.
use std::collections::HashMap;

use mongol_norm::{Locale, Shaper, WrittenUnit};

const ZWJ: char = '\u{200D}';
const NIRUGU: char = '\u{180A}';

fn equivalent(shaper: &Shaper, a: &str, b: &str) -> String {
    assert!(shaper.same_shape(a, b).unwrap(), "{a:?} / {b:?}");
    let canonical = shaper.normalize(a).unwrap();
    assert_eq!(shaper.normalize(b).unwrap(), canonical, "{a:?} / {b:?}");
    assert_eq!(shaper.shape(&canonical).unwrap(), shaper.shape(a).unwrap());
    assert_eq!(shaper.normalize(&canonical).unwrap(), canonical);
    canonical
}

#[test]
fn joiner_is_removed_before_duplicate_unification() {
    use WrittenUnit::{Aa, Zwj, G, I, L};
    let shaper = Shaper::default();
    let plain = "\u{182F}\u{1822}\u{1821}\u{180B}";
    let joined = "\u{182F}\u{1822}\u{200D}\u{1821}\u{180B}";
    assert_eq!(shaper.shape_raw(joined).unwrap(), [L, I, Zwj, Aa]);
    assert_eq!(shaper.shape(plain).unwrap(), [L, G]);
    assert_eq!(shaper.shape(joined).unwrap(), [L, G]);
    let canonical = equivalent(&shaper, plain, joined);
    assert!(!canonical.contains(ZWJ));
    let trace = shaper.trace(joined).unwrap();
    assert_eq!(trace.shape, [L, G]);
    assert_eq!(trace.positions.len(), 4); // raw diagnostics still include ZWJ
    assert_eq!(shaper.shape_detailed(joined).unwrap()[2].cp, ZWJ);
    assert_eq!(
        shaper
            .normalize_text(&format!("before {joined} after"))
            .unwrap(),
        format!("before {canonical} after")
    );
}

#[test]
fn nirugu_and_repeated_interior_joiners() {
    let shaper = Shaper::default();
    for middle in ["", "\u{180A}", "\u{180A}\u{180A}"] {
        let plain = format!("ᠪ{middle}ᠠ");
        for repeated in [1, 2, 4] {
            let joiners = ZWJ.to_string().repeat(repeated);
            for joined in [
                format!("ᠪ{joiners}{middle}ᠠ"),
                format!("ᠪ{middle}{joiners}ᠠ"),
                format!("ᠪ{joiners}{middle}{joiners}ᠠ"),
            ] {
                let canonical = equivalent(&shaper, &plain, &joined);
                assert_eq!(canonical.matches(NIRUGU).count(), middle.chars().count());
            }
        }
    }
}

#[test]
fn word_edges_short_words_and_control_only_inputs_are_unchanged() {
    let shaper = Shaper::default();
    for word in ["ᠪᠠ", "ᠰᠠ", "ᠨᠠ", "ᠠ"] {
        for (left, right) in [(1, 0), (0, 1), (1, 1), (2, 2)] {
            let text = format!(
                "{}{word}{}",
                ZWJ.to_string().repeat(left),
                ZWJ.to_string().repeat(right)
            );
            let shape = shaper.shape(&text).unwrap();
            assert_eq!(
                shape.iter().filter(|&&u| u == WrittenUnit::Zwj).count(),
                left + right
            );
            assert!(!shaper.same_shape(&text, word).unwrap());
            let canonical = equivalent(&shaper, &text, &text);
            assert_eq!(canonical.matches(ZWJ).count(), left + right);
        }
    }
    for text in [
        "\u{200D}",
        "\u{200D}\u{200D}",
        "\u{180A}\u{200D}",
        "\u{200D}\u{180A}",
        "\u{200D}\u{180A}\u{200D}",
    ] {
        assert_eq!(shaper.normalize(text).unwrap(), text);
    }
    // Nirugu is not evidence of a letter on its other side: no boundary inference here.
    for text in ["\u{180A}\u{200D}ᠠ", "ᠠ\u{200D}\u{180A}"] {
        assert!(shaper.shape(text).unwrap().contains(&WrittenUnit::Zwj));
        equivalent(&shaper, text, text);
    }
}

#[test]
fn mvs_nnbsp_and_unknown_letters_block_interior_classification() {
    let shaper = Shaper::default();
    for separator in ['\u{180E}', '\u{202F}', '\u{1843}'] {
        for text in [format!("ᠠ{separator}{ZWJ}ᠠ"), format!("ᠠ{ZWJ}{separator}ᠠ")] {
            assert!(
                shaper.shape(&text).unwrap().contains(&WrittenUnit::Zwj),
                "{text:?}"
            );
        }
    }
    // A real interior joiner can still be removed inside a suffix; surrounding MVS stays.
    for separator in ['\u{180E}', '\u{202F}'] {
        equivalent(
            &shaper,
            &format!("ᠠ{separator}ᠪᠠ"),
            &format!("ᠠ{separator}ᠪ{ZWJ}ᠠ"),
        );
    }
    assert!(shaper.shape("ᠠ\u{200C}ᠠ").is_err());
    assert!(shaper.normalize("ᠠ\u{200C}ᠠ").is_err());
}

#[test]
fn orphan_fvs_is_not_reattached_when_a_joiner_disappears() {
    let shaper = Shaper::default();
    for fvs in ['\u{180B}', '\u{180C}', '\u{180D}', '\u{180F}'] {
        equivalent(&shaper, "ᠠᠠ", &format!("ᠠ{ZWJ}{fvs}ᠠ"));
        equivalent(&shaper, "ᠠᠠ", &format!("ᠠ{ZWJ}{fvs}{ZWJ}ᠠ"));
    }
    // Deleting at the string level would attach FVS1 to the first a and change its shape.
    assert_ne!(
        shaper.shape("ᠠᠠ").unwrap(),
        shaper.shape("ᠠ\u{180B}ᠠ").unwrap()
    );
}

#[test]
fn explicit_written_unit_controls_keep_their_existing_contract() {
    use WrittenUnit::{Aa, Zwj, I, L};
    let shaper = Shaper::default();
    let requested = [L, I, Zwj, Aa];
    let text = shaper.normalize_written_units(&requested).unwrap();
    assert_eq!(shaper.shape_raw(&text).unwrap(), requested);
    assert!(text.contains(ZWJ));
    assert_eq!(shaper.shape(&text).unwrap(), [L, WrittenUnit::G]);
    assert!(!shaper.normalize(&text).unwrap().contains(ZWJ));
}

#[test]
fn other_locales_keep_interior_joiners() {
    for locale in [Locale::Tod, Locale::Sib, Locale::Mch] {
        assert!(Shaper::new(locale)
            .shape("ᠠ\u{200D}ᠠ")
            .unwrap()
            .contains(&WrittenUnit::Zwj));
    }
}

#[test]
fn every_hudum_letter_pair_and_fvs_combination_is_canonical() {
    let shaper = Shaper::default();
    let mut canonical_by_shape = HashMap::new();
    // 35 * 35 * 5 * 5 = 30,625 independently encoded pairs. This also checks raw
    // contextual-form invariance rather than confusing it with duplicate equivalence.
    for left in '\u{1820}'..='\u{1842}' {
        for right in '\u{1820}'..='\u{1842}' {
            for left_fvs in ["", "\u{180B}", "\u{180C}", "\u{180D}", "\u{180F}"] {
                for right_fvs in ["", "\u{180B}", "\u{180C}", "\u{180D}", "\u{180F}"] {
                    let plain = format!("{left}{left_fvs}{right}{right_fvs}");
                    let joined = format!("{left}{left_fvs}{ZWJ}{right}{right_fvs}");
                    let details: Vec<_> = shaper
                        .shape_detailed(&joined)
                        .unwrap()
                        .into_iter()
                        .filter(|t| t.cp != ZWJ)
                        .collect();
                    assert_eq!(details, shaper.shape_detailed(&plain).unwrap(), "{plain:?}");
                    let canonical = equivalent(&shaper, &plain, &joined);
                    let shape = shaper.shape(&plain).unwrap();
                    if let Some(previous) = canonical_by_shape.insert(shape, canonical.clone()) {
                        assert_eq!(previous, canonical, "independent spellings of {plain:?}");
                    }
                }
            }
        }
    }
}
