#![cfg(feature = "font")]
//! The font backend against the rule engine, word for word, on every Hudum word the crate's
//! fixtures hold: the corpora (`core-hud`, `eac-hud`, the inline round-trip cases) and the
//! canonical golden vectors. The two backends implement the same rules from two sources —
//! the font at the pinned mongfontbuilder commit, the engine ported from mongfontbuilder
//! 0.10.6 — so every word they shape differently is pinned here with the reason, and a new
//! disagreement (or an old one going away) fails the test.

mod common;

use common::json::Json;
use common::{all_corpus_words, hex, mgl, unit_names};
use mongol_norm::{FontShaper, Locale, Shaper};

/// The fixture words the two backends shape differently: (code points, font, engine, why).
///
/// One: `e n fvs3 mvs e` (EAC `XIM11-46`). The font draws the form the FVS3 after `n`
/// selects (`N`); the EAC suite reads that FVS3 as invalid and expects the default `A`, which
/// is what the engine shapes. mongfontbuilder marks the row `xfail` for every font it builds.
const KNOWN: &[(&str, &str, &str, &str)] = &[(
    "1821 1828 180D 180E 1821",
    "A N Mvs Aa",
    "A A Mvs Aa",
    "n fvs3 before an MVS: XIM11-46",
)];

/// Edge cases where the engine follows the font on purpose although GB or UTN #57 read
/// otherwise (rare input, not reported upstream). They must keep shaping alike: when a font bump
/// changes one, revisit the rule its note names. That rule's doc says what to change.
const WATCHED: &[(&str, &str)] = &[
    (
        "ch mvs a fvs4",
        "iii1_chachlag_at: an invalid FVS on the vowel keeps `Aa`",
    ),
    (
        "t mvs e fvs3",
        "iii1_chachlag_at: an invalid FVS on the vowel keeps `Aa`",
    ),
    (
        "a n mvs a fvs1",
        "iii2c_chachlag_onset_at: an FVS on the vowel drops the onset",
    ),
    (
        "s g mvs a fvs1",
        "iii1_chachlag_at: g.dotless rule (2) sees a chachlag vowel",
    ),
    (
        "a g mvs a fvs1",
        "precedes_mvs_and_isolated_a_e: g takes no gender",
    ),
    (
        "a g nirugu mvs a",
        "precedes_mvs_and_isolated_a_e: g takes no gender",
    ),
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
    let font = FontShaper::new();
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

#[test]
fn watched_edge_cases_shape_alike() {
    let font = FontShaper::new();
    let engine = Shaper::new(Locale::Mng);
    let mut drifted = Vec::new();
    for &(aliases, note) in WATCHED {
        let word = mgl(aliases);
        let a = unit_names(&font.shape(&word).unwrap()).join(" ");
        let b = unit_names(&engine.shape(&word).unwrap()).join(" ");
        if a != b {
            drifted.push(format!("{aliases}: font `{a}`, engine `{b}` ({note})"));
        }
    }
    assert!(
        drifted.is_empty(),
        "the font changed on a watched edge case:\n{}",
        drifted.join("\n")
    );
}
