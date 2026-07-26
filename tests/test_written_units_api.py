#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Public written-unit normalization API tests / 公开书写单元规范化 API 测试。"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from mongol_norm import MongolianShaper
from mongol_norm.shaper import _parse_written_units


_REPO_ROOT = Path(__file__).resolve().parents[1]


def _run_cli(*args, stdin=None):
    return subprocess.run(
        [
            sys.executable,
            "-c",
            "from mongol_norm.shaper import main; main()",
        ] + list(args),
        input=stdin,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        universal_newlines=True,
        cwd=str(_REPO_ROOT),
    )


class TestNormalizeWrittenUnitsCli(unittest.TestCase):
    def test_missing_subcommand_fails_cleanly(self):
        result = _run_cli()

        self.assertEqual(result.returncode, 2)
        self.assertIn("CMD", result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_ambiguous_compact_units_require_plus_separators(self):
        with self.assertRaisesRegex(ValueError, r"ambiguous.*\+"):
            _parse_written_units("AAA", {"A", "AA"})

    def test_shape_cli_pascal_case_output_pipes_back(self):
        shaped = _run_cli("shape", "\u182A\u200D")

        self.assertEqual(shaped.returncode, 0, shaped.stderr)
        self.assertEqual(shaped.stdout, "B+Zwj\n")
        normalized = _run_cli(
            "normalize-written-units",
            shaped.stdout.rstrip("\n"),
        )
        self.assertEqual(normalized.returncode, 0, normalized.stderr)
        self.assertEqual(
            MongolianShaper(locale="MNG").shape(normalized.stdout.rstrip("\n")),
            ["B", "Zwj"],
        )

    def test_compact_pascal_case_units(self):
        result = _run_cli("normalize-written-units", "BZwj")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            MongolianShaper(locale="MNG").shape(result.stdout.rstrip("\n")),
            ["B", "Zwj"],
        )

    def test_compact_units_are_segmented_before_shape_validation(self):
        result = _run_cli("normalize-written-units", "AAaBZwj")

        self.assertEqual(result.returncode, 2)
        self.assertIn("no canonical MNG encoding", result.stderr)
        self.assertNotIn("is unknown: 'AAaBZwj'", result.stderr)

    def test_all_pascal_case_control_spellings(self):
        shaper = MongolianShaper(locale="MNG")
        cases = [["Mvs", "Aa"], ["Nirugu", "U"], ["Zwj", "Dd"]]
        for units in cases:
            with self.subTest(units=units):
                expected = shaper.normalize_written_units(units)
                result = _run_cli("normalize-written-units", "+".join(units))
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, expected + "\n")

    def test_lowercase_control_spellings_are_rejected(self):
        for control in ("mvs", "nirugu", "zwj"):
            with self.subTest(control=control):
                result = _run_cli("normalize-written-units", control)
                self.assertEqual(result.returncode, 2)
                self.assertIn("is unknown", result.stderr)

    def test_canonical_control_capitalization(self):
        units = ["T", "A", "L", "Mvs", "Aa"]
        expected = MongolianShaper(locale="MNG").normalize_written_units(units)

        result = _run_cli("normalize-written-units", "+".join(units))

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, expected + "\n")

    def test_inline_plus_delimited_units(self):
        result = _run_cli("normalize-written-units", "B+Aa")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "\u182A\u1820\u180B\n")

    def test_non_batch_stdin_accepts_one_transport_newline(self):
        result = _run_cli(
            "normalize-written-units",
            "-",
            stdin="B+Aa\n",
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "\u182A\u1820\u180B\n")

    def test_file_input_and_output(self):
        with tempfile.TemporaryDirectory() as directory:
            input_path = Path(directory) / "units.txt"
            output_path = Path(directory) / "canonical.txt"
            input_path.write_text("B+Aa\n", encoding="utf-8")

            result = _run_cli(
                "normalize-written-units",
                "-i",
                str(input_path),
                "-o",
                str(output_path),
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, "")
            self.assertEqual(
                output_path.read_text(encoding="utf-8"),
                "\u182A\u1820\u180B",
            )

    def test_unknown_and_unencodable_sequences_fail_cleanly(self):
        cases = [("Unknown", "is unknown"), ("O", "no canonical MNG encoding")]
        for text, message in cases:
            with self.subTest(text=text):
                result = _run_cli("normalize-written-units", text)
                self.assertEqual(result.returncode, 2)
                self.assertIn(message, result.stderr)
                self.assertNotIn("Traceback", result.stderr)

    def test_stdin_batch_processes_one_sequence_per_line(self):
        result = _run_cli(
            "normalize-written-units",
            "--batch",
            "-",
            stdin="B+Aa\nB+Aa\n",
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "\u182A\u1820\u180B\n" * 2)

    def test_surrounding_whitespace_is_rejected(self):
        for text in (" B+Aa", "B+Aa ", "\tB+Aa", "B+Aa\t"):
            with self.subTest(text=text):
                result = _run_cli("normalize-written-units", text)
                self.assertEqual(result.returncode, 2)
                self.assertIn("cannot be empty or contain whitespace", result.stderr)

    def test_internal_whitespace_is_rejected(self):
        for text in ("A A", "A A+B"):
            with self.subTest(text=text):
                result = _run_cli("normalize-written-units", text)
                self.assertEqual(result.returncode, 2)
                self.assertIn("whitespace", result.stderr)
                self.assertNotIn("Traceback", result.stderr)

    def test_empty_unit_name_fails_without_a_traceback(self):
        result = _run_cli("normalize-written-units", "B++Aa")

        self.assertEqual(result.returncode, 2)
        self.assertIn("cannot be empty or contain whitespace", result.stderr)
        self.assertNotIn("Traceback", result.stderr)


class TestNormalizeWrittenUnits(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.shaper = MongolianShaper(locale="MNG")

    def test_shape_outputs_pascal_case_controls(self):
        self.assertEqual(self.shaper.shape("\u180E"), ["Mvs"])
        self.assertEqual(self.shaper.shape("\u180A\u1823"), ["Nirugu", "U"])
        self.assertEqual(self.shaper.shape("\u200D\u1833"), ["Zwj", "Dd"])

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

    def test_pascal_case_controls_reuse_structural_chain_encoding(self):
        cases = [
            "\u1832\u1820\u182F\u180E\u1820",  # tal + MVS + a
            "\u180A\u1823",                    # nirugu + o
            "\u200D\u1823",                    # ZWJ + o
        ]
        for nominal in cases:
            units = self.shaper.shape(nominal)
            result = self.shaper.normalize_written_units(units)

            self.assertEqual(result, self.shaper.normalize(nominal))
            self.assertEqual(self.shaper.shape(result), units)

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

    def test_rejects_old_control_spellings(self):
        for control in ("MVS", "mvs", "NIRUGU", "nirugu", "ZWJ", "zwj"):
            with self.subTest(control=control):
                with self.assertRaisesRegex(ValueError, "is unknown"):
                    self.shaper.normalize_written_units([control])

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
