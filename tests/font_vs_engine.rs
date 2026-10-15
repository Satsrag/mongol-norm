#![cfg(feature = "font")]
//! The font backend against the rule engine, word for word, on every Hudum word the crate's
//! fixtures hold: the corpora (`core-hud`, `eac-hud`, the inline round-trip cases) and the
//! canonical golden vectors. A second set spells out ~13.5k words around an MVS, a joiner or an
//! FVS, where random strings rarely reach (`boundary_words`). The two backends implement the same rules from two sources —
//! the font at the pinned mongfontbuilder commit, the engine ported from mongfontbuilder
//! 0.10.6 — so every word they shape differently is pinned here with the reason, and a new
//! disagreement (or an old one going away) fails the test.

mod common;

use common::json::Json;
use common::{all_corpus_words, hex, mgl, unit_names};
use std::collections::HashSet;

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

/// Every combination of `parts`, as alias strings: one choice from each part, joined with
/// spaces; an empty choice leaves the part out.
fn combinations(parts: &[&[&str]]) -> Vec<String> {
    let mut words = vec![String::new()];
    for choices in parts {
        let mut next = Vec::with_capacity(words.len() * choices.len());
        for word in &words {
            for choice in *choices {
                let mut joined = word.clone();
                if !choice.is_empty() {
                    if !joined.is_empty() {
                        joined.push(' ');
                    }
                    joined.push_str(choice);
                }
                next.push(joined);
            }
        }
        words = next;
    }
    words
}

const JOINERS: &[&str] = &["", "nirugu", "zwj"];
const FVS: &[&str] = &["", "fvs1", "fvs2", "fvs3", "fvs4"];

/// Words around an MVS, a joiner or an FVS: the contexts where the two backends parted until
/// #50–#52. A random sample rarely builds them, so they are spelled out here.
fn boundary_words() -> Vec<String> {
    let joiners: &[&str] = &["", "nirugu", "zwj", "zwj zwj", "nirugu zwj"];
    let mvs: &[&str] = &["mvs", "nnbsp", "mvs mvs", "nnbsp nnbsp", "mvs nnbsp"];
    let mvs3: &[&str] = &["mvs", "nnbsp", "mvs mvs"];
    let vowels_before_g: &[&str] = &[
        "a", "e", "i", "o", "u", "oe", "ue", "b a", "s", "d a", "n e",
    ];
    let sets = [
        // A consonant before an MVS: onset, devsger, chachlag onset, gender, dotless (#51).
        combinations(&[
            &["a", "e", "o", "a f zwj", "a s", "a d"],
            &["n", "t", "d", "g", "h", "j", "w"],
            joiners,
            mvs,
            &["a", "e", "a l", "u", "i", "n a"],
        ]),
        // A final vowel after a bowed consonant, across an MVS (#51).
        combinations(&[
            &["a b", "b", "a p", "a k", "a k2", "a g", "e g"],
            &["mvs", "nnbsp"],
            joiners,
            &["o", "u", "oe", "ue", "a", "e"],
        ]),
        // III.2a, d.marked, III.2e devsger, sh/g dotless and III.4 across an MVS (#51).
        combinations(&[
            &["ch", "s", "d", "t", "b", "n", "h fvs2", "g fvs4", "h"],
            JOINERS,
            mvs3,
            JOINERS,
            &["o", "u", "oe", "ue", "a", "e", "i", "a l", "o r"],
        ]),
        combinations(&[
            &["a", "e", "o", "i", "ue"],
            JOINERS,
            mvs3,
            JOINERS,
            &[
                "n", "t", "d", "n a", "t r", "d e", "i ue", "i a", "i", "sh i", "g u", "n zwj",
                "t zwj",
            ],
        ]),
        combinations(&[
            &["a sh", "sh", "a s", "a d", "e s"],
            JOINERS,
            mvs3,
            JOINERS,
            &["i", "i n", "a", "g u", "g mvs a", "u"],
        ]),
        // g/h before an MVS and an a/e, with an FVS or a joiner (#50).
        combinations(&[
            vowels_before_g,
            &["g", "h"],
            &["mvs", "nnbsp"],
            &["a", "e"],
            FVS,
        ]),
        combinations(&[
            vowels_before_g,
            &["g", "h"],
            &["mvs", "nnbsp"],
            &["i", "u", "n", "a n", "e r"],
        ]),
        combinations(&[
            vowels_before_g,
            &["g", "h"],
            &["nirugu", "zwj"],
            &["mvs a"],
            &["", "fvs1"],
        ]),
        // A consonant before an MVS and an a/e with an FVS (#50).
        combinations(&[
            &[
                "s g", "a g", "a h", "a n", "a j", "a w", "a y", "a m", "a d",
            ],
            &["mvs"],
            &["a", "e"],
            FVS,
        ]),
        // An FVS on the consonant before the MVS (#52).
        combinations(&[
            &["", "a", "e", "o"],
            &["n", "j", "w", "h", "g"],
            FVS,
            &["mvs", "nnbsp"],
            &["a", "e"],
        ]),
        // GB's i-devsger table after oe/ue, with and without a joiner (#52).
        combinations(&[
            &["", "t", "b"],
            &["oe", "ue", "o", "u", "a", "e"],
            JOINERS,
            FVS,
            &["i", "i n", "i e ue", "i l"],
        ]),
    ];
    let mut words: Vec<String> = sets.into_iter().flatten().collect();
    words.sort();
    words.dedup();
    words
}

/// The boundary words the engine shapes unlike the font on purpose: an FVS that GB writes no
/// rule for, on a final n or g before an MVS (`iii2c_chachlag_onset_at`; EAC XIM11-46).
fn deliberately_different() -> HashSet<String> {
    combinations(&[
        &["a", "e", "o"],
        &["n fvs3", "n fvs4", "g fvs4"],
        &["mvs", "nnbsp"],
        &["a", "e"],
    ])
    .into_iter()
    .collect()
}

#[test]
fn boundary_words_shape_alike_except_the_deliberate_ones() {
    let font = FontShaper::new();
    let engine = Shaper::new(Locale::Mng);
    let words = boundary_words();
    let deliberate = deliberately_different();
    assert!(
        words.len() > 13_000,
        "the boundary set shrank to {} words",
        words.len()
    );
    assert!(deliberate.iter().all(|word| words.contains(word)));

    let mut wrong = Vec::new();
    for aliases in &words {
        let word = mgl(aliases);
        let a = unit_names(&font.shape(&word).unwrap()).join(" ");
        let b = unit_names(&engine.shape(&word).unwrap()).join(" ");
        let expected = deliberate.contains(aliases);
        if (a != b) != expected {
            let note = if expected {
                "expected to differ"
            } else {
                "differ"
            };
            wrong.push(format!("{aliases}: font `{a}`, engine `{b}` ({note})"));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} boundary words:\n{}",
        wrong.len(),
        words.len(),
        wrong.join("\n")
    );
    eprintln!(
        "\n{} boundary words shape alike, {} differ on purpose.",
        words.len() - deliberate.len(),
        deliberate.len()
    );
}
