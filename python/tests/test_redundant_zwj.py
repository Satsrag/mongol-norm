# -*- coding: utf-8 -*-
"""
Redundant ZWJ (issue #33): a ZWJ that changes no glyph is dropped from the
public shape, so same ink ⟹ same shape() ⟹ same normalize() holds across
joiner spellings. The Rust twin is tests/redundant_zwj.rs.
冗余 ZWJ(#33):不改变字形的 ZWJ 从公开 shape 中去掉,保证同形 ⟹ 同 shape ⟹ 同 normalize。
"""
import unittest

from mongol_norm import MongolianShaper

ZWJ = '‍'
NIRUGU = '᠊'
MVS = '᠎'
FVS1 = '᠋'
A = 'ᠠ'
E = 'ᠡ'
I = 'ᠢ'
O = 'ᠣ'
U = 'ᠤ'
UE = 'ᠦ'
N = 'ᠨ'
B = 'ᠪ'
P = 'ᠫ'
L = 'ᠯ'
S = 'ᠰ'
T = 'ᠲ'
D = 'ᠳ'
R = 'ᠷ'


class _Base(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper(locale='MNG')

    def assert_same_word(self, with_zwj, without):
        """Both spellings shape and normalize identically; the canonical text round-trips."""
        self.assertEqual(self.s.shape(with_zwj), self.s.shape(without))
        self.assertTrue(self.s.same_shape(with_zwj, without))
        norm = self.s.normalize(with_zwj)
        self.assertEqual(norm, self.s.normalize(without))
        self.assertEqual(self.s.shape(norm), self.s.shape(with_zwj))
        self.assertEqual(self.s.normalize(norm), norm)

    def assert_zwj_kept(self, with_zwj, without):
        """The ZWJ matters: shapes differ, `Zwj` survives, normalize keeps the joiner."""
        self.assertNotEqual(self.s.shape(with_zwj), self.s.shape(without))
        self.assertIn('Zwj', self.s.shape(with_zwj))
        norm = self.s.normalize(with_zwj)
        self.assertIn(ZWJ, norm)
        self.assertEqual(self.s.shape(norm), self.s.shape(with_zwj))


class TestSegmentInterior(_Base):
    def test_interior_zwj_between_joined_letters_is_dropped(self):
        self.assert_same_word(L + I + ZWJ + E + FVS1, L + I + E + FVS1)
        self.assertEqual(self.s.shape(L + I + ZWJ + E + FVS1), ['L', 'G'])
        self.assert_same_word(A + R + A + ZWJ + D, A + R + A + D)
        self.assert_same_word(L + A + L + A + ZWJ + P + UE + D, L + A + L + A + P + UE + D)

    def test_zwj_next_to_a_nirugu_is_dropped(self):
        self.assert_same_word(A + L + ZWJ + NIRUGU, A + L + NIRUGU)
        self.assert_same_word(NIRUGU + ZWJ + L + A, NIRUGU + L + A)
        self.assert_same_word(A + L + NIRUGU + ZWJ, A + L + NIRUGU)
        self.assert_same_word(ZWJ + NIRUGU + U, NIRUGU + U)
        self.assert_same_word(A + L + NIRUGU + ZWJ + MVS + A, A + L + NIRUGU + MVS + A)

    def test_repeated_zwj_keeps_one(self):
        self.assert_same_word(ZWJ + ZWJ + N + A, ZWJ + N + A)
        self.assert_same_word(A + ZWJ + ZWJ, A + ZWJ)
        self.assert_same_word(L + I + ZWJ + ZWJ + E + FVS1, L + I + E + FVS1)
        self.assertEqual(self.s.shape(ZWJ + ZWJ), ['Zwj'])

    def test_zwj_next_to_mvs_is_kept(self):
        self.assert_zwj_kept(A + L + ZWJ + MVS + A, A + L + MVS + A)
        self.assert_zwj_kept(A + MVS + ZWJ + N + A, A + MVS + N + A)
        self.assertEqual(self.s.shape(A + L + ZWJ + MVS + A),
                         ['A', 'A', 'L', 'Zwj', 'Mvs', 'Aa'])


class TestWordEdges(_Base):
    def test_word_initial_zwj_follows_the_reviewed_table(self):
        # B:medi ≡ B:init, S:medi ≡ S:init (accepted near-equivalent); N differs.
        self.assert_same_word(ZWJ + B + A, B + A)
        self.assertEqual(self.s.shape(ZWJ + B + A), ['B', 'Aa'])
        self.assert_same_word(ZWJ + S + A, S + A)
        self.assert_same_word(ZWJ + D + A, D + A)
        self.assert_same_word(ZWJ + B + A + R, B + A + R)
        self.assert_zwj_kept(ZWJ + N + A, N + A)
        self.assert_zwj_kept(ZWJ + L + A, L + A)
        self.assert_zwj_kept(ZWJ + A + A, A + A)

    def test_word_initial_zwj_before_a_lone_letter(self):
        self.assert_same_word(ZWJ + I, I + FVS1)
        self.assert_same_word(ZWJ + U, U + FVS1)
        self.assertEqual(self.s.shape(ZWJ + D), ['O', 'A'])
        self.assert_zwj_kept(ZWJ + B, B)
        self.assert_zwj_kept(ZWJ + A, A)

    def test_word_final_zwj_follows_the_reviewed_table(self):
        self.assert_same_word(A + O + ZWJ, A + O + FVS1)
        self.assert_zwj_kept(A + L + ZWJ, A + L)
        self.assert_zwj_kept(A + ZWJ, A)
        self.assertEqual(self.s.shape(A + ZWJ), ['A', 'A', 'Zwj'])

    def test_word_final_zwj_after_a_lone_consonant_is_dropped(self):
        for letter in (B, L, N, S, T):
            with self.subTest(letter=letter):
                self.assert_same_word(letter + ZWJ, letter)
                self.assertNotIn('Zwj', self.s.shape(letter + ZWJ))
        self.assertEqual(self.s.normalize(B + ZWJ), B)

    def test_both_edges_resolve_to_a_fixed_point(self):
        self.assert_same_word(ZWJ + B + ZWJ, B)
        self.assertEqual(self.s.shape(ZWJ + B + ZWJ), ['B'])
        self.assertEqual(self.s.shape(ZWJ + A + ZWJ), ['Zwj', 'A', 'Zwj'])


class TestWrittenUnitApis(_Base):
    def test_normalize_written_units_folds_redundant_zwj_like_shape(self):
        cases = [
            (['L', 'I', 'Zwj', 'Aa'], ['L', 'G']),
            (['Zwj', 'B', 'Aa'], ['B', 'Aa']),
            (['B', 'Zwj'], ['B']),
            (['Zwj', 'B', 'Zwj'], ['B']),
            (['Nirugu', 'Zwj', 'L', 'A'], ['Nirugu', 'L', 'A']),
            (['Zwj', 'Zwj', 'N', 'A'], ['Zwj', 'N', 'A']),
        ]
        for spelled, folded in cases:
            with self.subTest(spelled=spelled):
                text = self.s.normalize_written_units(spelled)
                self.assertEqual(text, self.s.normalize_written_units(folded))
                self.assertEqual(self.s.shape(text), folded)
        self.assertEqual(self.s.normalize_written_units(['L', 'I', 'Zwj', 'Aa']),
                         self.s.normalize(L + I + ZWJ + E + FVS1))

    def test_shape_raw_and_diagnostics_keep_every_zwj(self):
        self.assertEqual(self.s._shape_raw(L + I + ZWJ + E + FVS1), ['L', 'I', 'Zwj', 'Aa'])
        self.assertEqual(self.s._shape_raw(ZWJ + D), ['Zwj', 'Dd'])
        self.assertEqual(len(self.s.shape_detailed(ZWJ + D)), 2)
        trace = self.s.trace(ZWJ + D)
        self.assertEqual(len(trace['positions']), 2)
        self.assertEqual(trace['shape'], self.s.shape(ZWJ + D))

    def test_edge_equivalences_are_hudum_only(self):
        # (`n` is a letter in every locale; Todo has no `b`.)
        for locale in ('TOD', 'SIB', 'MCH'):
            with self.subTest(locale=locale):
                shaper = MongolianShaper(locale=locale)
                self.assertIn('Zwj', shaper.shape(N + ZWJ))
                self.assertNotIn('Zwj', shaper.shape(N + A + ZWJ + N + A))


if __name__ == '__main__':
    unittest.main()
