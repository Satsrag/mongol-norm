//! Byte-for-byte canonical Unicode golden vectors for MNG corpus shapes — port of
//! `tests/test_canonical_golden.py`.

mod common;

use std::collections::HashSet;

use common::json::Json;
use common::{all_corpus_words, unit_names};
use mongol_norm::{Locale, Shaper};

fn load() -> (Json, Vec<Json>) {
    let text = common::read_fixture("golden/mng-canonical-v1.jsonl");
    let mut records: Vec<Json> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(Json::parse)
        .collect();
    assert!(!records.is_empty());
    let manifest = records.remove(0);
    assert_eq!(
        manifest.index("type").as_str(),
        "manifest",
        "first canonical golden record must be a manifest"
    );
    for record in &records {
        assert_eq!(
            record.index("type").as_str(),
            "vector",
            "canonical golden contains a non-vector record"
        );
    }
    (manifest, records)
}

fn text_of(cps: &Json) -> String {
    cps.as_array()
        .iter()
        .map(|cp| char::from_u32(cp.as_u64() as u32).unwrap())
        .collect()
}

/// Python `_shape_groups`: the smallest (code-point order) corpus word per shape.
fn corpus_shapes(shaper: &Shaper) -> HashSet<Vec<String>> {
    all_corpus_words()
        .iter()
        .map(|word| unit_names(&shaper.shape(word).unwrap()))
        .collect()
}

#[test]
fn fixture_metadata() {
    let (manifest, _) = load();
    assert_eq!(
        manifest.index("schema").as_str(),
        "mongol-norm-canonical-golden/1"
    );
    assert_eq!(manifest.index("locale").as_str(), "MNG");
    assert_eq!(
        manifest.index("canonical_version").as_str(),
        "mng-canonical/1"
    );
    assert_eq!(
        Shaper::new(Locale::Mng).canonical_version(),
        Some(manifest.index("canonical_version").as_str())
    );
}

#[test]
fn fixture_covers_every_current_corpus_shape_group() {
    let (_, vectors) = load();
    let golden: HashSet<Vec<String>> = vectors.iter().map(|v| v.index("shape").strings()).collect();
    assert_eq!(golden, corpus_shapes(&Shaper::new(Locale::Mng)));
}

#[test]
fn fixture_has_frozen_unique_cardinality() {
    let (_, vectors) = load();
    assert_eq!(vectors.len(), 1993);
    let ids: Vec<String> = vectors
        .iter()
        .map(|v| v.index("id").as_str().to_owned())
        .collect();
    let expected: Vec<String> = (1..=1993).map(|i| format!("shape-{i:04}")).collect();
    assert_eq!(ids, expected);
    let shapes: HashSet<Vec<String>> = vectors.iter().map(|v| v.index("shape").strings()).collect();
    assert_eq!(shapes.len(), 1993);
}

#[test]
fn canonical_codepoints_are_frozen() {
    let (_, vectors) = load();
    let shaper = Shaper::new(Locale::Mng);
    for vector in &vectors {
        let id = vector.index("id").as_str();
        let text = text_of(vector.index("input_cps"));
        assert_eq!(
            unit_names(&shaper.shape(&text).unwrap()),
            vector.index("shape").strings(),
            "{id}: shape"
        );
        let normalized: Vec<u64> = shaper
            .normalize(&text)
            .unwrap()
            .chars()
            .map(|c| c as u64)
            .collect();
        let expected: Vec<u64> = vector
            .index("normalized_cps")
            .as_array()
            .iter()
            .map(Json::as_u64)
            .collect();
        assert_eq!(normalized, expected, "{id}: normalized code points");
    }
}

#[test]
#[should_panic(expected = "unknown aliases")]
fn unknown_alias_fails_instead_of_shrinking_coverage() {
    common::aliases_to_words("a typo_alias");
}
