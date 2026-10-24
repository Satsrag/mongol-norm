#![cfg(feature = "font")]
//! The font backend's own contract: what `FontShaper` accepts, what it reports for the
//! controls and the glyphs HarfBuzz reorders, and that it never panics or leaks a glyph of
//! the composer's own on random input.

mod common;

use common::{hex, unit_names, ALIAS_TO_CP};
use mongol_norm::{Error, FontShaper, Locale, Position, Shaper, WrittenUnit};

#[track_caller]
fn shape(text: &str) -> Vec<String> {
    unit_names(
        &FontShaper::new()
            .shape(text)
            .unwrap_or_else(|e| panic!("shape({text:?}) failed: {e}")),
    )
}

#[track_caller]
fn raw(text: &str) -> Vec<String> {
    unit_names(
        &FontShaper::new()
            .shape_raw(text)
            .unwrap_or_else(|e| panic!("shape_raw({text:?}) failed: {e}")),
    )
}

/// The font and the rule engine shape `words` alike.
#[track_caller]
fn assert_like_the_engine(words: &[&str]) {
    let font = FontShaper::new();
    let rules = Shaper::new(Locale::Mng);
    for word in words {
        assert_eq!(
            font.shape(word).unwrap(),
            rules.shape(word).unwrap(),
            "{}",
            hex(word)
        );
    }
}

