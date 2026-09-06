//! Ported Python test helpers (`python/tests/test_*.py`) shared by the Rust integration tests.
#![allow(dead_code)]

pub mod json;

use std::path::PathBuf;

use mongol_norm::{Shaper, WrittenUnit};

/// Path of a file under the crate's `tests/` directory — the corpus and golden fixtures live
/// there once and are shared with the Python suite (`python/tests`).
pub fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(relative)
}

pub fn read_fixture(relative: &str) -> String {
    let path = fixture_path(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The complete alias → code point map used by every Python test module.
pub const ALIAS_TO_CP: &[(&str, u32)] = &[
    ("a", 0x1820),
    ("e", 0x1821),
    ("i", 0x1822),
    ("o", 0x1823),
    ("u", 0x1824),
    ("oe", 0x1825),
    ("ue", 0x1826),
    ("ee", 0x1827),
    ("n", 0x1828),
    ("ng", 0x1829),
    ("b", 0x182A),
    ("p", 0x182B),
    ("h", 0x182C),
    ("g", 0x182D),
    ("m", 0x182E),
    ("l", 0x182F),
    ("s", 0x1830),
    ("sh", 0x1831),
    ("t", 0x1832),
    ("d", 0x1833),
    ("ch", 0x1834),
    ("j", 0x1835),
    ("y", 0x1836),
    ("r", 0x1837),
    ("w", 0x1838),
    ("f", 0x1839),
    ("k2", 0x183A),
    ("k", 0x183B),
    ("c", 0x183C),
    ("z", 0x183D),
    ("hh", 0x183E),
    ("rh", 0x183F),
    ("lh", 0x1840),
    ("zr", 0x1841),
    ("cr", 0x1842),
    ("mvs", 0x180E),
    ("fvs1", 0x180B),
    ("fvs2", 0x180C),
    ("fvs3", 0x180D),
    ("fvs4", 0x180F),
    ("nnbsp", 0x202F),
    ("nirugu", 0x180A),
    ("zwj", 0x200D),
];

pub fn is_known_alias(alias: &str) -> bool {
    ALIAS_TO_CP.iter().any(|(name, _)| *name == alias)
}

pub fn alias_char(alias: &str) -> char {
    ALIAS_TO_CP
        .iter()
        .find(|(name, _)| *name == alias)
        .map(|(_, cp)| char::from_u32(*cp).unwrap())
        .unwrap_or_else(|| panic!("unknown aliases: {alias}"))
}

/// Python `_mgl`: `"s a i n"` → `"ᠰᠠᠢᠨ"`.
pub fn mgl(aliases: &str) -> String {
    aliases.split_whitespace().map(alias_char).collect()
}

/// Python `_aliases_to_words`: split on the `space` word boundary.
pub fn aliases_to_words(aliases: &str) -> Vec<String> {
    let mut words = vec![String::new()];
    for token in aliases.split_whitespace() {
        if token == "space" {
            words.push(String::new());
        } else {
            words.last_mut().unwrap().push(alias_char(token));
        }
    }
    words
}

/// Python `_shape_aliases`: shape each word independently, joined with `Mvs`.
pub fn shape_aliases(shaper: &Shaper, aliases: &str) -> Vec<WrittenUnit> {
    let mut out = Vec::new();
    for (i, word) in aliases_to_words(aliases).iter().enumerate() {
        if i > 0 {
            out.push(WrittenUnit::Mvs);
        }
        if word.is_empty() {
            continue;
        }
        out.extend(
            shaper
                .shape(word)
                .unwrap_or_else(|e| panic!("shape({aliases:?}) failed: {e}")),
        );
    }
    out
}

/// mongfontbuilder glyph-name artifacts that our shaper never emits (dropped from expectations).
pub const FONT_NAMING_ARTIFACTS: &[&str] = &["<", ">", "Fvs1", "Fvs2", "Fvs3", "Fvs4", "Nnbsp"];

/// Python `_normalize_expected` (eac variant, a superset of the core one): `_`/`-`/`Mvs` → `Mvs`,
/// `Ni` → `Nirugu`, artifacts dropped.
pub fn normalize_expected(expected: &str) -> Vec<String> {
    expected
        .split_whitespace()
        .filter_map(|token| match token {
            "_" | "-" | "Mvs" => Some("Mvs".to_owned()),
            "Ni" => Some("Nirugu".to_owned()),
            other if FONT_NAMING_ARTIFACTS.contains(&other) => None,
            other => Some(other.to_owned()),
        })
        .collect()
}

pub fn unit_names(units: &[WrittenUnit]) -> Vec<String> {
    units.iter().map(|unit| unit.as_str().to_owned()).collect()
}

/// Python `_load_tsv`: `(id, aliases, expected)` rows; comments (`#`) and short rows skipped.
pub fn load_tsv(relative: &str) -> Vec<(String, String, String)> {
    read_fixture(relative)
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let cols: Vec<&str> = line.split('\t').collect();
            (cols.len() >= 3).then(|| (cols[0].to_owned(), cols[1].to_owned(), cols[2].to_owned()))
        })
        .collect()
}

