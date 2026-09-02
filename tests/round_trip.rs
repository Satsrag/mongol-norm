//! Round-trip, shape-canonicity and particle-uniformity properties of normalize — port of
//! `python/tests/test_round_trip.py`.
//!
//! The defining property of a correct normalize is that it preserves shape:
//! `shape(input) == shape(normalize(input))`. The exact Unicode form does not matter here.

mod common;

use std::collections::{BTreeMap, HashMap, HashSet};
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
    let mut word_count = 0;
    for (label, aliases) in INLINE_CASES {
        for word in aliases_to_words(aliases) {
            if word.is_empty() {
                continue;
            }
            word_count += 1;
            check_word(&shaper, label, &word, &mut failures);
        }
    }
    eprintln!(
        "\nINLINE: {} / {word_count} round-tripped",
        word_count - failures.len()
    );
    report("inline", &failures, word_count);
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
    // Sort by unit names so the reported sample is deterministic (HashMap order is not).
    let mut divergences: Vec<_> = groups
        .iter()
        .filter(|(_, by_norm)| by_norm.len() > 1)
        .collect();
    divergences.sort_by_key(|(shape, _)| unit_names(shape));
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

/// Data-driven sweep over the full MNG particle dictionary (read from
/// `python/mongol_norm/data/MNG.json`, the source of the generated tables).
#[test]
fn particles_from_data() {
    let shaper = shaper();
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("python/mongol_norm/data/MNG.json");
    let data = Json::parse(&std::fs::read_to_string(&path).expect("read MNG.json"));
    let mut keys: Vec<String> = match data.index("particles") {
        Json::Object(fields) => fields.iter().map(|(k, _)| k.clone()).collect(),
        other => panic!("particles is not an object: {other:?}"),
    };
    keys.sort();
    assert_eq!(
        keys.len(),
        47,
        "the particle inventory in python/mongol_norm/data/MNG.json changed"
    );
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

// ── Bare-chain encodings (Python used private helpers; the public written-unit API is the same path) ──

const LONG_CHAIN: [WrittenUnit; 18] = [
    WrittenUnit::A,
    WrittenUnit::O,
    WrittenUnit::I,
    WrittenUnit::I,
    WrittenUnit::L,
    WrittenUnit::A,
    WrittenUnit::D,
    WrittenUnit::O,
    WrittenUnit::L,
    WrittenUnit::G,
    WrittenUnit::A,
    WrittenUnit::J,
    WrittenUnit::I,
    WrittenUnit::G,
    WrittenUnit::O,
    WrittenUnit::L,
    WrittenUnit::G,
    WrittenUnit::O,
];

/// An 18-unit single morpheme must encode in milliseconds (`TestNormalizeFast.test_long_chain_is_fast`).
#[test]
fn long_chain_is_fast() {
    let shaper = shaper();
    let start = Instant::now();
    let encoded = shaper.normalize_written_units(&LONG_CHAIN).unwrap();
    let elapsed = start.elapsed();
    assert_eq!(
        shaper.shape(&encoded).unwrap(),
        LONG_CHAIN,
        "long-chain encoding must round-trip"
    );
    assert!(
        elapsed.as_millis() < 200,
        "18-unit chain took {elapsed:?} (want <200ms)"
    );
}

/// Prefix stability: A = B + suffix ⟹ the shared region (all of B's letters except the boundary
/// letter, whose position flips fina→medi) encodes identically.
#[test]
fn ab_shared_prefix_identical() {
    let shaper = shaper();
    let a = &LONG_CHAIN[..];
    let b = &LONG_CHAIN[..9];
    let letters_a = common::split_letters(&shaper.normalize_written_units(a).unwrap());
    let letters_b = common::split_letters(&shaper.normalize_written_units(b).unwrap());
    let shared = letters_b.len() - 1;
    assert_eq!(
        letters_a[..shared],
        letters_b[..shared],
        "shared prefix diverges"
    );
}

/// Prefix stability over real corpus pairs: wherever shape(B) is a shape-prefix of shape(A), the
/// shared region (all of B's letters except the boundary one) encodes identically. This is exact
/// today — all 2237 pairs hold — and both the pair count and the violation count are pinned, so a
/// regression shows up as a failure rather than as a slightly lower percentage.
#[test]
fn corpus_real_pair_stability() {
    let shaper = shaper();
    let mut shapes: HashMap<Vec<WrittenUnit>, String> = HashMap::new();
    for word in all_corpus_words() {
        let shape = shaper.shape(&word).unwrap();
        if shape.iter().any(|unit| unit.is_structural()) {
            continue; // structural tokens split words into chains; only pure-letter chains qualify
        }
        let encoded = shaper
            .normalize_written_units(&shape)
            .unwrap_or_else(|e| panic!("{:?} failed to encode: {e}", unit_names(&shape)));
        shapes.insert(shape, encoded);
    }
    // Deterministic iteration order so the reported examples are reproducible (HashMap's is not).
    let mut ordered: Vec<(&Vec<WrittenUnit>, &String)> = shapes.iter().collect();
    ordered.sort_by_key(|(shape, _)| unit_names(shape));
    let (mut pairs, mut violations) = (0usize, 0usize);
    let mut examples = Vec::new();
    for (a, encoded_a) in ordered {
        for m in 1..a.len() {
            let Some(encoded_b) = shapes.get(&a[..m]) else {
                continue;
            };
            pairs += 1;
            let letters_a = common::split_letters(encoded_a);
            let letters_b = common::split_letters(encoded_b);
            let shared = letters_b.len().saturating_sub(1);
            if shared > 0 && letters_a[..shared] != letters_b[..shared] {
                violations += 1;
                if examples.len() < 5 {
                    examples.push((unit_names(a), unit_names(&a[..m]), letters_a, letters_b));
                }
            }
        }
    }
    let rate = if pairs == 0 {
        1.0
    } else {
        (pairs - violations) as f64 / pairs as f64
    };
    eprintln!(
        "\nprefix-stability (real corpus pairs): {}/{pairs} = {:.2}%",
        pairs - violations,
        rate * 100.0
    );
    assert_eq!(pairs, 2237, "corpus prefix-pair coverage drifted");
    let report: Vec<String> = examples
        .iter()
        .map(|(a, b, full, prefix)| {
            format!("  VIOL A={a:?} B={b:?}\n    full={full:?}\n    pre ={prefix:?}")
        })
        .collect();
    assert_eq!(
        violations,
        0,
        "prefix-stability regressed: {violations} of {pairs} pairs diverge\n{}",
        report.join("\n")
    );
}

/// Every corpus shape group is reachable through the public written-unit API: compact PascalCase
/// parses back to the shape, and the API agrees with `normalize` of a representative word.
#[test]
fn public_written_unit_api_covers_all_shape_groups() {
    let shaper = shaper();
    let mut representatives: Vec<(Vec<WrittenUnit>, String)> = Vec::new();
    let mut seen: HashSet<Vec<WrittenUnit>> = HashSet::new();
    for word in all_corpus_words() {
        let shape = shaper.shape(&word).unwrap();
        if seen.insert(shape.clone()) {
            representatives.push((shape, word));
        }
    }
    assert_eq!(
        representatives.len(),
        1993,
        "corpus shape-group coverage drifted"
    );
    let mut failures = Vec::new();
    for (shape, word) in &representatives {
        let compact: String = shape.iter().map(|unit| unit.as_str()).collect();
        match shaper.parse_written_units(&compact) {
            Ok(parsed) if &parsed == shape => {}
            Ok(parsed) => failures.push(format!(
                "compact {compact:?} parsed as {:?}",
                unit_names(&parsed)
            )),
            Err(e) => failures.push(format!("compact {compact:?}: {e}")),
        }
        match shaper.normalize_written_units(shape) {
            Err(e) => failures.push(format!("shape={:?}: {e}", unit_names(shape))),
            Ok(encoded) => {
                let expected = shaper.normalize(word).unwrap();
                if encoded != expected {
                    failures.push(format!(
                        "shape={:?}: written-unit API {encoded:?} != normalize {expected:?}",
                        unit_names(shape)
                    ));
                } else if shaper.shape(&encoded).unwrap() != *shape {
                    failures.push(format!(
                        "shape={:?}: output reshaped differently",
                        unit_names(shape)
                    ));
                }
            }
        }
    }
    for failure in failures.iter().take(20) {
        eprintln!("{failure}");
    }
    assert!(
        failures.is_empty(),
        "{} of {} shape groups failed the public written-unit API",
        failures.len(),
        representatives.len()
    );
}
