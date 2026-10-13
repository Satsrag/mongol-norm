#![cfg(feature = "font")]
//! The font backend's own contract: what `FontShaper` accepts, what it reports for the
//! controls and the glyphs HarfBuzz reorders, and that it never panics or leaks a glyph of
//! the composer's own on random input.

mod common;

use common::{hex, unit_names, FontAliases};
use mongol_norm::{Error, FontShaper, Locale, Position, Shaper, WrittenUnit};

fn names(units: &[WrittenUnit]) -> Vec<String> {
    unit_names(units)
}

#[track_caller]
fn shape(locale: Locale, text: &str) -> Vec<String> {
    names(
        &FontShaper::new(locale)
            .shape(text)
            .unwrap_or_else(|e| panic!("shape({text:?}) failed: {e}")),
    )
}

#[track_caller]
fn raw(locale: Locale, text: &str) -> Vec<String> {
    names(
        &FontShaper::new(locale)
            .shape_raw(text)
            .unwrap_or_else(|e| panic!("shape_raw({text:?}) failed: {e}")),
    )
}

#[test]
fn hudum_shapes_like_the_rule_engine_on_the_readme_words() {
    let font = FontShaper::new(Locale::Mng);
    let rules = Shaper::new(Locale::Mng);
    for word in [
        "ᠰᠠᠢᠨ",
        "ᠰᠡᠢᠨ",
        "ᠰᠠᠶ᠋ᠢᠨ",
        "ᠠᠷᠠᠳ",
        "ᠠᠷᠠᠤᠠ",
        "ᠲᠠᠯ᠎ᠠ",
        "ᠪᠠᠭᠰᠢ",
        "ᠮᠣᠩᠭᠣᠯ",
    ] {
        assert_eq!(
            font.shape(word).unwrap(),
            rules.shape(word).unwrap(),
            "{word}"
        );
    }
    assert_eq!(font.shape_str("ᠰᠠᠢᠨ").unwrap(), "S+A+I+I+A");
    assert!(font.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ").unwrap());
    assert!(!font.same_shape("ᠰᠠᠢᠨ", "ᠰᠠᠢ").unwrap());
    assert_eq!(font.locale(), Locale::Mng);
}

#[test]
fn the_locale_selects_the_writing_systems_letterforms() {
    // Final m: Hudum's small tail is `M`, Sibe and Manchu draw the large one, `M3`.
    assert_eq!(shape(Locale::Mng, "ᠠᠮ"), ["A", "A", "M"]);
    assert_eq!(shape(Locale::Sib, "ᠠᠮ"), ["A", "A", "M3"]);
    assert_eq!(shape(Locale::Mch, "ᠠᠮ"), ["A", "A", "M3"]);
    // Todo: a n a h u (core-todo a-3). The public shape unifies the medial `Hx` with `N N`,
    // as the rule engine does for every locale.
    assert_eq!(raw(Locale::Tod, "ᠠᠨᠠᡍᡇ"), ["A", "A", "N", "A", "Hx", "Up"]);
    assert_eq!(
        shape(Locale::Tod, "ᠠᠨᠠᡍᡇ"),
        ["A", "A", "N", "A", "N", "N", "Up"]
    );
}

#[test]
fn non_mongolian_characters_are_rejected_with_their_index() {
    let font = FontShaper::new(Locale::Mng);
    assert_eq!(
        font.shape("ᠰᠠ ᠢᠨ"),
        Err(Error::NonMongolianChar { ch: ' ', index: 2 })
    );
    assert_eq!(
        font.shape("a"),
        Err(Error::NonMongolianChar { ch: 'a', index: 0 })
    );
    // Punctuation of the block, the digits, and ZWNJ are not word characters either.
    for ch in ['\u{1800}', '\u{1810}', '\u{200C}'] {
        assert_eq!(
            font.shape(&ch.to_string()),
            Err(Error::NonMongolianChar { ch, index: 0 })
        );
    }
    assert_eq!(font.shape("").unwrap(), Vec::<WrittenUnit>::new());
}

#[test]
fn characters_the_font_draws_no_written_form_for_are_unsupported() {
    let font = FontShaper::new(Locale::Mng);
    // The unassigned code points of the block shape to nothing in the rule engine; the font
    // would draw its `.notdef` box.
    assert_eq!(
        font.shape("ᠰ\u{181A}"),
        Err(Error::UnsupportedChar {
            ch: '\u{181A}',
            index: 1
        })
    );
    // U+1878 (Buryat CHA) and U+1898 have joined forms but no isolated one: alone, the font
    // falls back to the nominal glyph, which is no written form.
    for ch in ['\u{1878}', '\u{1898}'] {
        assert_eq!(
            font.shape(&ch.to_string()),
            Err(Error::UnsupportedChar { ch, index: 0 })
        );
    }
    assert_eq!(shape(Locale::Mng, "ᠠ\u{1878}"), ["A", "A", "Cx"]);
}

#[test]
fn the_sibe_syllable_boundary_marker_is_a_letter() {
    // core-sibe sbm-1, second word: j u sbm i.
    assert_eq!(shape(Locale::Sib, "ᡪᡠ\u{1807}ᡞ"), ["I", "Oh", "A", "I"]);
    // The rule engine's classification keeps it out of a word (its SIB data has the letter).
    assert!(matches!(
        Shaper::new(Locale::Sib).shape("ᡪᡠ\u{1807}ᡞ"),
        Err(Error::NonMongolianChar { index: 2, .. })
    ));
}

#[test]
fn an_fvs_is_never_a_unit() {
    // Valid: FVS1 selects the isolated `A` of ᠠ.
    assert_eq!(shape(Locale::Mng, "ᠠ᠋"), ["A"]);
    // Invalid: the font draws a placeholder for an FVS4 ᠠ has no variant for; the crate,
    // like the rule engine, reports the letter alone.
    assert_eq!(shape(Locale::Mng, "ᠠ᠏"), ["A", "A"]);
    assert_eq!(shape(Locale::Mng, "ᠠ᠏"), shape(Locale::Mng, "ᠠ"));
    assert_eq!(raw(Locale::Mng, "ᠠ᠏"), ["A", "A"]);
}

#[test]
fn mvs_and_nnbsp_are_one_mvs_each() {
    // A separator that separates nothing is drawn as a wide space plus a silent glyph.
    assert_eq!(raw(Locale::Mng, "ᠠ\u{180E}ᠪ"), ["A", "A", "Mvs", "B"]);
    assert_eq!(raw(Locale::Mng, "\u{180E}"), ["Mvs"]);
    assert_eq!(raw(Locale::Mng, "\u{202F}"), ["Mvs"]);
    assert_eq!(raw(Locale::Mng, "ᠠ\u{202F}ᠠ"), ["A", "A", "Mvs", "Aa"]);
    assert_eq!(
        raw(Locale::Mng, "ᠲᠠᠯ\u{180E}ᠠ"),
        ["T", "A", "L", "Mvs", "Aa"]
    );
    let rules = Shaper::new(Locale::Mng);
    for word in ["ᠠ\u{180E}ᠪ", "\u{180E}", "\u{202F}", "ᠠ\u{202F}ᠠ"] {
        assert_eq!(
            FontShaper::new(Locale::Mng).shape(word).unwrap(),
            rules.shape(word).unwrap(),
            "{}",
            hex(word)
        );
    }
}

#[test]
fn nirugu_is_verbatim() {
    assert_eq!(
        raw(Locale::Mng, "ᠲ\u{180A}\u{180A}\u{180A}ᠠ"),
        ["T", "Nirugu", "Nirugu", "Nirugu", "A"]
    );
    assert_eq!(raw(Locale::Mng, "\u{180A}"), ["Nirugu"]);
    // The stem the font inserts after a bowed letter (`nirugu.extend`) is not a nirugu.
    assert_eq!(raw(Locale::Mng, "ᠻᠳ"), ["K", "Dd"]);
    assert_eq!(shape(Locale::Mng, "ᠻᠳ"), ["K", "O", "A"]);
}

#[test]
fn a_zwj_the_ligature_skipped_is_dropped_from_the_public_shape() {
    let font = FontShaper::new(Locale::Mng);
    // b + ZWJ + o: the font ligates b and o across the joiner (one glyph, `BO`), and
    // HarfBuzz moves the joiner after it.
    let glyphs = font.shape_glyphs("ᠪ\u{200D}ᠣ").unwrap();
    assert_eq!(glyphs.len(), 2);
    assert_eq!(glyphs[0].units, &[WrittenUnit::B, WrittenUnit::O]);
    assert_eq!(glyphs[0].index, 0);
    assert_eq!(glyphs[0].form, Some(Position::Isol));
    assert!(!glyphs[0].skipped);
    assert_eq!(glyphs[1].units, &[WrittenUnit::Zwj]);
    assert_eq!(glyphs[1].index, 1);
    assert!(glyphs[1].skipped);
    assert_eq!(raw(Locale::Mng, "ᠪ\u{200D}ᠣ"), ["B", "O", "Zwj"]);
    assert_eq!(shape(Locale::Mng, "ᠪ\u{200D}ᠣ"), ["B", "O"]);
    // Same for a final `Aa` the edge table would otherwise keep a trailing joiner after.
    assert_eq!(raw(Locale::Mng, "ᠪ\u{200D}ᠠ"), ["B", "Aa", "Zwj"]);
    assert_eq!(shape(Locale::Mng, "ᠪ\u{200D}ᠠ"), ["B", "Aa"]);
    // A joiner after the ligature is not skipped: it is a redundant interior joiner.
    let glyphs = font.shape_glyphs("ᠪᠣ\u{200D}ᠯ").unwrap();
    assert!(glyphs.iter().all(|g| !g.skipped));
    assert_eq!(raw(Locale::Mng, "ᠪᠣ\u{200D}ᠯ"), ["B", "O", "Zwj", "L"]);
    assert_eq!(shape(Locale::Mng, "ᠪᠣ\u{200D}ᠯ"), ["B", "O", "L"]);
    let rules = Shaper::new(Locale::Mng);
    for word in [
        "ᠪ\u{200D}ᠣ",
        "ᠪ\u{200D}ᠠ",
        "ᠪᠣ\u{200D}ᠯ",
        "ᠪ\u{200D}\u{200D}ᠣ",
        "ᠪ᠋\u{200D}ᠣ",
    ] {
        assert_eq!(
            font.shape(word).unwrap(),
            rules.shape(word).unwrap(),
            "{}",
            hex(word)
        );
    }
}

#[test]
fn joiners_at_the_edges_follow_the_shared_redundant_zwj_pass() {
    let font = FontShaper::new(Locale::Mng);
    let rules = Shaper::new(Locale::Mng);
    for word in [
        "\u{200D}",
        "ᠠ\u{200D}",
        "\u{200D}ᠠ",
        "\u{200D}ᠪ",
        "ᠪ\u{200D}",
        "ᠠ\u{200D}ᠶᠢᠨ",
        "ᠠ\u{180E}\u{200D}ᠶᠢᠨ",
        "ᠲᠠᠯ\u{180E}\u{200D}ᠠ",
        "ᠠᠣ\u{200D}\u{180E}ᠠ",
        "ᠯᠢ\u{200D}ᠡ᠋",
    ] {
        assert_eq!(
            font.shape(word).unwrap(),
            rules.shape(word).unwrap(),
            "{}",
            hex(word)
        );
    }
    assert_eq!(shape(Locale::Mng, "\u{200D}"), ["Zwj"]);
    assert_eq!(shape(Locale::Mng, "ᠯᠢ\u{200D}ᠡ᠋"), ["L", "G"]);
}

#[test]
fn the_ali_gali_signs_are_units() {
    // core-hudum-ali-gali pu8: damaru kX a.
    assert_eq!(
        shape(Locale::Mng, "\u{1882}\u{1889}ᠠ"),
        ["Damaru", "G", "Aa"]
    );
    // A lone sign: HarfBuzz draws a dotted circle under it, which is not reported.
    assert_eq!(shape(Locale::Mng, "\u{1886}"), ["Tribaluda"]);
    assert_eq!(shape(Locale::Mng, "\u{1880}"), ["Anusvara"]);
    assert_eq!(shape(Locale::Mng, "\u{1880}᠋"), ["Anusvara"]);
}

#[test]
fn shape_glyphs_reports_the_form_and_the_joining_position() {
    let font = FontShaper::new(Locale::Mng);
    // An isolated n is drawn with the initial form: `N:init` serving `isol`.
    let glyphs = font.shape_glyphs("ᠨ").unwrap();
    assert_eq!(glyphs.len(), 1);
    assert_eq!(glyphs[0].units, &[WrittenUnit::N]);
    assert_eq!(glyphs[0].form, Some(Position::Init));
    assert_eq!(glyphs[0].joining, Some(Position::Isol));
    let glyphs = font.shape_glyphs("ᠰᠠᠢᠨ").unwrap();
    assert_eq!(
        glyphs.iter().map(|g| g.form).collect::<Vec<_>>(),
        [
            Some(Position::Init),
            Some(Position::Medi),
            Some(Position::Medi),
            Some(Position::Fina)
        ]
    );
    assert_eq!(
        glyphs.iter().map(|g| g.index).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    // Controls carry no position and the glyph of a silent FVS no units.
    let glyphs = font.shape_glyphs("ᠠ᠋\u{180E}ᠠ").unwrap();
    assert_eq!(
        glyphs.iter().map(|g| g.units).collect::<Vec<_>>(),
        [
            &[WrittenUnit::A][..],
            &[],
            &[WrittenUnit::Mvs],
            &[WrittenUnit::Aa],
        ]
    );
    assert_eq!(glyphs[2].form, None);
    assert_eq!(glyphs[1].index, 1);
}

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Random strings over every letter and control the suites name, for every locale: nothing
/// panics, every rejection is one of the two documented errors, and no glyph of the
/// composer's own (a building block `_X`, a `marker.*`) reaches the output.
#[test]
fn random_input_never_panics_or_leaks_an_internal_glyph() {
    let order = common::read_fixture("data/../../fonts/unified.glyphs.txt");
    let internal: Vec<bool> = order
        .lines()
        .map(|name| name.starts_with('_') || name.starts_with("marker."))
        .collect();
    assert!(internal.iter().filter(|b| **b).count() > 1000);
    let mut alphabet: Vec<char> = Vec::new();
    for namespace in ["MNG", "MNGx", "TOD", "TODx", "SIB", "MCH", "MCHx"] {
        alphabet.extend(FontAliases::load(namespace).chars());
    }
    alphabet.extend([
        '\u{180A}', '\u{180E}', '\u{202F}', '\u{200D}', '\u{181A}', '\u{1878}',
    ]);
    // The alias tables also name the space and ZWNJ, which are not word characters.
    alphabet.retain(|c| !matches!(*c, ' ' | '\u{200C}'));
    alphabet.sort();
    alphabet.dedup();
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    for locale in Locale::ALL {
        let font = FontShaper::new(locale);
        for _ in 0..4000 {
            let len = rng.below(10);
            let text: String = (0..len)
                .map(|_| alphabet[rng.below(alphabet.len())])
                .collect();
            match font.shape_glyphs(&text) {
                Ok(glyphs) => {
                    for glyph in &glyphs {
                        assert!(
                            !internal[glyph.glyph as usize],
                            "{locale:?} {}: glyph {} is internal",
                            hex(&text),
                            glyph.glyph
                        );
                    }
                    let shape = font.shape(&text).unwrap();
                    assert_eq!(shape, font.shape(&text).unwrap());
                    let _ = font.shape_str(&text).unwrap();
                    let _ = font.shape_raw(&text).unwrap();
                }
                Err(Error::UnsupportedChar { ch, index }) => {
                    // The unassigned code point, or a letter at a position it has no form at.
                    assert!(
                        matches!(ch as u32, 0x181A | 0x1878 | 0x1898),
                        "{}: {ch:?} at {index}",
                        hex(&text)
                    );
                    assert_eq!(text.chars().nth(index), Some(ch));
                }
                Err(error) => panic!("{locale:?} {}: {error}", hex(&text)),
            }
        }
    }
}
