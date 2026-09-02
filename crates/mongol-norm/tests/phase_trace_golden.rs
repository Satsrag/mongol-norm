//! Golden traces for the ordered MNG shaping rules (port of `tests/test_phase_trace_golden.py`).
//! The fixture freezes rule order and every condition transition; `Shaper::trace` is the
//! verifier.

mod common;

use std::collections::HashSet;

use common::json::Json;
use mongol_norm::{Locale, ShapeTrace, Shaper};

fn golden() -> Json {
    Json::parse(&common::read_fixture("golden/mng-phase-trace-v1.json"))
}

fn text_of(cps: &Json) -> String {
    cps.as_array()
        .iter()
        .map(|cp| char::from_u32(cp.as_u64() as u32).expect("scalar value"))
        .collect()
}

fn opt_names(values: &Json) -> Vec<Option<String>> {
    values
        .as_array()
        .iter()
        .map(|v| v.as_str_opt().map(str::to_owned))
        .collect()
}

fn assert_trace_matches(id: &str, actual: &ShapeTrace, expected: &Json) {
    let positions: Vec<String> = actual
        .positions
        .iter()
        .map(|p| p.as_str().to_owned())
        .collect();
    assert_eq!(
        positions,
        expected.index("positions").strings(),
        "{id}: positions"
    );

    let expected_transitions = expected.index("transitions").as_array();
    let actual_rules: Vec<&str> = actual.transitions.iter().map(|t| t.rule).collect();
    let expected_rules: Vec<String> = expected_transitions
        .iter()
        .map(|t| t.index("rule").as_str().to_owned())
        .collect();
    assert_eq!(actual_rules, expected_rules, "{id}: transition rule order");
    for (transition, expected_transition) in actual.transitions.iter().zip(expected_transitions) {
        let actual_changes: Vec<(usize, Option<String>, Option<String>)> = transition
            .changes
            .iter()
            .map(|c| {
                (
                    c.token,
                    c.before.map(|x| x.as_str().to_owned()),
                    c.after.map(|x| x.as_str().to_owned()),
                )
            })
            .collect();
        let expected_changes: Vec<(usize, Option<String>, Option<String>)> = expected_transition
            .index("changes")
            .as_array()
            .iter()
            .map(|c| {
                (
                    c.index("token").as_usize(),
                    c.index("before").as_str_opt().map(str::to_owned),
                    c.index("after").as_str_opt().map(str::to_owned),
                )
            })
            .collect();
        assert_eq!(
            actual_changes, expected_changes,
            "{id}: changes of {}",
            transition.rule
        );
    }

    let final_conditions: Vec<Option<String>> = actual
        .final_conditions
        .iter()
        .map(|c| c.map(|x| x.as_str().to_owned()))
        .collect();
    assert_eq!(
        final_conditions,
        opt_names(expected.index("final_conditions")),
        "{id}: final_conditions"
    );

    let written: Vec<Vec<String>> = actual
        .written_by_token
        .iter()
        .map(|units| common::unit_names(units))
        .collect();
    let expected_written: Vec<Vec<String>> = expected
        .index("written_by_token")
        .as_array()
        .iter()
        .map(Json::strings)
        .collect();
    assert_eq!(written, expected_written, "{id}: written_by_token");
    assert_eq!(
        common::unit_names(&actual.shape),
        expected.index("shape").strings(),
        "{id}: shape"
    );
}

#[test]
fn schema_and_rule_order() {
    let golden = golden();
    assert_eq!(golden.index("schema").as_str(), "mongol-norm-phase-trace/1");
    assert_eq!(golden.index("locale").as_str(), "MNG");
    let rules: Vec<String> = Shaper::new(Locale::Mng)
        .rule_names()
        .iter()
        .map(|r| (*r).to_owned())
        .collect();
    assert_eq!(golden.index("rules").strings(), rules);
    assert_eq!(rules.len(), 15);
}

#[test]
fn vectors_match_runtime() {
    let golden = golden();
    let shaper = Shaper::new(Locale::Mng);
    let vectors = golden.index("vectors").as_array();
    assert_eq!(vectors.len(), 15);
    for vector in vectors {
        let id = vector.index("id").as_str();
        let trace = shaper.trace(&text_of(vector.index("input_cps"))).unwrap();
        assert_trace_matches(id, &trace, vector.index("expected"));
    }
}

#[test]
fn every_rule_has_a_transition_vector() {
    let golden = golden();
    let exercised: HashSet<String> = golden
        .index("vectors")
        .as_array()
        .iter()
        .flat_map(|v| v.index("expected").index("transitions").as_array().iter())
        .map(|t| t.index("rule").as_str().to_owned())
        .collect();
    let rules: HashSet<String> = golden.index("rules").strings().into_iter().collect();
    assert_eq!(exercised, rules);
}

#[test]
fn each_vector_exercises_its_declared_witness() {
    let golden = golden();
    for vector in golden.index("vectors").as_array() {
        let transitioned: HashSet<String> = vector
            .index("expected")
            .index("transitions")
            .as_array()
            .iter()
            .map(|t| t.index("rule").as_str().to_owned())
            .collect();
        let witness = vector.index("witness_rule").as_str();
        assert!(
            transitioned.contains(witness),
            "{}: witness {witness} did not fire",
            vector.index("id").as_str()
        );
    }
}
