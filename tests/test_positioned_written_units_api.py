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

    def test_rejects_explicit_zwj_input(self):
        with self.assertRaisesRegex(
            ValueError,
            "unsupported positioned control 'Zwj'",
        ):
            self.shaper.normalize_positioned_written_units([
                {"unit": "Zwj", "position": "control"},
            ])

    def test_rejects_unsupported_f_isol_pair(self):
        with self.assertRaisesRegex(
            ValueError,
            "unsupported positioned written unit 'F:isol'",
        ):
            self.shaper.normalize_positioned_written_units([
                {"unit": "F", "position": "isol"},
            ])


    def test_i_isol_and_init_use_the_plain_i_canonical(self):
        expected = self.shaper.normalize_written_units(["I"])
        for position in ("isol", "init"):
            with self.subTest(position=position):
                self.assertEqual(
                    self.shaper.normalize_positioned_written_units([
                        {"unit": "I", "position": position},
                    ]),
                    expected,
                )

    def test_isolated_consonant_borrows_its_initial_written_unit(self):
        result = self.shaper.normalize_positioned_written_units([
            {"unit": "B", "position": "init"},
        ])

        self.assertEqual(result, "\u182a")
        self.assertNotIn("\u200d", result)

    def test_isolated_fa_borrows_the_initial_f_written_unit(self):
        self.assertEqual(self.shaper.shape("\u1839"), ["F"])
        result = self.shaper.normalize_positioned_written_units([
            {"unit": "F", "position": "init"},
        ])

        self.assertEqual(result, "\u1839")
        self.assertNotIn("\u200d", result)

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
            {"unit": "B", "position": "init"},
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

    def test_complete_compound_needs_no_implicit_zwj(self):
        positioned = [
            {"unit": "B", "position": "init"},
            {"unit": "O", "position": "medi"},
            {"unit": "G", "position": "fina"},
        ]
        self.assertEqual(
            self.shaper.normalize_positioned_written_units(positioned),
            self.shaper.normalize_written_units(["B", "O", "G"]),
        )

    def test_medi_started_compound_gets_a_leading_zwj(self):
        positioned = [
            {"unit": "B", "position": "medi"},
            {"unit": "O", "position": "medi"},
            {"unit": "G", "position": "fina"},
        ]
        self.assertEqual(
            self.shaper.normalize_positioned_written_units(positioned),
            self.shaper.normalize_written_units(["Zwj", "B", "O", "G"]),
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
        self.assertEqual(
            self.shaper.normalize_positioned_written_units([
                {"unit": "Mvs", "position": "control"},
                {"unit": "Aa", "position": "fina"},
            ]),
            self.shaper.normalize_written_units(["Mvs", "Zwj", "Aa"]),
        )


    def test_controls_require_control_position_and_letters_reject_it(self):
        malformed = [
            {"unit": "Mvs", "position": "isol"},
            {"unit": "B", "position": "control"},
        ]
        expected = ["requires position 'control'", "unsupported positioned"]
        for record, message in zip(malformed, expected):
            with self.subTest(record=record):
                with self.assertRaisesRegex(ValueError, message):
                    self.shaper.normalize_positioned_written_units([record])

    def test_rejects_unknown_unit_position_pair(self):
        with self.assertRaisesRegex(
            ValueError,
            r"unsupported positioned written unit 'Unknown:isol'",
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

    def test_explicit_nirugu_controls_make_a_medi_position_valid(self):
        positioned = [
            {"unit": "Nirugu", "position": "control"},
            {"unit": "O", "position": "medi"},
            {"unit": "Nirugu", "position": "control"},
        ]

        self.assertEqual(
            self.shaper.normalize_positioned_written_units(positioned),
            self.shaper.normalize_written_units(["Nirugu", "O", "Nirugu"]),
        )

    def test_one_sided_joiners_and_repeated_controls(self):
        cases = [
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

    def test_long_invalid_chain_fails_closed_without_recursion_error(self):
        positioned = [
            {"unit": "A", "position": "isol"}
            for _index in range(1000)
        ]
        with self.assertRaisesRegex(
            ValueError,
            "no canonical MNG encoding in the supplied context",
        ):
            self.shaper.normalize_positioned_written_units(positioned)

    def test_long_control_sequence_stays_iterative(self):
        positioned = [
            {"unit": "Mvs", "position": "control"}
            for _index in range(1000)
        ]
        positioned.append({"unit": "F", "position": "init"})
        self.assertEqual(
            self.shaper.normalize_positioned_written_units(positioned),
            "\u180e" * 1000 + "\u1839",
        )

    def test_record_limit_fails_closed(self):
        positioned = [
            {"unit": "Mvs", "position": "control"}
            for _index in range(1025)
        ]
        with self.assertRaisesRegex(ValueError, "at most 1024 records"):
            self.shaper.normalize_positioned_written_units(positioned)

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

    def test_singleton_medi_and_fina_insert_zwj_by_position(self):
        cases = [
            ("O", "medi", ["Zwj", "O", "Zwj"], 2),
            ("U", "fina", ["Zwj", "U"], 1),
        ]
        for unit, position, plain, zwj_count in cases:
            with self.subTest(unit=unit, position=position):
                result = self.shaper.normalize_positioned_written_units([
                    {"unit": unit, "position": position},
                ])
                self.assertEqual(
                    result,
                    self.shaper.normalize_written_units(plain),
                )
                self.assertEqual(result.count("\u200d"), zwj_count)

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
