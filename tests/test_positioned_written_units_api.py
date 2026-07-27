#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Public positioned written-unit normalization API tests."""

import subprocess
import sys
import unittest

from mongol_norm import MongolianShaper


class TestNormalizePositionedWrittenUnits(unittest.TestCase):
    def setUp(self):
        self.shaper = MongolianShaper(locale="MNG")

    def test_snapshots_an_accepted_sequence_once(self):
        class ChangingList(list):
            def __init__(self, values):
                super().__init__(values)
                self.iterations = 0

            def __iter__(self):
                self.iterations += 1
                if self.iterations == 1:
                    return super().__iter__()
                return iter(())

        positioned = ChangingList([
            {"unit": "B", "position": "isol"},
        ])

        self.assertEqual(
            self.shaper.normalize_positioned_written_units(positioned),
            self.shaper.normalize_written_units(["B"]),
        )
        self.assertEqual(positioned.iterations, 1)

    def test_empty_sequence_returns_empty_string(self):
        self.assertEqual(
            self.shaper.normalize_positioned_written_units([]),
            "",
        )

    def test_all_letter_positions_follow_the_explicit_sequence(self):
        cases = [
            ([{"unit": "B", "position": "isol"}], ["B"]),
            ([
                {"unit": "B", "position": "init"},
                {"unit": "O", "position": "medi"},
                {"unit": "N", "position": "fina"},
            ], ["B", "O", "N"]),
        ]
        for positioned, plain in cases:
            with self.subTest(positioned=positioned):
                self.assertEqual(
                    self.shaper.normalize_positioned_written_units(positioned),
                    self.shaper.normalize_written_units(plain),
                )

    def test_mvs_splits_letter_position_chains_without_joining(self):
        positioned = [
            {"unit": "T", "position": "init"},
            {"unit": "A", "position": "medi"},
            {"unit": "L", "position": "fina"},
            {"unit": "Mvs", "position": "control"},
            {"unit": "Aa", "position": "isol"},
        ]
        plain = [record["unit"] for record in positioned]

        self.assertEqual(
            self.shaper.normalize_positioned_written_units(positioned),
            self.shaper.normalize_written_units(plain),
        )

    def test_valid_position_still_requires_an_exact_canonical_encoding(self):
        with self.assertRaisesRegex(ValueError, "no canonical MNG encoding"):
            self.shaper.normalize_positioned_written_units([
                {"unit": "O", "position": "isol"},
            ])

    def test_controls_require_control_position_and_letters_reject_it(self):
        malformed = [
            {"unit": "Mvs", "position": "isol"},
            {"unit": "B", "position": "control"},
        ]
        for record in malformed:
            with self.subTest(record=record):
                with self.assertRaisesRegex(ValueError, "sequence gives"):
                    self.shaper.normalize_positioned_written_units([record])

    def test_rejects_unknown_unit_name(self):
        with self.assertRaisesRegex(
            ValueError,
            r"positioned_units\[0\] has unknown unit 'Unknown'",
        ):
            self.shaper.normalize_positioned_written_units([
                {"unit": "Unknown", "position": "isol"},
            ])

    def test_rejects_unknown_position_name(self):
        with self.assertRaisesRegex(
            ValueError,
            r"positioned_units\[0\] has unknown position 'middle'",
        ):
            self.shaper.normalize_positioned_written_units([
                {"unit": "B", "position": "middle"},
            ])

    def test_rejects_non_string_record_values(self):
        malformed = [
            {"unit": 1, "position": "isol"},
            {"unit": "B", "position": None},
        ]
        for record in malformed:
            with self.subTest(record=record):
                with self.assertRaisesRegex(
                    TypeError,
                    r"positioned_units\[0\].*must be a string",
                ):
                    self.shaper.normalize_positioned_written_units([record])

    def test_rejects_missing_or_extra_record_fields(self):
        malformed = [
            {"unit": "B"},
            {"position": "isol"},
            {"unit": "B", "position": "isol", "extra": "value"},
        ]
        for record in malformed:
            with self.subTest(record=record):
                with self.assertRaisesRegex(
                    ValueError,
                    r"positioned_units\[0\] must contain exactly 'unit' and 'position'",
                ):
                    self.shaper.normalize_positioned_written_units([record])

    def test_rejects_non_record_items(self):
        class DictSubclass(dict):
            pass

        malformed = [
            "B",
            ("B", "init"),
            ["B", "init"],
            object(),
            DictSubclass(unit="B", position="isol"),
        ]
        for record in malformed:
            with self.subTest(record=type(record).__name__):
                with self.assertRaisesRegex(
                    TypeError,
                    r"positioned_units\[0\] must be a record",
                ):
                    self.shaper.normalize_positioned_written_units([record])

    def test_rejects_non_sequence_outer_inputs(self):
        malformed = [
            "records",
            b"records",
            {"unit": "B", "position": "isol"},
            {"record"},
            (record for record in []),
            iter([]),
        ]
        for positioned in malformed:
            with self.subTest(positioned=type(positioned).__name__):
                with self.assertRaisesRegex(
                    TypeError,
                    "positioned_units must be an ordered sequence of records",
                ):
                    self.shaper.normalize_positioned_written_units(positioned)

    def test_explicit_zwj_controls_make_a_medi_position_valid(self):
        positioned = [
            {"unit": "Zwj", "position": "control"},
            {"unit": "O", "position": "medi"},
            {"unit": "Zwj", "position": "control"},
        ]

        self.assertEqual(
            self.shaper.normalize_positioned_written_units(positioned),
            self.shaper.normalize_written_units(["Zwj", "O", "Zwj"]),
        )

    def test_one_sided_joiners_and_repeated_controls(self):
        cases = [
            ([
                {"unit": "Zwj", "position": "control"},
                {"unit": "B", "position": "fina"},
            ], ["Zwj", "B"]),
            ([
                {"unit": "B", "position": "init"},
                {"unit": "Zwj", "position": "control"},
            ], ["B", "Zwj"]),
            ([
                {"unit": "Nirugu", "position": "control"},
                {"unit": "U", "position": "fina"},
            ], ["Nirugu", "U"]),
            ([
                {"unit": "A", "position": "init"},
                {"unit": "Nirugu", "position": "control"},
            ], ["A", "Nirugu"]),
            ([
                {"unit": "Mvs", "position": "control"},
                {"unit": "Mvs", "position": "control"},
                {"unit": "Aa", "position": "isol"},
            ], ["Mvs", "Mvs", "Aa"]),
        ]
        for positioned, plain in cases:
            with self.subTest(plain=plain):
                self.assertEqual(
                    self.shaper.normalize_positioned_written_units(positioned),
                    self.shaper.normalize_written_units(plain),
                )

    def test_positioned_records_have_no_cli_subcommand(self):
        result = subprocess.run(
            [
                sys.executable,
                "-c",
                "from mongol_norm.shaper import main; main()",
                "normalize-positioned-written-units",
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            universal_newlines=True,
        )

        self.assertEqual(result.returncode, 2)
        self.assertIn("invalid choice", result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_rejects_a_position_that_needs_implicit_zwj(self):
        positioned = [
            {"unit": "O", "position": "medi"},
        ]

        with self.assertRaisesRegex(
            ValueError,
            r"positioned_units\[0\] requests position 'medi', "
            r"but the sequence gives 'isol'",
        ):
            self.shaper.normalize_positioned_written_units(positioned)

    def test_encodes_a_valid_positioned_sequence(self):
        positioned = (
            {"unit": "B", "position": "init"},
            {"unit": "Aa", "position": "fina"},
        )

        self.assertEqual(
            self.shaper.normalize_positioned_written_units(positioned),
            self.shaper.normalize_written_units(["B", "Aa"]),
        )


if __name__ == "__main__":
    unittest.main()