#[test]
fn shapes_like_the_rule_engine_on_the_readme_words() {
    assert_like_the_engine(&[
        "ᠰᠠᠢᠨ",
        "ᠰᠡᠢᠨ",
        "ᠰᠠᠶ᠋ᠢᠨ",
        "ᠠᠷᠠᠳ",
        "ᠠᠷᠠᠤᠠ",
        "ᠲᠠᠯ᠎ᠠ",
        "ᠪᠠᠭᠰᠢ",
        "ᠮᠣᠩᠭᠣᠯ",
    ]);
    let font = FontShaper::default();
    assert_eq!(font.shape_str("ᠰᠠᠢᠨ").unwrap(), "S+A+I+I+A");
    assert!(font.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ").unwrap());
    assert!(!font.same_shape("ᠰᠠᠢᠨ", "ᠰᠠᠢ").unwrap());
}

#[test]
fn non_mongolian_characters_are_rejected_with_their_index() {
    let font = FontShaper::new();
    assert_eq!(
        font.shape("ᠰᠠ ᠢᠨ"),
        Err(Error::NonMongolianChar { ch: ' ', index: 2 })
    );
    assert_eq!(
        font.shape("a"),
        Err(Error::NonMongolianChar { ch: 'a', index: 0 })
    );
    // Punctuation of the block (the Sibe syllable boundary marker included) and the digits are
    // not word characters — the same classification as the rule engine.
    let rules = Shaper::new(Locale::Mng);
    for ch in ['\u{1800}', '\u{1807}', '\u{1810}'] {
        let text = ch.to_string();
        assert_eq!(
            font.shape(&text),
            Err(Error::NonMongolianChar { ch, index: 0 })
        );
        assert_eq!(font.shape(&text), rules.shape(&text));
    }
    assert_eq!(font.shape("").unwrap(), Vec::<WrittenUnit>::new());
}

#[test]
fn characters_of_other_writing_systems_are_unsupported() {
    let font = FontShaper::new();
    // Letters of the block the Hudum font has no glyph for: an unassigned code point, the
    // Todo long vowel sign, Todo e, Sibe e, Manchu i, the Ali Gali letter a and the Ali Gali
    // signs (anusvara, baluda, dagalga). The rule engine shapes them to nothing; the font
    // would draw its `.notdef` box.
    for ch in [
        '\u{181A}', '\u{1843}', '\u{1844}', '\u{185D}', '\u{1873}', '\u{1887}', '\u{1880}',
        '\u{1885}', '\u{18A9}',
    ] {
        assert_eq!(
            font.shape(&format!("ᠰ{ch}ᠠ")),
            Err(Error::UnsupportedChar { ch, index: 1 }),
            "U+{:04X}",
            ch as u32
        );
    }
}

#[test]
fn an_fvs_is_never_a_unit() {
    // Valid: FVS1 selects the isolated `A` of ᠠ.
    assert_eq!(shape("ᠠ᠋"), ["A"]);
    // Invalid: the font draws a placeholder for an FVS4 ᠠ has no variant for; the crate,
    // like the rule engine, reports the letter alone.
    assert_eq!(shape("ᠠ᠏"), ["A", "A"]);
    assert_eq!(shape("ᠠ᠏"), shape("ᠠ"));
    assert_eq!(raw("ᠠ᠏"), ["A", "A"]);
}

#[test]
fn mvs_and_nnbsp_are_one_mvs_each() {
    // A separator that separates nothing is drawn as a wide space plus a silent glyph.
    assert_eq!(raw("ᠠ\u{180E}ᠪ"), ["A", "A", "Mvs", "B"]);
    assert_eq!(raw("\u{180E}"), ["Mvs"]);
    assert_eq!(raw("\u{202F}"), ["Mvs"]);
    assert_eq!(raw("ᠠ\u{202F}ᠠ"), ["A", "A", "Mvs", "Aa"]);
    assert_eq!(raw("ᠲᠠᠯ\u{180E}ᠠ"), ["T", "A", "L", "Mvs", "Aa"]);
    assert_like_the_engine(&["ᠠ\u{180E}ᠪ", "\u{180E}", "\u{202F}", "ᠠ\u{202F}ᠠ"]);
}

#[test]
fn nirugu_is_verbatim() {
    assert_eq!(
        raw("ᠲ\u{180A}\u{180A}\u{180A}ᠠ"),
        ["T", "Nirugu", "Nirugu", "Nirugu", "A"]
    );
    assert_eq!(raw("\u{180A}"), ["Nirugu"]);
    assert_eq!(raw("ᠻᠳ"), ["K", "Dd"]);
    assert_eq!(shape("ᠻᠳ"), ["K", "O", "A"]);
}

#[test]
fn a_zwj_the_ligature_skipped_is_dropped_from_the_public_shape() {
    let font = FontShaper::new();
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
    assert_eq!(raw("ᠪ\u{200D}ᠣ"), ["B", "O", "Zwj"]);
    assert_eq!(shape("ᠪ\u{200D}ᠣ"), ["B", "O"]);
    // Same for a final `Aa` the edge table would otherwise keep a trailing joiner after.
    assert_eq!(raw("ᠪ\u{200D}ᠠ"), ["B", "Aa", "Zwj"]);
    assert_eq!(shape("ᠪ\u{200D}ᠠ"), ["B", "Aa"]);
    // A joiner after the ligature is not skipped: it is a redundant interior joiner.
    let glyphs = font.shape_glyphs("ᠪᠣ\u{200D}ᠯ").unwrap();
    assert!(glyphs.iter().all(|g| !g.skipped));
    assert_eq!(raw("ᠪᠣ\u{200D}ᠯ"), ["B", "O", "Zwj", "L"]);
    assert_eq!(shape("ᠪᠣ\u{200D}ᠯ"), ["B", "O", "L"]);
    assert_like_the_engine(&[
        "ᠪ\u{200D}ᠣ",
        "ᠪ\u{200D}ᠠ",
        "ᠪᠣ\u{200D}ᠯ",
        "ᠪ\u{200D}\u{200D}ᠣ",
        "ᠪ᠋\u{200D}ᠣ",
    ]);
}

#[test]
fn joiners_at_the_edges_follow_the_shared_redundant_zwj_pass() {
    assert_like_the_engine(&[
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
    ]);
    assert_eq!(shape("\u{200D}"), ["Zwj"]);
    assert_eq!(shape("ᠯᠢ\u{200D}ᠡ᠋"), ["L", "G"]);
}

#[test]
fn a_zwnj_is_a_unit() {
    // t a l + ZWNJ + a: the break makes the l final and the a isolated, and the font reports it.
    assert_eq!(raw("ᠲᠠᠯ\u{200C}ᠠ"), ["T", "A", "L", "Zwnj", "A", "A"]);
    assert_eq!(shape("ᠲᠠᠯ\u{200C}ᠠ"), ["T", "A", "L", "Zwnj", "A", "A"]);
    // At a word edge too, as in the rule engine.
    assert_eq!(raw("\u{200C}ᠪᠠ"), ["Zwnj", "B", "Aa"]);
    assert_eq!(shape("\u{200C}ᠪᠠ"), ["Zwnj", "B", "Aa"]);
    assert_eq!(shape("\u{200C}"), ["Zwnj"]);
    assert_like_the_engine(&[
        "\u{200C}",
        "\u{200C}\u{200C}",
        "ᠪᠠ\u{200C}",
        "ᠲᠠᠯ\u{200C}ᠠ",
        "ᠨᠠ\u{200C}ᠪ",
        "ᠠ\u{200C}\u{180E}ᠠ",
        "ᠠ\u{180E}\u{200C}ᠠ",
        "ᠯᠠᠯᠠ\u{200D}\u{200C}ᠫᠦᠳ",
        "ᠲᠠᠩ\u{200D}\u{200C}ᠫᠤᠷᠤᠮ",
        "ᠰᠠ\u{200C}ᠢᠨ",
    ]);
}

#[test]
fn shape_glyphs_reports_the_form_and_the_joining_position() {
    let font = FontShaper::new();
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
    // Controls carry no position and the glyph of an FVS no units.
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

/// Random strings over every Hudum letter and control, plus a few characters the font has no
/// glyph for: nothing panics, every rejection is `UnsupportedChar` for one of those, and no
/// glyph of the composer's own (a building block `_X`, a `marker.*`) reaches the output.
#[test]
fn random_input_never_panics_or_leaks_an_internal_glyph() {
    let order = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fonts/hudum.glyphs.txt"
    ))
    .unwrap();
    let internal: Vec<bool> = order
        .lines()
        .map(|name| name.starts_with('_') || name.starts_with("marker."))
        .collect();
    assert!(internal.iter().filter(|b| **b).count() > 200);
    const FOREIGN: [char; 4] = ['\u{181A}', '\u{1843}', '\u{185D}', '\u{1880}'];
    let mut alphabet: Vec<char> = ALIAS_TO_CP
        .iter()
        .map(|(_, cp)| char::from_u32(*cp).unwrap())
        .collect();
    alphabet.extend(FOREIGN);
    let font = FontShaper::new();
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    for _ in 0..5_000 {
        let len = rng.below(10);
        let text: String = (0..len)
            .map(|_| alphabet[rng.below(alphabet.len())])
            .collect();
        match font.shape_glyphs(&text) {
            Ok(glyphs) => {
                for glyph in &glyphs {
                    assert!(
                        !internal[glyph.glyph as usize],
                        "{}: glyph {} is internal",
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
                assert!(FOREIGN.contains(&ch), "{}: {ch:?} at {index}", hex(&text));
                assert_eq!(text.chars().nth(index), Some(ch));
            }
            Err(error) => panic!("{}: {error}", hex(&text)),
        }
    }
}
