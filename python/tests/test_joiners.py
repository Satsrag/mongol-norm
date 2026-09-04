# -*- coding: utf-8 -*-
"""
Joiner tokens (`Nirugu` / `Zwj`) in shape and normalize.

Nirugu (U+180A) renders a visible stem-extender glyph and ZWJ (U+200D)
invisibly forces joining. Both must appear VERBATIM in shape() output —
like `Mvs` — because they are the evidence for why a neighbouring letter
takes its init/medi/fina form, and (for nirugu) a visible glyph in their
own right. normalize() must preserve them exactly (count and kind) while
canonicalizing the letters between them.
`Nirugu`/`Zwj` 与 `Mvs` 同等对待:shape 原样输出、normalize 原样保留,
字母的 init/medi/fina 位置以它们为依据。
"""
import unittest

from mongol_norm import MongolianShaper

NIRUGU = '᠊'
ZWJ = '‍'
O = 'ᠣ'      # o
U = 'ᠤ'      # u
OE = 'ᠥ'     # oe
A = 'ᠠ'      # a
D = 'ᠳ'      # d
J = 'ᠵ'      # j
FVS2 = '᠌'
FVS3 = '᠍'


class _Base(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper(locale='MNG')


class TestJoinerTokensInShape(_Base):
    def test_nirugu_is_a_shape_token(self):
        self.assertEqual(self.s.shape(NIRUGU + O + NIRUGU),
                         ['Nirugu', 'O', 'Nirugu'])

    def test_nirugu_run_count_preserved(self):
        self.assertEqual(self.s.shape(NIRUGU * 2 + O + NIRUGU),
                         ['Nirugu', 'Nirugu', 'O', 'Nirugu'])

    def test_zwj_is_a_shape_token(self):
        # `d` joined onward by the ZWJ renders `Dd`, which the public shape spells `O A`.
        self.assertEqual(self.s.shape(ZWJ + D), ['Zwj', 'O', 'A'])

    def test_nirugu_vs_zwj_shapes_differ(self):
        # visible stem vs invisible joiner — must NOT be conflated
        self.assertFalse(self.s.same_shape(NIRUGU + D, ZWJ + D))

    def test_same_letter_form_between_joiners_still_matches(self):
        # o and u both render written 'O' at medi — same shape
        self.assertTrue(self.s.same_shape(NIRUGU + O + NIRUGU,
                                          NIRUGU + U + NIRUGU))


class TestJoinerNormalize(_Base):
    def _round_trips(self, text):
        norm = self.s.normalize(text)
        self.assertEqual(self.s.shape(norm), self.s.shape(text),
                         f"round-trip broken for {text!r} -> {norm!r}")
        return norm

    def test_nirugu_preserved_letter_canonicalized(self):
        # u at medi renders 'O'; canonical letter for (medi, O) is bare o
        self.assertEqual(self._round_trips(NIRUGU + U + NIRUGU),
                         NIRUGU + O + NIRUGU)

    def test_same_shape_same_canonical_across_encodings(self):
        # oe+fvs3 at medi also renders 'O' (EAC MOZ10-2) — same canonical
        self.assertEqual(self.s.normalize(NIRUGU + OE + FVS3 + NIRUGU),
                         self.s.normalize(NIRUGU + O + NIRUGU))

    def test_nirugu_count_preserved(self):
        text = NIRUGU * 2 + O + NIRUGU
        self.assertEqual(self._round_trips(text), text)

    def test_zwj_preserved(self):
        # The ZWJ survives normalize verbatim, and the shape round-trips. The word itself
        # is no longer a fixed point: its shape is `Zwj O A`, so the canonical spelling is
        # the `O`+`A` pair, not the single `d` that renders the same ink as `Dd`.
        self.assertEqual(self._round_trips(ZWJ + D), ZWJ + O + A + FVS2)

    def test_single_sided_nirugu_round_trips(self):
        for text in (NIRUGU + J,          # joined-left J  -> fina form
                     NIRUGU + D,          # joined-left Dd (shape `Nirugu O A`)
                     U + '᠋' + NIRUGU):  # u+fvs1 joined-right (EAC MVS20-1)
            self._round_trips(text)


class TestJoinerNormalizeText(_Base):
    def test_nirugu_word_uses_the_same_joining_context_as_normalize(self):
        word = NIRUGU + U + NIRUGU
        self.assertEqual(self.s.normalize_text(word), self.s.normalize(word))

    def test_nirugu_word_inside_mixed_text_uses_the_same_joining_context(self):
        word = NIRUGU + U + NIRUGU
        self.assertEqual(self.s.normalize_text('A ' + word + ' B'),
                         'A ' + self.s.normalize(word) + ' B')

    def test_zwj_word_uses_the_same_joining_context_as_normalize(self):
        word = ZWJ + D
        self.assertEqual(self.s.normalize_text(word), self.s.normalize(word))


if __name__ == '__main__':
    unittest.main()