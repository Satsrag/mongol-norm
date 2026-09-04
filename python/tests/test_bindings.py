#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Tests for the binding layer: ``mongol_norm/_api.py`` over the native extension.

The engine's behaviour is covered by the shaping / normalization suites; these
tests pin what the wrapper adds — the dict formats ``shape_detailed`` and
``trace`` build from the native tuples, the exception mapping and messages,
version lockstep with the installed distribution, the ``mongol_norm.shaper``
compatibility shim and the console-script entry point.
绑定层测试:字典格式、异常映射与文案、版本一致、兼容 shim 与命令行入口。
"""
import importlib.metadata
import re
import subprocess
import sys
import unittest
from pathlib import Path

import mongol_norm
from mongol_norm import MongolianShaper, NormalizationFallbackError
from tests._support import empty_table_shaper, needs_testing_hook, run_cli

SAIN = "ᠰᠠᠢᠨ"
MONGGOL = "ᠮᠣᠩᠭᠣᠯ"
_NO_TABLE_MESSAGE = (
    "no bundled normalize table for locale 'TOD'; "
    "generate it with scripts/gen_normalize_table.py"
)


class TestShapeDetailed(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.shaper = MongolianShaper(locale="MNG")

    def test_exact_records_for_fvs_mvs_and_nnbsp(self):
        # b a g+FVS2 MVS a NNBSP y i n: an FVS'd letter, a chachlag vowel after
        # MVS, and a particle after NNBSP (which shaping treats as MVS).
        # (Escapes: the format controls are invisible and editors mangle them.)
        text = "ᠪᠠᠭ\u180C\u180Eᠠ\u202Fᠶᠢᠨ"
        self.assertEqual(self.shaper.shape_detailed(text), [
            {"cp": "U+182A", "alias": "b", "position": "init",
             "fvs": "", "condition": "", "written": ["B"]},
            {"cp": "U+1820", "alias": "a", "position": "medi",
             "fvs": "", "condition": "", "written": ["A"]},
            {"cp": "U+182D", "alias": "g", "position": "fina",
             "fvs": "+FVS2", "condition": "", "written": ["G"]},
            {"cp": "U+180E", "alias": "mvs", "position": "isol",
             "fvs": "", "condition": "", "written": []},
            {"cp": "U+1820", "alias": "a", "position": "isol",
             "fvs": "", "condition": "chachlag", "written": ["Aa"]},
            {"cp": "U+180E", "alias": "mvs", "position": "isol",
             "fvs": "", "condition": "", "written": []},
            {"cp": "U+1836", "alias": "y", "position": "init",
             "fvs": "", "condition": "particle", "written": ["I"]},
            {"cp": "U+1822", "alias": "i", "position": "medi",
             "fvs": "", "condition": "", "written": ["I"]},
            {"cp": "U+1828", "alias": "n", "position": "fina",
             "fvs": "", "condition": "devsger", "written": ["A"]},
        ])

    def test_fvs_labels(self):
        cases = [
            ("ᠠ\u180B", "+FVS1"),
            ("ᠠ\u180C", "+FVS2"),
            ("ᠠ\u180D", "+FVS3"),
            ("ᠭ\u180F", "+FVS4"),
            ("ᠠ", ""),
        ]
        for text, label in cases:
            with self.subTest(text=text):
                details = self.shaper.shape_detailed(text)
                self.assertEqual(len(details), 1)
                self.assertEqual(details[0]["fvs"], label)

    def test_structural_tokens_use_their_control_aliases(self):
        for text, cp, alias in [
            ("\u180Aᠣ", "U+180A", "nirugu"),   # nirugu o
            ("\u200Dᠳ", "U+200D", "zwj"),      # ZWJ d
            ("\u180E", "U+180E", "mvs"),            # MVS
            ("\u202F", "U+180E", "mvs"),            # NNBSP reads as MVS
        ]:
            with self.subTest(text=text):
                first = self.shaper.shape_detailed(text)[0]
                self.assertEqual((first["cp"], first["alias"]), (cp, alias))
                self.assertEqual(first["written"], [])
                self.assertEqual(first["fvs"], "")
                self.assertEqual(first["condition"], "")

    def test_record_keys_and_written_units_concatenate_to_shape(self):
        words = [
            SAIN, MONGGOL,
            "\u180Aᠣ\u180A",                    # nirugu o nirugu
            "\u200Dᠳ",                          # ZWJ d
            "ᠲᠠᠯ\u180Eᠠ",        # t a l MVS a
        ]
        for text in words:
            with self.subTest(text=text):
                details = self.shaper.shape_detailed(text)
                for detail in details:
                    self.assertEqual(
                        set(detail),
                        {"cp", "alias", "position", "fvs", "condition", "written"},
                    )
                # shape_detailed reports each token's own written units, so the
                # concatenation is the RAW sequence — shape() folds the four duplicate
                # encodings, which is a whole-word rewrite no single token can carry.
                # shape_detailed 报告每个 token 自身的书写单元,故拼接结果是原始序列。
                self.assertEqual(
                    [unit for detail in details for unit in detail["written"]],
                    [unit for unit in self.shaper._shape_raw(text)
                     if unit not in ("Mvs", "Nirugu", "Zwj")],
                )

    def test_rejects_non_mongolian_input_like_shape(self):
        with self.assertRaisesRegex(
                ValueError, r"non-Mongolian character 'H' \(U\+0048\) at index 0"):
            self.shaper.shape_detailed("Hello")
        self.assertEqual(self.shaper.shape_detailed(""), [])


class TestTrace(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.shaper = MongolianShaper(locale="MNG")

    def test_dict_format(self):
        trace = self.shaper.trace("ᠲᠠᠯ\u180Eᠠ")  # t a l MVS a
        self.assertEqual(trace, {
            "positions": ["init", "medi", "fina", "isol", "isol"],
            "transitions": [
                {"rule": "III.1.chachlag",
                 "changes": [{"token": 4, "before": None, "after": "chachlag"}]},
                {"rule": "III.2e.n_t_d.onset_devsger",
                 "changes": [{"token": 0, "before": None, "after": "onset"}]},
            ],
            "final_conditions": ["onset", None, None, None, "chachlag"],
            "written_by_token": [["T"], ["A"], ["L"], [], ["Aa"]],
            "shape": ["T", "A", "L", "Mvs", "Aa"],
        })

    def test_empty_text(self):
        self.assertEqual(self.shaper.trace(""), {
            "positions": [], "transitions": [], "final_conditions": [],
            "written_by_token": [], "shape": [],
        })

    def test_transitions_follow_the_pipeline_and_only_record_changes(self):
        rules = self.shaper.rule_names()
        for text in (SAIN, MONGGOL, "ᠪᠠ", "ᠭᠡᠷ"):
            with self.subTest(text=text):
                trace = self.shaper.trace(text)
                order = [rules.index(transition["rule"])
                         for transition in trace["transitions"]]
                self.assertEqual(order, sorted(order))
                self.assertEqual(len(order), len(set(order)))
                for transition in trace["transitions"]:
                    self.assertTrue(transition["changes"])
                    for change in transition["changes"]:
                        self.assertNotEqual(change["before"], change["after"])
                        self.assertLess(change["token"], len(trace["positions"]))

    def test_rule_names_are_the_ordered_pipeline(self):
        rules = self.shaper.rule_names()
        self.assertEqual(rules[0], "III.1.chachlag")
        self.assertEqual(rules[-1], "III.5.post_bowed")
        self.assertEqual(len(rules), len(set(rules)))
        self.assertIsInstance(rules, list)


class TestErrorMapping(unittest.TestCase):
    @needs_testing_hook
    def test_fallback_error_carries_text_and_written_units(self):
        shaper = empty_table_shaper()
        self.assertIsInstance(shaper, MongolianShaper)
        self.assertEqual(shaper.locale, "MNG")
        self.assertEqual(shaper.shape(SAIN), ["S", "A", "I", "I", "A"])

        with self.assertRaises(NormalizationFallbackError) as raised:
            shaper.normalize(SAIN)
        error = raised.exception
        self.assertIsInstance(error, ValueError)
        self.assertEqual(error.text, SAIN)
        self.assertEqual(error.written_units, ("S", "A", "I", "I", "A"))
        self.assertIsInstance(error.written_units, tuple)
        self.assertEqual(
            str(error),
            "normalization fallback: no canonical encoding for written units S+A+I+I+A",
        )
        # The native signal is translated, not chained.
        self.assertIsNone(error.__cause__)
        self.assertTrue(error.__suppress_context__)

    @needs_testing_hook
    def test_fallback_error_inside_mixed_text_names_the_word(self):
        shaper = empty_table_shaper()
        with self.assertRaises(NormalizationFallbackError) as raised:
            shaper.normalize_text("Hello " + SAIN + " world")
        self.assertEqual(raised.exception.text, SAIN)
        self.assertEqual(raised.exception.written_units, ("S", "A", "I", "I", "A"))

    def test_locales_without_a_table_raise_runtime_error(self):
        tod = MongolianShaper("TOD")
        self.assertEqual(tod.locale, "TOD")
        self.assertTrue(tod.shape("ᠠ"))  # shaping needs no table
        with self.assertRaises(RuntimeError) as raised:
            tod.canonical_version
        self.assertEqual(str(raised.exception), _NO_TABLE_MESSAGE)
        with self.assertRaises(RuntimeError) as raised:
            tod.normalize_written_units(["A"])
        self.assertEqual(str(raised.exception), _NO_TABLE_MESSAGE)
        with self.assertRaises(RuntimeError) as raised:
            tod.normalize("ᠠ")
        self.assertEqual(str(raised.exception), _NO_TABLE_MESSAGE)

    def test_unknown_locale_is_a_value_error(self):
        with self.assertRaises(ValueError) as raised:
            MongolianShaper("XX")
        self.assertEqual(str(raised.exception), "unknown locale 'XX'")

    def test_default_locale_is_mng(self):
        self.assertEqual(MongolianShaper().locale, "MNG")

    def test_non_mongolian_input_is_a_value_error_everywhere(self):
        shaper = MongolianShaper()
        message = r"non-Mongolian character 'H' \(U\+0048\) at index 0"
        for call in (
            lambda: shaper.shape("Hello"),
            lambda: shaper.shape_str("Hello"),
            lambda: shaper.shape_detailed("Hello"),
            lambda: shaper.trace("Hello"),
            lambda: shaper.normalize("Hello"),
            lambda: shaper.normalize("Hello", strict=False),
            lambda: shaper.same_shape(SAIN, "Hello"),
        ):
            with self.assertRaisesRegex(ValueError, message):
                call()
        self.assertEqual(shaper.normalize_text("Hello"), "Hello")


class TestPackageSurface(unittest.TestCase):
    def test_version_matches_the_installed_distribution(self):
        # Independent anchor: the version maturin recorded in the installed
        # distribution's metadata (every installed copy of mongol-norm is
        # considered, so leftover metadata elsewhere on sys.path cannot mask
        # the one the extension was built for).
        installed = {}
        for dist in importlib.metadata.distributions():
            name = re.sub(r"[-_.]+", "-", dist.metadata["Name"] or "").lower()
            if name == "mongol-norm":
                installed[str(getattr(dist, "_path", dist))] = dist.version
        if not installed:
            self.skipTest("mongol-norm is not installed as a distribution")
        self.assertIn(
            mongol_norm.__version__, set(installed.values()),
            "mongol_norm.__version__ matches no installed mongol-norm "
            "distribution: {}".format(installed),
        )
        self.assertRegex(mongol_norm.__version__, r"^\d+\.\d+\.\d+")

    def test_shaper_module_re_exports_the_api(self):
        from mongol_norm import shaper as shim
        self.assertIs(shim.MongolianShaper, mongol_norm.MongolianShaper)
        self.assertIs(shim.NormalizationFallbackError,
                      mongol_norm.NormalizationFallbackError)
        self.assertTrue(callable(shim.main))
        self.assertEqual(
            shim.__all__, ["MongolianShaper", "NormalizationFallbackError", "main"]
        )


class TestConsoleScript(unittest.TestCase):
    def test_version_flags(self):
        for flag in ("--version", "-V"):
            with self.subTest(flag=flag):
                result = run_cli(flag)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(
                    result.stdout, "mongol-norm {}\n".format(mongol_norm.__version__)
                )
                self.assertEqual(result.stderr, "")

    def test_help_flag(self):
        for flag in ("--help", "-h"):
            with self.subTest(flag=flag):
                result = run_cli(flag)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertTrue(result.stdout.startswith("usage: mongol-norm"))
                for command in ("shape", "normalize", "normalize-written-units",
                                "normalize-text", "same"):
                    self.assertIn(command, result.stdout)
                self.assertEqual(result.stderr, "")

    def test_main_exits_with_the_cli_status(self):
        shaped = run_cli("shape", SAIN)
        self.assertEqual((shaped.returncode, shaped.stdout), (0, "S+A+I+I+A\n"))

        unknown = run_cli("bogus")
        self.assertEqual(unknown.returncode, 2)
        self.assertIn("bogus", unknown.stderr)
        self.assertNotIn("Traceback", unknown.stderr)

    def test_installed_console_script(self):
        name = "mongol-norm.exe" if sys.platform == "win32" else "mongol-norm"
        script = Path(sys.executable).parent / name
        if not script.exists():
            self.skipTest("console script not installed next to {}".format(sys.executable))
        result = subprocess.run(
            [str(script), "shape", SAIN],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            encoding="utf-8",
            timeout=60,
        )
        self.assertEqual((result.returncode, result.stdout), (0, "S+A+I+I+A\n"),
                         result.stderr)


if __name__ == "__main__":
    unittest.main()
