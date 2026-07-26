#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Public written-unit normalization API tests / 公开书写单元规范化 API 测试。"""

import unittest

from mongol_norm import MongolianShaper


class TestNormalizeWrittenUnits(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.shaper = MongolianShaper(locale="MNG")

    def test_does_not_insert_unrequested_zwj(self):
        # O has connected-position encodings but no isolated encoding. The API
        # must not invent a surrounding ZWJ to make this singleton encodable.
        with self.assertRaisesRegex(ValueError, "no canonical MNG encoding"):
            self.shaper.normalize_written_units(["O"])

    def test_empty_sequence_encodes_as_empty_text(self):
        self.assertEqual(self.shaper.normalize_written_units([]), "")

    def test_shape_output_is_accepted_directly(self):
        nominal = "\u1832\u1820\u182F\u180E\u1820"
        units = self.shaper.shape(nominal)

        self.assertEqual(
            self.shaper.normalize_written_units(units),
            self.shaper.normalize(nominal),
        )

    def test_existing_velar_feminine_refinement_is_reused(self):
        nominal = "\u182C\u180C\u1826"  # h+FVS2 + ue -> G Ue
        units = self.shaper.shape(nominal)

        result = self.shaper.normalize_written_units(units)

        self.assertEqual(result, self.shaper.normalize(nominal))
        self.assertEqual(self.shaper.shape(result), units)

    def test_public_control_names_reuse_structural_chain_encoding(self):
        cases = [
            "\u1832\u1820\u182F\u180E\u1820",  # tal + MVS + a
            "\u180A\u1823",                    # nirugu + o
            "\u200D\u1823",                    # ZWJ + o
        ]
        public_controls = {"mvs": "MVS", "nirugu": "Nirugu", "zwj": "ZWJ"}

        for nominal in cases:
            internal_shape = self.shaper.shape(nominal)
            public_units = [public_controls.get(unit, unit) for unit in internal_shape]
            result = self.shaper.normalize_written_units(public_units)

            self.assertEqual(result, self.shaper.normalize(nominal))
            self.assertEqual(self.shaper.shape(result), internal_shape)

    def test_rejects_iterables_that_are_not_ordered_sequences(self):
        invalid_inputs = [
            {"B": None, "Aa": None},
            {"B", "Aa"},
            (unit for unit in ["B", "Aa"]),
            iter(["B", "Aa"]),
        ]

        for invalid in invalid_inputs:
            with self.subTest(input_type=type(invalid).__name__):
                with self.assertRaisesRegex(TypeError, "ordered sequence"):
                    self.shaper.normalize_written_units(invalid)

    def test_rejects_unknown_unit_with_its_index(self):
        with self.assertRaisesRegex(ValueError, r"written_units\[1\].*Unknown"):
            self.shaper.normalize_written_units(["B", "Unknown"])

    def test_rejects_non_string_unit_with_its_index(self):
        with self.assertRaisesRegex(TypeError, r"written_units\[1\].*string"):
            self.shaper.normalize_written_units(["B", None])

    def test_rejects_string_instead_of_a_unit_sequence(self):
        with self.assertRaisesRegex(TypeError, "ordered sequence"):
            self.shaper.normalize_written_units("B")

    def test_plain_units_encode_to_canonical_unicode(self):
        result = self.shaper.normalize_written_units(["B", "Aa"])

        self.assertEqual(result, "\u182A\u1820\u180B")
        self.assertEqual(self.shaper.shape(result), ["B", "Aa"])


if __name__ == "__main__":
    unittest.main()
