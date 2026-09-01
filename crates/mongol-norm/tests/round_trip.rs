//! Round-trip, shape-canonicity and particle-uniformity properties of normalize — port of
//! `tests/test_round_trip.py`.
//!
//! The defining property of a correct normalize is that it preserves shape:
//! `shape(input) == shape(normalize(input))`. The exact Unicode form does not matter here.

mod common;

use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

use common::json::Json;
use common::{
    aliases_to_words, all_corpus_words, hex, is_known_alias, load_tsv, mgl, unit_names,
    INLINE_CASES, PARTICLE_CASES, PARTICLE_EQUIVALENCE_GROUPS,
};
use mongol_norm::{Locale, Shaper, WrittenUnit};

const MVS_CHAR: char = '\u{180E}';

fn shaper() -> Shaper {
    Shaper::new(Locale::Mng)
}

/// Python `_check_word`: push a failure description if `word` does not round-trip.
fn check_word(shaper: &Shaper, label: &str, word: &str, failures: &mut Vec<String>) {
    let s1 = match shaper.shape(word) {
        Ok(s) => s,
        Err(e) => return failures.push(format!("{label:30}  CRASH  {e}")),
    };
    let norm = match shaper.normalize(word) {
        Ok(n) => n,
        Err(e) => return failures.push(format!("{label:30}  CRASH  {e}")),
    };
    let s2 = shaper.shape(&norm).unwrap();
    if s1 != s2 {
        failures.push(format!(
            "{label:30}\n   input  : {word:?}  ({})\n   shape1 : {:?}\n   norm   : {norm:?}  ({})\n   shape2 : {:?}",
            hex(word),
            unit_names(&s1),
            hex(&norm),
            unit_names(&s2)
        ));
    }
}

fn check_aliases(shaper: &Shaper, label: &str, aliases: &str, failures: &mut Vec<String>) {
    for word in aliases_to_words(aliases) {
        if !word.is_empty() {
            check_word(shaper, label, &word, failures);
        }
    }
}

fn report(kind: &str, failures: &[String], total: usize) {
    if !failures.is_empty() {
        eprintln!("First 30 failures:");
        for failure in failures.iter().take(30) {
            eprintln!("{failure}");
        }
        if failures.len() > 30 {
            eprintln!("... ({} more)", failures.len() - 30);
        }
        panic!("{} of {total} {kind} round-trips failed", failures.len());
    }
}

#[test]
fn round_trip_inline() {
    let shaper = shaper();
    let mut failures = Vec::new();
    for (label, aliases) in INLINE_CASES {
        check_aliases(&shaper, label, aliases, &mut failures);
    }
    eprintln!(
        "\nINLINE: {} / {} round-tripped",
        INLINE_CASES.len() - failures.len(),
        INLINE_CASES.len()
    );
    report("inline", &failures, INLINE_CASES.len());
}

#[test]
fn round_trip_core_hud() {
    let shaper = shaper();
    let mut failures = Vec::new();
    let mut word_count = 0;
    for (index, aliases, _) in load_tsv("data/core-hud.tsv") {
        for word in aliases_to_words(&aliases) {
            if word.is_empty() {
                continue;
            }
            word_count += 1;
            check_word(&shaper, &index, &word, &mut failures);
        }
    }
    eprintln!(
        "\nCORE-HUD: {} / {word_count} round-tripped",
        word_count - failures.len()
    );
    report("core-hud", &failures, word_count);
}

#[test]
fn round_trip_eac_hud() {
    let shaper = shaper();
    let mut failures = Vec::new();
    let mut word_count = 0;
    for (index, aliases, _) in load_tsv("data/eac-hud.tsv") {
        if aliases
            .split_whitespace()
            .any(|t| t != "space" && !is_known_alias(t))
        {
            continue;
        }
        for word in aliases_to_words(&aliases) {
            if word.is_empty() {
                continue;
            }
            word_count += 1;
            check_word(&shaper, &index, &word, &mut failures);
        }
    }
    eprintln!(
        "\nEAC-HUD: {} / {word_count} round-tripped",
        word_count - failures.len()
    );
    report("eac-hud", &failures, word_count);
}

