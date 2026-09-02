"""
Tests for the ``mongol-norm`` command line (``normalize`` / ``normalize-text``
error reporting and the ``--allow-fallback`` flag).

The CLI is the Rust crate's (``crates/mongol-norm/src/cli.rs``, reached through
``mongol_norm.shaper.main``) and writes to the process's real file descriptors,
so every test runs it in a subprocess (``tests._support.run_cli``). The
normalization fallback paths — which no real input triggers — are covered
in-process by the crate's ``cli::tests``
(``strict_fallback_prints_error_and_exits_nonzero``,
``strict_normalize_text_reports_the_failing_word``,
``strict_batch_reports_the_failing_line``, ``allow_fallback_returns_uncovered_input``).
"""
import unittest

from mongol_norm import MongolianShaper
from tests._support import run_cli

SAIN = "ᠰᠠᠢᠨ"
MONGGOL = "ᠮᠣᠩᠭᠣᠯ"


class TestNormalizeCli(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.shaper = MongolianShaper(locale="MNG")

    def test_allow_fallback_is_accepted_and_still_normalizes_a_covered_word(self):
        cases = (
            ("normalize", SAIN, self.shaper.normalize(SAIN)),
            ("normalize-text", "Hello " + SAIN + " world",
             self.shaper.normalize_text("Hello " + SAIN + " world")),
        )
        for command, text, expected in cases:
            with self.subTest(command=command):
                result = run_cli(command, "--allow-fallback", text)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, expected + "\n")
                self.assertEqual(result.stderr, "")
                # The flag only matters for uncovered words: a covered one is
                # canonicalized exactly as without it.
                self.assertEqual(result.stdout, run_cli(command, text).stdout)

    def test_strict_normalize_rejects_a_non_mongolian_character(self):
        result = run_cli("normalize", "Hello")

        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, "")
        self.assertIn(
            "error: non-Mongolian character 'H' (U+0048) at index 0",
            result.stderr,
        )
        self.assertIn("use normalize_text()", result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_batch_reports_the_failing_line_for_non_mongolian_input(self):
        cases = (
            ("normalize", SAIN + "\nHello", "line 2"),
            ("shape", "Hello\n" + SAIN, "line 1"),
        )
        for command, text, expected_line in cases:
            with self.subTest(command=command):
                result = run_cli(command, "--batch", text)
                self.assertEqual(result.returncode, 2)
                self.assertIn(
                    expected_line + ": non-Mongolian character", result.stderr
                )
                self.assertNotIn("Traceback", result.stderr)

    def test_normalize_text_batch_keeps_one_result_per_line(self):
        lines = ["Hello " + SAIN, "English only", MONGGOL + " " + SAIN]
        result = run_cli("normalize-text", "--batch", "\n".join(lines))

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout,
            "".join(self.shaper.normalize_text(line) + "\n" for line in lines),
        )

    def test_normalize_text_leaves_non_mongolian_text_untouched(self):
        result = run_cli("normalize-text", "Hello, world! 123")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "Hello, world! 123\n")

    def test_same_reports_visual_identity_via_exit_code(self):
        same = run_cli("same", SAIN, "ᠰᠡᠢᠨ")
        self.assertEqual((same.returncode, same.stdout), (0, "true\n"), same.stderr)

        different = run_cli("same", SAIN, MONGGOL)
        self.assertEqual(
            (different.returncode, different.stdout), (1, "false\n"), different.stderr
        )


if __name__ == "__main__":
    unittest.main()