pub fn hex(text: &str) -> String {
    text.chars()
        .map(|c| format!("{:04X}", c as u32))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The code points of every non-empty word of a `space`-separated alias row, joined with ` | `
/// (used in corpus-test failure output).
pub fn row_hex(aliases: &str) -> String {
    aliases_to_words(aliases)
        .iter()
        .filter(|word| !word.is_empty())
        .map(|word| hex(word))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Python `_split_letters`: per-letter chunks, each letter with its trailing FVS marks.
pub fn split_letters(text: &str) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    for c in text.chars() {
        if matches!(c as u32, 0x180B | 0x180C | 0x180D | 0x180F) && !chunks.is_empty() {
            chunks.last_mut().unwrap().push(c);
        } else {
            chunks.push(c.to_string());
        }
    }
    chunks
}

/// `python/tests/test_round_trip.py::INLINE_CASES`.
pub const INLINE_CASES: &[(&str, &str)] = &[
    ("sain.base", "s a i n"),
    ("sain.e_variant", "s e i n"),
    ("sain.na_fvs2", "s n fvs2 i i n"),
    ("sain.ya_fvs1_i", "s a y fvs1 i n"),
    ("sain.ya_fvs1_ya_fvs1", "s a y fvs1 y fvs1 n"),
    ("bayan_ondor", "b a y a n fvs2 ue fvs2 n d ue r"),
    ("utn.g_fvs2_blocks", "b a g fvs2 mvs a"),
    ("utn.g_fvs3_chachlag", "b a g fvs3 mvs a"),
    ("utn.nnbsp_alone", "nnbsp"),
    ("utn.nnbsp_chachlag", "b a g nnbsp a"),
    ("utn.nnbsp_particle", "a b u nnbsp y i n"),
    ("utn.nnbsp_mvs_token", "a b u nnbsp e j i"),
    ("chachlag.tala", "t a l mvs a"),
    ("particle.talayin", "t a l mvs a mvs y i n"),
    ("plain.morin", "m o r i n"),
    ("plain.nom", "n o m"),
    ("plain.tngri", "t ng r i"),
    ("paticle.i", "mvs i"),
    ("paticle.i.iso", "i fvs1"),
];

/// `python/tests/test_round_trip.py::PARTICLE_CASES`.
pub const PARTICLE_CASES: &[(&str, &str)] = &[
    ("mvs+i", "mvs i"),
    ("mvs+u (masc)", "mvs u"),
    ("mvs+ue (fem)", "mvs ue"),
    ("mvs+a chachlag", "mvs a"),
    ("mvs+e chachlag", "mvs e"),
    ("mvs+yin (genitive)", "mvs y i n"),
    ("mvs+un (u variant)", "mvs u n"),
    ("mvs+un (ue variant)", "mvs ue n"),
    ("mvs+du (u dative)", "mvs d u"),
    ("mvs+du (ue dative)", "mvs d ue"),
    ("mvs+tu (t-dative)", "mvs t u"),
    ("mvs+ban (reflexive)", "mvs b a n"),
    ("mvs+ben (reflexive)", "mvs b e n"),
    ("mvs+daga (locative)", "mvs d a g a"),
    ("mvs+yi (acc)", "mvs y i"),
    ("mvs+iyer (instr)", "mvs i y e r"),
    ("I iso (j alone)", "j"),
    ("I iso (i+fvs1)", "i fvs1"),
];

/// `python/tests/test_round_trip.py::PARTICLE_EQUIVALENCE_GROUPS`.
pub const PARTICLE_EQUIVALENCE_GROUPS: &[(&str, &[&str])] = &[
    ("mvs+u vs mvs+ue", &["mvs u", "mvs ue"]),
    ("mvs+a vs mvs+e (chachlag)", &["mvs a", "mvs e"]),
    ("mvs+un (u/ue)", &["mvs u n", "mvs ue n"]),
    ("mvs+du (u/ue)", &["mvs d u", "mvs d ue"]),
    ("mvs+ban vs mvs+ben", &["mvs b a n", "mvs b e n"]),
    ("I iso (j vs i+fvs1)", &["j", "i fvs1"]),
];

/// The EAC cases where the GB spec and the UTN model intentionally disagree
/// (`python/tests/test_eac_hud.py::_UTN_XFAIL_CASES`, the same set mongfontbuilder marks `xfail`).
pub const UTN_XFAIL_CASES: &[&str] =
    &["XIM11-38", "XIM11-39", "XIM11-40", "XIM11-41", "XIM11-1012"];

/// Every non-empty word of a loaded corpus TSV. Panics on an unknown alias instead of silently
/// shrinking coverage; `file` only names the offending corpus in that message.
pub fn corpus_words_from_rows(file: &str, rows: &[(String, String, String)]) -> Vec<String> {
    let mut words = Vec::new();
    for (index, aliases, _) in rows {
        let unknown: Vec<&str> = aliases
            .split_whitespace()
            .filter(|t| *t != "space" && !is_known_alias(t))
            .collect();
        assert!(
            unknown.is_empty(),
            "{file}:{index} unknown aliases: {}",
            unknown.join(", ")
        );
        words.extend(
            aliases_to_words(aliases)
                .into_iter()
                .filter(|w| !w.is_empty()),
        );
    }
    words
}

/// `python/tests/test_canonical_golden.py::_all_corpus_words`: every non-empty word of the inline cases
/// and both TSV corpora.
pub fn all_corpus_words() -> Vec<String> {
    let mut words = Vec::new();
    for (_, aliases) in INLINE_CASES {
        words.extend(
            aliases_to_words(aliases)
                .into_iter()
                .filter(|w| !w.is_empty()),
        );
    }
    for file in ["data/core-hud.tsv", "data/eac-hud.tsv"] {
        words.extend(corpus_words_from_rows(file, &load_tsv(file)));
    }
    words
}
