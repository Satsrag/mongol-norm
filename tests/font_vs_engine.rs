#![cfg(feature = "font")]
//! The font backend against the rule engine, word for word, on every Hudum word the crate's
//! fixtures hold: the corpora (`core-hud`, `eac-hud`, the inline round-trip cases) and the
//! canonical golden vectors. A second set spells out ~15k words around an MVS, a joiner or an
//! FVS, where random strings rarely reach (`boundary_words`). The two backends implement the same rules from two sources —
//! the font at the pinned mongfontbuilder commit, the engine ported from mongfontbuilder
//! 0.10.6 — so every word they shape differently is pinned here with the reason, and a new
//! disagreement (or an old one going away) fails the test.

mod common;

use common::json::Json;
use common::{all_corpus_words, hex, mgl, unit_names};
use mongol_norm::{FontShaper, Locale, Shaper};

/// The fixture words the two backends shape differently: (code points, font, engine, why).
///
/// None. The engine follows the font on every fixture word, XIM11-46 included. The EAC suite
/// expects otherwise there and skips it (`UTN_XFAIL_CASES`). A word added here needs its reason.
const KNOWN: &[(&str, &str, &str, &str)] = &[];

/// Edge cases where the engine follows the font on purpose although GB, its EAC suite or UTN #57
/// read otherwise (rare input, not reported upstream). They must keep shaping alike: when a font bump
/// changes one, revisit the rule its note names. That rule's doc says what to change.
const WATCHED: &[(&str, &str)] = &[
    (
        "e n fvs3 mvs e",
        "iii2c_chachlag_onset_at: an undefined FVS on n keeps the onset (EAC XIM11-46)",
    ),
    (
        "a g fvs4 mvs a",
        "iii2c_chachlag_onset_at: an undefined FVS on g keeps the onset",
    ),
    (
        "d fvs3 fvs2 oe",
        "iii2a_d_marked_at: two FVS keep the marked d",
    ),
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
        // An FVS on a joiner, two FVS, and an FVS on the MVS (#55).
        combinations(&[
            &["", "a mvs"],
            &["d"],
            &["nirugu fvs1", "nirugu fvs4", "zwj fvs2", "zwj fvs3"],
            &["a", "e", "o", "u", "oe", "ue", "i"],
        ]),
        combinations(&[
            &["g", "h"],
            &["", "fvs2", "fvs4", "fvs2 fvs4", "fvs4 fvs1", "fvs1 fvs2"],
            &["", "zwj fvs4", "nirugu fvs2", "zwj fvs1"],
            &["oe", "ue"],
        ]),
        combinations(&[
            &["", "a mvs", "ch mvs"],
            &["u", "ue"],
            &["zwj fvs4", "nirugu fvs2", "nirugu fvs1"],
            &["u", "ue"],
        ]),
        combinations(&[
            &["", "t", "b", "a"],
            &["oe", "ue", "o", "u", "a", "e"],
            &[
                "fvs2 fvs1",
                "fvs1 fvs3",
                "fvs1 fvs4",
                "fvs1 fvs2",
                "fvs3 fvs3",
            ],
            &["", "nirugu", "zwj fvs1"],
            &["i n", "i l"],
        ]),
        combinations(&[
            &["a", "e"],
            &["n", "j", "w", "g", "h"],
            &["mvs", "nnbsp"],
            &["fvs1", "fvs3"],
            &["a", "e"],
            &["", "fvs1", "fvs2", "fvs4"],
        ]),
        // Two FVS between an initial d and its final vowel keep the marked d (#55).
        combinations(&[
            &["", "a mvs"],
            &["d"],
            &[
                "fvs3 fvs2",
                "fvs4 fvs1",
                "fvs3 nirugu fvs1",
                "nirugu fvs1 zwj fvs2",
            ],
            &["a", "e", "o", "u", "oe", "ue"],
        ]),
    ];
    let mut words: Vec<String> = sets.into_iter().flatten().collect();
    words.sort();
    words.dedup();
    words
}

#[test]
fn boundary_words_shape_alike() {
    let font = FontShaper::new();
    let engine = Shaper::new(Locale::Mng);
    let words = boundary_words();
    assert!(
        words.len() > 13_000,
        "the boundary set shrank to {} words",
        words.len()
    );

    let mut wrong = Vec::new();
    for aliases in &words {
        let word = mgl(aliases);
        let a = unit_names(&font.shape(&word).unwrap()).join(" ");
        let b = unit_names(&engine.shape(&word).unwrap()).join(" ");
        if a != b {
            wrong.push(format!("{aliases}: font `{a}`, engine `{b}`"));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} boundary words differ:\n{}",
        wrong.len(),
        words.len(),
        wrong.join("\n")
    );
    eprintln!("\n{} boundary words shape alike.", words.len());
}
