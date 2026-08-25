import contextlib
import io
import sys
import unittest
from unittest import mock

from mongol_norm.shaper import MongolianShaper, main


def _install_empty_normalize_table(shaper):
    shaper._unit_enc = {}
    shaper._unit_enc_fem = {}
    shaper._unit_enc_max_len = 1


class TestNormalizeStrictCli(unittest.TestCase):
    def _run_cli_expect_error(self, argv):
        stderr = io.StringIO()
        with mock.patch.object(sys, "argv", argv):
            with mock.patch.object(
                    MongolianShaper, "_build_unit_enc",
                    _install_empty_normalize_table):
                with contextlib.redirect_stderr(stderr):
                    with self.assertRaises(SystemExit) as raised:
                        main()
        return raised.exception.code, stderr.getvalue()

    def test_strict_fallback_prints_error_and_exits_nonzero(self):
        code, stderr = self._run_cli_expect_error(
            ["mongol-norm", "normalize", "ᠰᠠᠢᠨ"])
        self.assertEqual(code, 2)
        self.assertIn("normalization fallback", stderr)

    def test_strict_normalize_text_reports_the_failing_word(self):
        code, stderr = self._run_cli_expect_error([
            "mongol-norm", "normalize-text", "Hello ᠰᠠᠢᠨ world",
        ])
        self.assertEqual(code, 2)
        self.assertIn("normalization fallback", stderr)

    def test_strict_batch_reports_the_failing_line(self):
        cases = (
            ("normalize", "ᠰᠠᠢᠨ\nᠮᠣᠩᠭᠣᠯ", "line 1"),
            ("normalize-text", "English only\nHello ᠰᠠᠢᠨ", "line 2"),
        )
        for command, text, expected_line in cases:
            with self.subTest(command=command):
                code, stderr = self._run_cli_expect_error([
                    "mongol-norm", command, "--batch", text,
                ])
                self.assertEqual(code, 2)
                self.assertIn(expected_line + ": normalization fallback", stderr)

    def test_allow_fallback_returns_uncovered_input(self):
        cases = (
            ("normalize", "ᠰᠠᠢᠨ"),
            ("normalize-text", "Hello ᠰᠠᠢᠨ world"),
        )
        for command, text in cases:
            with self.subTest(command=command):
                stdout = io.StringIO()
                argv = [
                    "mongol-norm", command, "--allow-fallback", text,
                ]
                with mock.patch.object(sys, "argv", argv):
                    with mock.patch.object(
                            MongolianShaper, "_build_unit_enc",
                            _install_empty_normalize_table):
                        with contextlib.redirect_stdout(stdout):
                            main()
                self.assertEqual(stdout.getvalue(), text + "\n")


if __name__ == "__main__":
    unittest.main()