/// Stronger property: same shape ⟹ same normalize output, over every corpus word.
#[test]
fn same_shape_same_normalize() {
    let shaper = shaper();
    let mut groups: HashMap<Vec<WrittenUnit>, BTreeMap<String, Vec<String>>> = HashMap::new();
    for word in all_corpus_words() {
        let shape = shaper.shape(&word).unwrap();
        let norm = shaper.normalize(&word).unwrap();
        groups
            .entry(shape)
            .or_default()
            .entry(norm)
            .or_default()
            .push(word);
    }
    let divergences: Vec<_> = groups
        .iter()
        .filter(|(_, by_norm)| by_norm.len() > 1)
        .collect();
    eprintln!(
        "\nSHAPE-CANONICITY: {} / {} shape-groups converge to one normalize",
        groups.len() - divergences.len(),
        groups.len()
    );
    for (shape, by_norm) in divergences.iter().take(10) {
        eprintln!("  shape={:?}", unit_names(shape));
        for (norm, examples) in by_norm.iter() {
            eprintln!(
                "    normalize → {norm:?}  from {:?}",
                &examples[..examples.len().min(3)]
            );
        }
    }
    assert!(
        divergences.is_empty(),
        "{} of {} shape-groups produced >1 distinct normalize output — same shape must map to same Unicode.",
        divergences.len(),
        groups.len()
    );
}

// ── TestParticleUniform ────────────────────────────────────────────────────

#[test]
fn particles_round_trip() {
    let shaper = shaper();
    let mut failures = Vec::new();
    for (label, aliases) in PARTICLE_CASES {
        check_aliases(&shaper, label, aliases, &mut failures);
    }
    report("particle", &failures, PARTICLE_CASES.len());
}

/// For a chain after MVS (except chachlag), stripping the MVS from normalize() output must give
/// a chain that, shaped ALONE, equals the chain portion of the MVS-context shape.
#[test]
fn mvs_uniform_no_mvs_dependency() {
    let shaper = shaper();
    let mut failures = Vec::new();
    for (label, aliases) in PARTICLE_CASES {
        for word in aliases_to_words(aliases) {
            if word.is_empty() || !word.starts_with(MVS_CHAR) {
                continue;
            }
            let norm = shaper.normalize(&word).unwrap();
            if !norm.starts_with(MVS_CHAR) {
                failures.push(format!("{label}: normalize lost MVS prefix: {norm:?}"));
                continue;
            }
            let in_ctx = shaper.shape(&word).unwrap();
            let chain: Vec<WrittenUnit> = in_ctx
                .iter()
                .copied()
                .filter(|u| *u != WrittenUnit::Mvs)
                .collect();
            if chain == [WrittenUnit::Aa] {
                continue; // chachlag keeps `mvs + bare a/e`
            }
            let alone = shaper.shape(&norm[MVS_CHAR.len_utf8()..]).unwrap();
            if alone != chain {
                failures.push(format!(
                    "{label}: chain after MVS depends on MVS to render\n   input: {word:?}\n   normalize: {norm:?}\n   chain alone: {:?}\n   chain in ctx: {:?}",
                    unit_names(&alone),
                    unit_names(&chain)
                ));
            }
        }
    }
    for failure in &failures {
        eprintln!("{failure}");
    }
    assert!(
        failures.is_empty(),
        "{} particle cases depend on MVS for chain rendering",
        failures.len()
    );
}

/// Rule 1: shape ['I'] at iso → `i+fvs1` (not bare `j`).
#[test]
fn i_iso_always_i_fvs1() {
    let shaper = shaper();
    let expected = "\u{1822}\u{180B}";
    for aliases in ["j", "i fvs1"] {
        assert_eq!(
            shaper.normalize(&mgl(aliases)).unwrap(),
            expected,
            "input {aliases}"
        );
    }
}

#[test]
fn no_nirugu_in_normalize_output() {
    let shaper = shaper();
    for (label, aliases) in PARTICLE_CASES {
        for word in aliases_to_words(aliases) {
            if word.is_empty() {
                continue;
            }
            let norm = shaper.normalize(&word).unwrap();
            assert!(
                !norm.contains('\u{180A}'),
                "{label}: normalize({word:?}) = {norm:?} contains nirugu"
            );
        }
    }
}

