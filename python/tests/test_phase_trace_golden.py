"""Golden traces for the ordered MNG shaping rules.

The fixture freezes rule order and condition transitions at every phase. It was
produced by ``python/scripts/gen_compat_goldens.py`` from ``MongolianShaper.trace()``;
the committed JSON is the independent oracle a refactor of the engine must keep
reproducing exactly (the crate's ``tests/phase_trace_golden.rs`` checks the same
file from the Rust side).
"""
import json
import unittest

from mongol_norm import MongolianShaper
from tests._support import fixture_path


_GOLDEN = fixture_path("golden", "mng-phase-trace-v1.json")


class TestMNGPhaseTraceGolden(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.shaper = MongolianShaper(locale="MNG")
        with _GOLDEN.open(encoding="utf-8") as stream:
            cls.golden = json.load(stream)

    def test_schema_and_rule_order(self):
        self.assertEqual(self.golden["schema"], "mongol-norm-phase-trace/1")
        self.assertEqual(self.golden["locale"], "MNG")
        self.assertEqual(self.golden["rules"], self.shaper.rule_names())

    def test_vectors_match_runtime(self):
        for vector in self.golden["vectors"]:
            with self.subTest(vector=vector["id"]):
                text = "".join(chr(cp) for cp in vector["input_cps"])
                self.assertEqual(self.shaper.trace(text), vector["expected"])

    def test_written_by_token_agrees_with_shape_detailed(self):
        for vector in self.golden["vectors"]:
            text = "".join(chr(cp) for cp in vector["input_cps"])
            expected = vector["expected"]
            details = self.shaper.shape_detailed(text)
            with self.subTest(vector=vector["id"]):
                self.assertEqual(
                    [detail["written"] for detail in details],
                    expected["written_by_token"],
                )
                self.assertEqual(
                    [detail["position"] for detail in details],
                    expected["positions"],
                )
                self.assertEqual(
                    [detail["condition"] or None for detail in details],
                    expected["final_conditions"],
                )
                self.assertEqual(self.shaper.shape(text), expected["shape"])

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
