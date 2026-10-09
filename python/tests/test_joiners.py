# -*- coding: utf-8 -*-
"""
Joiner tokens (`Nirugu` / `Zwj`) in shape and normalize.

Nirugu (U+180A) renders a visible stem-extender glyph and ZWJ (U+200D)
invisibly forces joining. A nirugu always appears VERBATIM in shape() output —
like `Mvs` — because it is the evidence for why a neighbouring letter takes
its init/medi/fina form, and a visible glyph in its own right. A ZWJ appears
only where it changes the ink; a redundant one (between joined letters, next
to a nirugu, doubled, or at a word edge whose joined and unjoined forms are
the same shape) is dropped — see test_redundant_zwj. normalize() preserves
the surviving controls exactly (count and kind) while canonicalizing the
letters between them.
`Nirugu` 与 `Mvs` 同等对待:shape 原样输出、normalize 原样保留;`Zwj` 只在
改变字形时保留,冗余的 ZWJ 被去掉(见 test_redundant_zwj)。
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
N = 'ᠨ'      # n
FVS1 = '᠋'
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
        # A ZWJ whose joined form differs from the unjoined one stays in the shape:
        # N:medi is not N:init.
        self.assertEqual(self.s.shape(ZWJ + N + A), ['Zwj', 'N', 'A'])

    def test_redundant_zwj_is_dropped_from_the_shape(self):
        # `d` joined onward by the ZWJ renders `Dd`, which the public shape spells `O A`;
        # a lone Dd:fina is the same shape as Dd:isol, so the joiner itself is gone.
        self.assertEqual(self.s.shape(ZWJ + D), ['O', 'A'])

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
        # A ZWJ that changes the ink survives normalize verbatim, and the shape round-trips.
        self.assertEqual(self._round_trips(ZWJ + N + A), ZWJ + N + FVS1 + A + FVS2)

    def test_redundant_zwj_is_dropped_by_normalize(self):
        # `ZWJ d` shapes to `O A` (see test_redundant_zwj_is_dropped_from_the_shape), so
        # its canonical spelling is the `O`+`A` pair without any joiner.
        self.assertEqual(self._round_trips(ZWJ + D), U + FVS1 + A + FVS2)

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