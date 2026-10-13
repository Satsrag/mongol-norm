#![cfg(feature = "font")]
//! The font backend against the rule engine, word for word, on every Hudum word the crate's
//! fixtures hold: the corpora (`core-hud`, `eac-hud`, the inline round-trip cases) and the
//! canonical golden vectors. The two backends implement the same rules from two sources —
//! the font at the pinned mongfontbuilder commit, the engine ported from mongfontbuilder
//! 0.10.6 — so every word they shape differently is pinned here with the reason, and a new
//! disagreement (or an old one going away) fails the test.

mod common;

use common::json::Json;
use common::{all_corpus_words, hex, unit_names};
use mongol_norm::{FontShaper, Locale, Shaper};

/// The fixture words the two backends shape differently: (code points, font, engine, why).
///
/// All fifteen are one disagreement: an FVS2 or FVS3 after `n`, which the Hudum standard
/// declares invalid (so the engine, like the EAC suite, leaves the letter in its default
/// form) but which the unified font honours, because the same character takes those
/// selectors in Sibe and Manchu and the font judges an FVS by the character, not the writing
/// system — mongfontbuilder's `FVS_LOCALE` reason (`tests/fixtures.py`), the same rows it
/// marks `xfail` for the unified font (see `tests/font_suites.rs`).
const KNOWN: &[(&str, &str, &str, &str)] = &[
    ("180A 1828 180D", "Nirugu Nx", "Nirugu A", "n fvs3: MNM10-2"),
    (
        "180E 1822 1836 1820 1828 180D",
        "Mvs A I Y A Nx",
        "Mvs A I Y A A",
        "n fvs3: XIM11-678",
    ),
    (
        "180E 1822 1836 1821 1828 180D",
        "Mvs A I Y A Nx",
        "Mvs A I Y A A",
        "n fvs3: XIM11-681",
    ),
    (
        "180E 1824 1828 180D",
        "Mvs A O Nx",
        "Mvs A O A",
        "n fvs3: XIM11-684",
    ),
    (
        "180E 1826 1828 180D",
        "Mvs A O I Nx",
        "Mvs A O I A",
        "n fvs3: XIM11-687",
    ),
    (
        "180E 1833 1820 182D 1820 1828 180D",
        "Mvs T A N N A Nx",
        "Mvs T A N N A A",
        "n fvs3: XIM11-690",
    ),
    (
        "180E 1833 1821 182D 180C 1821 1828 180D",
        "Mvs T A G A Nx",
        "Mvs T A G A A",
        "n fvs3: XIM11-694",
    ),
    (
        "180E 1836 1822 1828 180D",
        "Mvs Y I Nx",
        "Mvs Y I A",
        "n fvs3: XIM11-675",
    ),
    (
        "1820 1828 180D 1833 1820",
        "A A Nx D A",
        "A A A D A",
        "n fvs3: MNZ11-3",
    ),
    (
        "1821 1828 180D 180E 1821",
        "A Nx Mvs Aa",
        "A A Mvs Aa",
        "n fvs3: XIM11-46",
    ),
    (
        "1826 1828 180D 1826",
        "A O I Nx U",
        "A O I N U",
        "n fvs3: MNZ21-5",
    ),
    ("1828 180C", "N2", "N", "n fvs2: MND11-2"),
    (
        "1828 180C 1820 1837 1828",
        "N2 A R A",
        "N A R A",
        "n fvs2: XIM11-11",
    ),
    (
        "1828 180C 1821 182C 1821 1822",
        "N2 A G A I",
        "N A G A I",
        "n fvs2: MNS11-26",
    ),
    ("1832 1820 1828 180D", "T A Nx", "T A A", "n fvs3: MNM11-2"),
];

fn golden_words() -> Vec<String> {
    common::read_fixture("golden/mng-canonical-v1.jsonl")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(Json::parse)
        .filter(|record| record.index("type").as_str() == "vector")
        .map(|record| {
            record
                .index("input_cps")
                .as_array()
                .iter()
                .map(|cp| char::from_u32(cp.as_u64() as u32).unwrap())
                .collect()
        })
        .collect()
}

#[test]
fn fixture_words_shape_alike_except_the_known_ones() {
    let font = FontShaper::new(Locale::Mng);
    let engine = Shaper::new(Locale::Mng);
    let mut words = all_corpus_words();
    words.extend(golden_words());
    words.sort();
    words.dedup();
    assert!(
        words.len() > 3000,
        "the fixtures shrank to {} words",
        words.len()
    );

    let mut found = Vec::new();
    for word in &words {
        let a = unit_names(&font.shape(word).unwrap()).join(" ");
        let b = match engine.shape(word) {
            Ok(units) => unit_names(&units).join(" "),
            Err(error) => format!("error: {error}"),
        };
        if a != b {
            found.push((hex(word), a, b));
        }
    }
    let known: Vec<(String, String, String)> = KNOWN
        .iter()
        .map(|(cps, f, e, _)| (cps.to_string(), f.to_string(), e.to_string()))
        .collect();
    if found != known {
        eprintln!("{} fixture words shape differently:", found.len());
        for (cps, f, e) in &found {
            eprintln!("    (\"{cps}\", \"{f}\", \"{e}\", \"\"),");
        }
        panic!("the font/engine disagreements are not the pinned ones");
    }
    eprintln!(
        "\n{} fixture words shape alike, {} known disagreements.",
        words.len() - found.len(),
        found.len()
    );
}
