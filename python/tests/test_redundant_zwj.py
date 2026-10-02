"""Issue #33 stage-one Python parity: interior ZWJ only, before duplicates."""
import unittest

from mongol_norm import MongolianShaper

ZWJ = "\u200d"
NIRUGU = "\u180a"


class TestRedundantZwj(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper()

    def assertEquivalent(self, plain, joined):
        self.assertTrue(self.s.same_shape(plain, joined))
        canonical = self.s.normalize(plain)
        self.assertEqual(self.s.normalize(joined), canonical)
        self.assertEqual(self.s.shape(canonical), self.s.shape(plain))
        self.assertEqual(self.s.normalize(canonical), canonical)
        return canonical

    def test_contraction_happens_after_joiner_removal(self):
        plain = "ᠯᠢᠡ\u180b"
        joined = "ᠯᠢ\u200dᠡ\u180b"
        self.assertEqual(self.s._shape_raw(joined), ["L", "I", "Zwj", "Aa"])
        self.assertEqual(self.s.shape(joined), ["L", "G"])
        canonical = self.assertEquivalent(plain, joined)
        self.assertNotIn(ZWJ, canonical)
        self.assertEqual(self.s.trace(joined)["shape"], ["L", "G"])
        self.assertEqual(len(self.s.trace(joined)["positions"]), 4)
        self.assertEqual(self.s.shape_detailed(joined)[2]["cp"], "U+200D")
        self.assertEqual(self.s.normalize_text("before " + joined + " after"),
                         "before " + canonical + " after")

    def test_nirugu_repetitions_and_contextual_forms(self):
        for left, right in [("ᠪ", "ᠠ"), ("ᠠ", "ᠢᠨ"), ("ᠭ\u180c", "ᠥ"),
                            ("ᠪ\u180b", "ᠥ"), ("ᠠ\u180eᠪ", "ᠠ")]:
            for middle in ["", NIRUGU, NIRUGU * 2]:
                plain = left + middle + right
                for run in [ZWJ, ZWJ * 2, ZWJ * 4]:
                    for joined in [left + run + middle + right,
                                   left + middle + run + right,
                                   left + run + middle + run + right]:
                        with self.subTest(joined=joined):
                            self.assertEquivalent(plain, joined)
                            details = [t for t in self.s.shape_detailed(joined)
                                       if t["cp"] != "U+200D"]
                            self.assertEqual(details, self.s.shape_detailed(plain))

    def test_edges_short_words_and_control_only_are_preserved(self):
        for word in ["ᠪᠠ", "ᠰᠠ", "ᠨᠠ", "ᠠ"]:
            for left, right in [(1, 0), (0, 1), (1, 1), (2, 2)]:
                text = ZWJ * left + word + ZWJ * right
                self.assertEqual(self.s.shape(text).count("Zwj"), left + right)
                self.assertFalse(self.s.same_shape(text, word))
                canonical = self.assertEquivalent(text, text)
                self.assertEqual(canonical.count(ZWJ), left + right)
        for text in [ZWJ, ZWJ * 2, NIRUGU + ZWJ, ZWJ + NIRUGU,
                     ZWJ + NIRUGU + ZWJ]:
            self.assertEqual(self.s.normalize(text), text)
        for text in [NIRUGU + ZWJ + "ᠠ", "ᠠ" + ZWJ + NIRUGU]:
            self.assertIn("Zwj", self.s.shape(text))

    def test_mvs_nnbsp_unknown_letters_and_zwnj_are_not_reinterpreted(self):
        for separator in ["\u180e", "\u202f", "\u1843"]:
            for text in ["ᠠ" + separator + ZWJ + "ᠠ", "ᠠ" + ZWJ + separator + "ᠠ"]:
                self.assertIn("Zwj", self.s.shape(text))
        for separator in ["\u180e", "\u202f"]:
            self.assertEquivalent("ᠠ" + separator + "ᠪᠠ", "ᠠ" + separator + "ᠪ" + ZWJ + "ᠠ")
        for fn in [self.s.shape, self.s.normalize]:
            with self.assertRaises(ValueError):
                fn("ᠠ\u200cᠠ")
        for locale in ["TOD", "SIB", "MCH"]:
            self.assertIn("Zwj", MongolianShaper(locale=locale).shape("ᠠ" + ZWJ + "ᠠ"))

    def test_fvs_binding_and_independent_equivalent_spellings(self):
        for fvs in ["\u180b", "\u180c", "\u180d", "\u180f"]:
            self.assertEquivalent("ᠠᠠ", "ᠠ" + ZWJ + fvs + "ᠠ")
            self.assertEquivalent("ᠠᠠ", "ᠠ" + ZWJ + fvs + ZWJ + "ᠠ")
        self.assertNotEqual(self.s.shape("ᠠᠠ"), self.s.shape("ᠠ\u180bᠠ"))
        for a, b in [("ᠪᠠ", "ᠪ" + ZWJ + "ᠡ"), ("ᠰᠠᠢᠨ", "ᠰᠡ" + ZWJ + "ᠢᠨ")]:
            self.assertEquivalent(a, b)

    def test_direct_written_units_preserve_explicit_controls(self):
        units = ["L", "I", "Zwj", "Aa"]
        text = self.s.normalize_written_units(units)
        self.assertEqual(self.s._shape_raw(text), units)
        self.assertIn(ZWJ, text)
        self.assertEqual(self.s.shape(text), ["L", "G"])
        self.assertNotIn(ZWJ, self.s.normalize(text))
