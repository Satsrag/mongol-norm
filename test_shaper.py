#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Tests for MongolianShaper: shape, same_shape, normalize.
MongolianShaper 测试：shape（字形处理）、same_shape（视觉比较）、normalize（规范化）。

The test suite centers on "sain" (ᠰᠠᠢᠨ, meaning "good") because it perfectly
illustrates the encoding ambiguity problem: five different Unicode sequences
all render as the same visual word. If normalization works correctly, all five
must produce the identical canonical output.
测试套件以"sain"（ᠰᠠᠢᠨ，意为"好"）为中心，因为它完美地展示了编码歧义问题：
五种不同的 Unicode 序列都渲染为相同的视觉词。如果规范化正确工作，
所有五种都必须产生相同的规范输出。
"""
import unittest
from shaper import MongolianShaper


class TestShape(unittest.TestCase):
    """
    shape() returns the correct written-unit sequence.
    shape() 返回正确的书写单元序列。

    The key assertion for sain variants: ALL five encodings must produce
    ['S', 'A', 'I', 'I', 'A'] — proving they are visually identical.
    sain 变体的关键断言：所有五种编码都必须产生 ['S', 'A', 'I', 'I', 'A']——
    证明它们在视觉上完全相同。
    """

    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper(locale="MNG")

    # ── sain (good) — 5 encodings of the same word / "好"的5种编码 ──
    # Encoding 1: S + A + I + NA (base form, simplest encoding)
    # 编码1：S + A + I + NA（基本形式，最简编码）
    def test_sain_base(self):
        self.assertEqual(self.s.shape("ᠰᠠᠢᠨ"), ["S", "A", "I", "I", "A"])

    # Encoding 2: S + E + I + NA (E instead of A — same glyph in medial position)
    # 编码2：S + E + I + NA（E 代替 A——中间位置字形相同）
    def test_sain_e_variant(self):
        self.assertEqual(self.s.shape("ᠰᠡᠢᠨ"), ["S", "A", "I", "I", "A"])

    # Encoding 3: S + NA+FVS2 + I + I + NA (NA+FVS2 produces 'A' glyph)
    # 编码3：S + NA+FVS2 + I + I + NA（NA+FVS2 产生 'A' 字形）
    def test_sain_na_fvs2(self):
        self.assertEqual(self.s.shape("ᠰᠨ᠌ᠢᠢᠨ"), ["S", "A", "I", "I", "A"])

    # Encoding 4: S + A + YA+FVS1 + I + NA (YA+FVS1 produces single 'I' tooth)
    # 编码4：S + A + YA+FVS1 + I + NA（YA+FVS1 产生单个 'I' 齿）
    def test_sain_ya_fvs1_i(self):
        self.assertEqual(self.s.shape("ᠰᠠᠶ᠋ᠢᠨ"), ["S", "A", "I", "I", "A"])

    # Encoding 5: S + A + YA+FVS1 + YA+FVS1 + NA (two YA+FVS1 = two teeth)
    # 编码5：S + A + YA+FVS1 + YA+FVS1 + NA（两个 YA+FVS1 = 两个齿）
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
    """
    same_shape() correctly identifies visually identical encodings.
    same_shape() 正确识别视觉上相同的编码。
    """

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
    """
    normalize() returns canonical bare-Unicode encoding.
    normalize() 返回规范的裸 Unicode 编码。

    Two critical properties / 两个关键属性:
      1. Convergence: all visual variants → same canonical form
         收敛性：所有视觉变体 → 相同的规范形式
      2. Idempotence: normalize(normalize(x)) == normalize(x)
         幂等性：normalize(normalize(x)) == normalize(x)
    """

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


class TestNormalizeText(unittest.TestCase):
    """
    normalize_text() normalizes multi-word / mixed-script strings.
    normalize_text() 规范化多词/混合文字字符串。

    Key properties / 关键属性:
      1. Each Mongolian word is normalized independently
         每个蒙古文词独立规范化
      2. Non-Mongolian text (spaces, punctuation, Latin, etc.) is preserved verbatim
         非蒙古文文本（空格、标点、拉丁文等）原样保留
      3. Single-word input produces the same result as normalize()
         单词输入与 normalize() 产生相同结果
    """

    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper(locale="MNG")

    CANONICAL_SAIN = "ᠰᠠᠢᠨ"

    def test_single_word_matches_normalize(self):
        # normalize_text on a single word should match normalize
        # 单词的 normalize_text 应与 normalize 一致
        for word in ["ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ", "ᠨᠠᠢᠮᠠ", "ᠣᠷᠣᠨ"]:
            self.assertEqual(self.s.normalize_text(word), self.s.normalize(word))

    def test_two_words_space_separated(self):
        # Two Mongolian words separated by a space
        # 空格分隔的两个蒙古文词
        text = "ᠰᠡᠢᠨ ᠨᠠᠢᠮᠠ"
        result = self.s.normalize_text(text)
        expected = self.s.normalize("ᠰᠡᠢᠨ") + " " + self.s.normalize("ᠨᠠᠢᠮᠠ")
        self.assertEqual(result, expected)

    def test_space_preserved(self):
        # Spaces must be preserved exactly
        # 空格必须精确保留
        text = "ᠰᠠᠢᠨ  ᠨᠠᠢᠮᠠ"  # double space
        result = self.s.normalize_text(text)
        self.assertIn("  ", result)

    def test_mixed_script(self):
        # Mongolian words surrounded by Latin text
        # 蒙古文词被拉丁文包围
        text = "Hello ᠰᠡᠢᠨ world"
        result = self.s.normalize_text(text)
        self.assertEqual(result, "Hello " + self.CANONICAL_SAIN + " world")

    def test_punctuation_preserved(self):
        # Mongolian punctuation and regular punctuation preserved
        # 蒙古文标点和普通标点保留
        text = "ᠰᠡᠢᠨ, ᠨᠠᠢᠮᠠ!"
        result = self.s.normalize_text(text)
        self.assertIn(",", result)
        self.assertIn("!", result)
        self.assertIn(" ", result)

    def test_empty_string(self):
        self.assertEqual(self.s.normalize_text(""), "")

    def test_no_mongolian(self):
        # Pure non-Mongolian text passes through unchanged
        # 纯非蒙古文文本原样通过
        text = "Hello, world! 123"
        self.assertEqual(self.s.normalize_text(text), text)

    def test_idempotent(self):
        # normalize_text(normalize_text(x)) == normalize_text(x)
        text = "ᠰᠡᠢᠨ ᠨᠠᠢᠮᠠ"
        n1 = self.s.normalize_text(text)
        n2 = self.s.normalize_text(n1)
        self.assertEqual(n1, n2)

    def test_multiword_each_word_independent(self):
        # Each word should be normalized independently — verify by checking
        # that multi-word normalize_text matches word-by-word normalize
        # 每个词应独立规范化——通过检查多词结果与逐词结果一致来验证
        words = ["ᠰᠡᠢᠨ", "ᠣᠷᠣᠨ", "ᠨᠠᠢᠮᠠ"]
        text = " ".join(words)
        result = self.s.normalize_text(text)
        expected = " ".join(self.s.normalize(w) for w in words)
        self.assertEqual(result, expected)


if __name__ == "__main__":
    unittest.main(verbosity=2)
