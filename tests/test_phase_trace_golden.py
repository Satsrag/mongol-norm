"""Golden traces for the ordered MNG shaping rules.

The fixture freezes rule order and condition transitions at every phase.  It is
intentionally independent of ``shape_detailed()`` so a refactor cannot make the
producer and verifier share the same new tracing implementation.
"""
import json
import unittest
from pathlib import Path

from mongol_norm import MongolianShaper


_GOLDEN = Path(__file__).parent / "golden" / "mng-phase-trace-v1.json"


def _trace(shaper, codepoints):
    text = "".join(chr(cp) for cp in codepoints)
    tokens = shaper.tokenize(text)
    shaper.assign_positions(tokens)

    transitions = []
    for rule in shaper._shaping_rules:
        before = [token.condition for token in tokens]
        rule.apply(tokens, shaper)
        changes = []
        for index, (old, token) in enumerate(zip(before, tokens)):
            if old != token.condition:
                changes.append({
                    "token": index,
                    "before": old,
                    "after": token.condition,
                })
        if changes:
            transitions.append({"rule": rule.name, "changes": changes})

    for token in tokens:
        shaper._resolve_token_written(token)

    return {
        "positions": [token.position for token in tokens],
        "transitions": transitions,
        "final_conditions": [token.condition for token in tokens],
        "written_by_token": [list(token.written or ()) for token in tokens],
        "shape": shaper.shape(text),
    }


class TestMNGPhaseTraceGolden(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.shaper = MongolianShaper(locale="MNG")
        with _GOLDEN.open(encoding="utf-8") as stream:
            cls.golden = json.load(stream)

    def test_schema_and_rule_order(self):
        self.assertEqual(self.golden["schema"], "mongol-norm-phase-trace/1")
        self.assertEqual(self.golden["locale"], "MNG")
        self.assertEqual(
            self.golden["rules"],
            [rule.name for rule in self.shaper._shaping_rules],
        )

    def test_vectors_match_runtime(self):
        for vector in self.golden["vectors"]:
            with self.subTest(vector=vector["id"]):
                self.assertEqual(
                    _trace(self.shaper, vector["input_cps"]),
                    vector["expected"],
                )

    def test_every_rule_has_a_transition_vector(self):
        exercised = {
            transition["rule"]
            for vector in self.golden["vectors"]
            for transition in vector["expected"]["transitions"]
        }
        self.assertEqual(exercised, set(self.golden["rules"]))

    def test_each_vector_exercises_its_declared_witness(self):
        for vector in self.golden["vectors"]:
            transitioned = {item["rule"]
                            for item in vector["expected"]["transitions"]}
            with self.subTest(vector=vector["id"]):
                self.assertIn(vector["witness_rule"], transitioned)


if __name__ == "__main__":
    unittest.main()
