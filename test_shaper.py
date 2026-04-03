#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Tests for MongolianShaper: shape, same_shape, normalize."""
import unittest
from shaper import MongolianShaper


class TestShape(unittest.TestCase):
    """shape() returns the correct written-unit sequence."""

    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper(locale="MNG")

    # ── sain (good) — README examples ───────────────────────────

    def test_sain_base(self):
        self.assertEqual(self.s.shape("ᠰᠠᠢᠨ"), ["S", "A", "I", "I", "A"])

    def test_sain_e_variant(self):
        self.assertEqual(self.s.shape("ᠰᠡᠢᠨ"), ["S", "A", "I", "I", "A"])

    def test_sain_na_fvs2(self):
        self.assertEqual(self.s.shape("ᠰᠨ᠌ᠢᠢᠨ"), ["S", "A", "I", "I", "A"])

    def test_sain_ya_fvs1_i(self):
        self.assertEqual(self.s.shape("ᠰᠠᠶ᠋ᠢᠨ"), ["S", "A", "I", "I", "A"])

    def test_sain_ya_fvs1_ya_fvs1(self):
        self.assertEqual(self.s.shape("ᠰᠠᠶ᠋ᠶ᠋ᠨ"), ["S", "A", "I", "I", "A"])

    # ── vowel harmony ────────────────────────────────────────────

    def test_masculine_a_not_e(self):
        # ᠣᠷᠣᠨ (oron) — masculine word, A-position vowel stays A-form
        shape = self.s.shape("ᠣᠷᠣᠨ")
        self.assertIn("O", shape)

    def test_feminine_post_bowed(self):
        # ᠥᠭᠡ (üge) — feminine: GA after OE takes post-bowed Aa form at final
        self.assertEqual(self.s.shape("ᠥᠭᠡ"), ["A", "O", "I", "G", "Aa"])

    # ── devsger: I after vowel gets double-tooth ─────────────────

    def test_devsger_double_tooth(self):
        # ᠠᠢᠯ (ail) — I in medial position after vowel → double-tooth I,I
        self.assertEqual(self.s.shape("ᠠᠢᠯ"), ["A", "A", "I", "I", "L"])

    def test_devsger_final_no_double(self):
        # ᠠᠢ — I at final position, devsger does not apply
        self.assertEqual(self.s.shape("ᠠᠢ"), ["A", "A", "I"])

    # ── single letter / edge cases ───────────────────────────────

    def test_single_vowel(self):
        self.assertEqual(self.s.shape("ᠠ"), ["A", "A"])

    def test_empty_string(self):
        self.assertEqual(self.s.shape(""), [])


class TestSameShape(unittest.TestCase):
    """same_shape() correctly identifies visually identical encodings."""

    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper(locale="MNG")

    def test_sain_variants_equal(self):
        variants = ["ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ", "ᠰᠨ᠌ᠢᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ", "ᠰᠠᠶ᠋ᠶ᠋ᠨ"]
        for v in variants[1:]:
            self.assertTrue(self.s.same_shape(variants[0], v), f"Expected same shape: {variants[0]} vs {v}")

    def test_different_words_not_equal(self):
        self.assertFalse(self.s.same_shape("ᠰᠠᠢᠨ", "ᠨᠠᠢᠮᠠ"))

    def test_same_string_reflexive(self):
        self.assertTrue(self.s.same_shape("ᠰᠠᠢᠨ", "ᠰᠠᠢᠨ"))


class TestNormalize(unittest.TestCase):
    """normalize() returns canonical bare-Unicode encoding."""

    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper(locale="MNG")

    CANONICAL_SAIN = "ᠰᠠᠢᠨ"

    def test_sain_base(self):
        self.assertEqual(self.s.normalize("ᠰᠠᠢᠨ"), self.CANONICAL_SAIN)

    def test_sain_e_variant(self):
        self.assertEqual(self.s.normalize("ᠰᠡᠢᠨ"), self.CANONICAL_SAIN)

    def test_sain_ya_fvs1(self):
        self.assertEqual(self.s.normalize("ᠰᠠᠶ᠋ᠢᠨ"), self.CANONICAL_SAIN)

    def test_sain_ya_fvs1_ya_fvs1(self):
        self.assertEqual(self.s.normalize("ᠰᠠᠶ᠋ᠶ᠋ᠨ"), self.CANONICAL_SAIN)

    def test_idempotent(self):
        # normalize(normalize(x)) == normalize(x)
        for word in ["ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ", "ᠨᠠᠢᠮᠠ", "ᠣᠷᠣᠨ"]:
            n1 = self.s.normalize(word)
            n2 = self.s.normalize(n1)
            self.assertEqual(n1, n2, f"Not idempotent: {word!r} → {n1!r} → {n2!r}")

    def test_normalized_same_shape_as_original(self):
        for word in ["ᠰᠡᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ", "ᠰᠠᠶ᠋ᠶ᠋ᠨ"]:
            self.assertTrue(self.s.same_shape(word, self.s.normalize(word)))


if __name__ == "__main__":
    unittest.main(verbosity=2)