#[test]
fn equivalence_groups_converge() {
    let shaper = shaper();
    for (label, alias_list) in PARTICLE_EQUIVALENCE_GROUPS {
        let mut shapes = std::collections::HashSet::new();
        let mut outputs = std::collections::HashSet::new();
        for aliases in *alias_list {
            for word in aliases_to_words(aliases) {
                if word.is_empty() {
                    continue;
                }
                shapes.insert(shaper.shape(&word).unwrap());
                outputs.insert(shaper.normalize(&word).unwrap());
            }
        }
        assert_eq!(
            shapes.len(),
            1,
            "{label}: inputs have DIFFERENT shapes — not a valid equivalence group"
        );
        assert_eq!(
            outputs.len(),
            1,
            "{label}: inputs {alias_list:?} produced {} distinct normalize: {outputs:?}",
            outputs.len()
        );
    }
}

/// Python `_check_chain_shape_uniform`: `None` on pass / N/A, else a failure description.
fn check_chain_shape_uniform(shaper: &Shaper, word: &str) -> Option<String> {
    let with_mvs_shape = shaper.shape(word).unwrap();
    if with_mvs_shape.first() != Some(&WrittenUnit::Mvs) {
        return None;
    }
    let except_shape = &with_mvs_shape[1..];
    let with_mvs_norm = shaper.normalize(word).unwrap();
    if !with_mvs_norm.starts_with(MVS_CHAR) {
        return None;
    }
    let without_mvs_norm = &with_mvs_norm[MVS_CHAR.len_utf8()..];
    let without_mvs_shape = shaper.shape(without_mvs_norm).unwrap();
    (except_shape != without_mvs_shape.as_slice()).then(|| {
        format!(
            "input {word:?}\n   shape(input): {:?}\n   expect (strip 1st mvs): {:?}\n   normalize: {with_mvs_norm:?}\n   shape of stripped: {:?}",
            unit_names(&with_mvs_shape),
            unit_names(except_shape),
            unit_names(&without_mvs_shape)
        )
    })
}

/// Data-driven sweep over the full MNG particle dictionary (read from `mongol_norm/data/MNG.json`).
#[test]
fn particles_from_data() {
    let shaper = shaper();
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../mongol_norm/data/MNG.json");
    let data = Json::parse(&std::fs::read_to_string(&path).expect("read MNG.json"));
    let mut keys: Vec<String> = match data.index("particles") {
        Json::Object(fields) => fields.iter().map(|(k, _)| k.clone()).collect(),
        other => panic!("particles is not an object: {other:?}"),
    };
    keys.sort();
    assert_eq!(keys.len(), 47);
    let mut failures = Vec::new();
    let (mut checked, mut skipped_no_mvs) = (0, 0);
    for key in &keys {
        let word = mgl(key);
        if !word.starts_with(MVS_CHAR) {
            skipped_no_mvs += 1;
            continue;
        }
        if let Some(failure) = check_chain_shape_uniform(&shaper, &word) {
            failures.push(format!("particle {key:?}:\n   {failure}"));
        }
        checked += 1;
    }
    eprintln!("\nparticle data sweep: {} particles total, {checked} checked, {skipped_no_mvs} no-mvs skipped", keys.len());
    for failure in failures.iter().take(20) {
        eprintln!("{failure}");
    }
    assert!(
        failures.is_empty(),
        "{} particles fail shape-uniformity",
        failures.len()
    );
}

// ── TestNormalizeFast ──────────────────────────────────────────────────────

/// A paragraph of long compound words normalizes well under the old ~10-20s: cold < 5s, and each
/// word round-trips.
#[test]
fn normalize_text_throughput() {
    let shaper = shaper();
    let words = [
        "ᠦᠢᠯᠡᠳᠦᠯᠭᠡᠵᠢᠭᠦᠯᠬᠦ",
        "ᠳᠡᠭᠡᠭᠰᠢᠯᠡᠭᠦᠯᠬᠦ",
        "ᠲᠡᠷᠭᠡᠯᠰᠢᠭᠦᠯᠬᠦ",
        "ᠪᠦᠯᠬᠦᠮᠳᠡᠰᠦᠭᠡᠢ",
    ];
    let start = Instant::now();
    for word in words {
        let norm = shaper.normalize(word).unwrap();
        assert_eq!(
            shaper.shape(&norm).unwrap(),
            shaper.shape(word).unwrap(),
            "{word:?} did not round-trip"
        );
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "long-word batch took {elapsed:?} (want <5s)"
    );
}
