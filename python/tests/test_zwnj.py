# -*- coding: utf-8 -*-
"""
ZWNJ (U+200C) as an in-word joining break (issue #34): the letter before it is
written fina/isol, the letter after it init/isol, the context rules look through
it as the reference font does (Kushim-Jiang/mongolian#58), and the public shape
keeps the break as a Zwnj unit. The Rust twin is tests/zwnj.rs; the table there
is the one of the pull request and was cross-checked against HarfBuzz with the
bundled hudum.otf.
ZWNJ(U+200C,#34):打断连写但上下文规则穿过它;公开 shape 保留每个 ZWNJ。
"""
import random
import unittest

from mongol_norm import MongolianShaper, NormalizationFallbackError

_ALIAS_TO_CP = {
    'a': 0x1820, 'e': 0x1821, 'i': 0x1822, 'o': 0x1823, 'u': 0x1824, 'oe': 0x1825,
    'ue': 0x1826, 'ee': 0x1827, 'n': 0x1828, 'ng': 0x1829, 'b': 0x182A, 'p': 0x182B,
    'h': 0x182C, 'g': 0x182D, 'm': 0x182E, 'l': 0x182F, 's': 0x1830, 'sh': 0x1831,
    't': 0x1832, 'd': 0x1833, 'ch': 0x1834, 'j': 0x1835, 'y': 0x1836, 'r': 0x1837,
    'w': 0x1838, 'f': 0x1839, 'k2': 0x183A, 'k': 0x183B, 'c': 0x183C, 'z': 0x183D,
    'hh': 0x183E, 'rh': 0x183F, 'lh': 0x1840, 'zr': 0x1841, 'cr': 0x1842,
    'mvs': 0x180E, 'fvs1': 0x180B, 'fvs2': 0x180C, 'fvs3': 0x180D, 'fvs4': 0x180F,
    'nnbsp': 0x202F, 'nirugu': 0x180A, 'zwj': 0x200D, 'zwnj': 0x200C,
}
ZWNJ = '\u200C'


def _mgl(aliases):
    return ''.join(chr(_ALIAS_TO_CP[alias]) for alias in aliases.split())


def _hex(text):
    return ' '.join(f'{ord(c):04X}' for c in text)


# (aliases, shape with the ZWNJ(s), shape without them)
TABLE = [
    # word edges
    ('zwnj b a', 'Zwnj+B+Aa', 'B+Aa'),
    ('b a zwnj', 'B+Aa+Zwnj', 'B+Aa'),
    ('zwnj n a', 'Zwnj+N+A', 'N+A'),
    ('n a zwnj', 'N+A+Zwnj', 'N+A'),
    ('zwnj a', 'Zwnj+A+A', 'A+A'),
    ('a zwnj', 'A+A+Zwnj', 'A+A'),
    # control-only input
    ('zwnj', 'Zwnj', ''),
    ('zwnj zwnj', 'Zwnj+Zwnj', ''),
    ('zwnj fvs1', 'Zwnj', ''),
    ('zwnj mvs', 'Zwnj+Mvs', 'Mvs'),
    ('mvs zwnj', 'Mvs+Zwnj', 'Mvs'),
    ('zwj zwnj', 'Zwnj', 'Zwj'),
    ('zwnj zwj', 'Zwnj', 'Zwj'),
    # repeated breaks
    ('a zwnj zwnj b', 'A+A+Zwnj+Zwnj+B', 'A+A+B'),
    ('zwnj b zwnj', 'Zwnj+B+Zwnj', 'B'),
    # between letters
    ('b zwnj b a', 'B+Zwnj+B+Aa', 'B+B+Aa'),
    ('a o zwnj b a', 'A+A+U+Zwnj+B+Aa', 'A+A+O+B+Aa'),
    ('b zwnj a', 'B+Zwnj+A+A', 'B+Aa'),
    ('n zwnj a', 'N+Zwnj+A+A', 'N+A'),
    ('a zwnj a', 'A+A+Zwnj+A+A', 'A+A+A'),
    ('a b zwnj b a', 'A+A+B+Zwnj+B+Aa', 'A+A+B+B+Aa'),
    ('l i zwnj e fvs1', 'L+I+Zwnj+Aa', 'L+G'),
    ('s zwnj g', 'S+Zwnj+Hx', 'S+G'),
    ('b zwnj s', 'B+Zwnj+S', 'B+S'),
    ('t a l zwnj a', 'T+A+L+Zwnj+A+A', 'T+A+L+A'),
    ('a zwnj b zwnj a', 'A+A+Zwnj+B+Zwnj+A+A', 'A+A+B+Aa'),
    ('a zwnj n zwnj a', 'A+A+Zwnj+N+Zwnj+A+A', 'A+A+N+A'),
    # beside an MVS / NNBSP
    ('mvs zwnj a', 'Mvs+Zwnj+Aa', 'Mvs+Aa'),
    ('zwnj mvs a', 'Zwnj+Mvs+Aa', 'Mvs+Aa'),
    ('a zwnj mvs a', 'A+A+Zwnj+Mvs+Aa', 'A+A+Mvs+Aa'),
    ('a zwnj nnbsp a', 'A+A+Zwnj+Mvs+Aa', 'A+A+Mvs+Aa'),
    ('mvs zwnj i', 'Mvs+Zwnj+I', 'Mvs+I'),
    ('mvs zwnj u n', 'Mvs+Zwnj+O+A', 'Mvs+O+A'),
    ('mvs zwnj y i n', 'Mvs+Zwnj+I+I+A', 'Mvs+I+I+A'),
    ('n zwnj mvs a', 'N+Zwnj+Mvs+Aa', 'N+Mvs+Aa'),
    ('a n zwnj mvs a', 'A+A+N+Zwnj+Mvs+Aa', 'A+A+N+Mvs+Aa'),
    ('s g zwnj mvs a', 'S+H+Zwnj+Mvs+Aa', 'S+H+Mvs+Aa'),
    ('g zwnj mvs a', 'Hx+Zwnj+Mvs+Aa', 'Hx+Mvs+Aa'),
    ('t a l mvs zwnj a', 'T+A+L+Mvs+Zwnj+Aa', 'T+A+L+Mvs+Aa'),
    # a break inside a particle ends it
    ('mvs i zwnj y a r', 'Mvs+I+Zwnj+Y+A+R', 'Mvs+I+I+A+R'),
    ('mvs y zwnj i n', 'Mvs+Y+Zwnj+A+I+A', 'Mvs+I+I+A'),
    ('mvs u zwnj u', 'Mvs+U+Zwnj+A+O', 'Mvs+O+U'),
    ('a mvs d u zwnj r', 'A+A+Mvs+D+U+Zwnj+R', 'A+A+Mvs+D+O+R'),
    # beside a nirugu
    ('n zwnj nirugu', 'N+Zwnj+Nirugu', 'N+Nirugu'),
    ('zwnj nirugu i e fvs1', 'Zwnj+Nirugu+G', 'Nirugu+G'),
    ('a zwnj nirugu b', 'A+A+Zwnj+Nirugu+B', 'A+A+Nirugu+B'),
    ('a nirugu zwnj b', 'A+A+Nirugu+Zwnj+B', 'A+A+Nirugu+B'),
    ('nirugu zwnj b', 'Nirugu+Zwnj+B', 'Nirugu+B'),
    # beside a ZWJ, both orders
    ('b zwnj zwj a', 'B+Zwnj+Aa', 'B+Aa'),
    ('n zwnj zwj a', 'N+Zwnj+Zwj+A', 'N+A'),
    ('zwj zwnj n a', 'Zwnj+N+A', 'Zwj+N+A'),
    ('n a zwnj zwj', 'N+A+Zwnj', 'N+A+Zwj'),
    ('a zwj zwnj b a', 'A+A+Zwj+Zwnj+B+Aa', 'A+A+B+Aa'),
    ('a zwj zwnj b', 'A+A+Zwj+Zwnj+B', 'A+A+B'),
    ('a zwnj zwj b', 'A+A+Zwnj+Zwj+B', 'A+A+B'),
    ('b zwj zwnj a', 'B+Zwnj+A+A', 'B+Aa'),
    # an FVS after a break selects nothing (it is the break's); one before it is the letter's
    ('a zwnj fvs1 a', 'A+A+Zwnj+A+A', 'A+A'),
    ('a fvs1 zwnj a', 'A+Zwnj+A+A', 'A+A'),
    ('b a fvs1 zwnj', 'B+Aa+Zwnj', 'B+Aa'),
    # gender and harmony cross the break
    ('n ue zwnj h nirugu', 'N+Ue+Zwnj+G+Nirugu', 'N+O+I+G+Nirugu'),
    ('h zwnj oe', 'H+Zwnj+A+Ue', 'G+Ue'),
    ('g zwnj e', 'Hx+Zwnj+A', 'G+Aa'),
    ('a zwnj g', 'A+A+Zwnj+Hx', 'A+A+H'),
    ('a l zwnj g', 'A+A+L+Zwnj+Hx', 'A+A+L+G'),
    ('a b zwnj g', 'A+A+B+Zwnj+Hx', 'A+A+B+G'),
    ('i g zwnj b a', 'A+I+H+Zwnj+B+Aa', 'A+I+A+A+B+Aa'),
    ('i g zwnj a', 'A+I+G+Zwnj+A+A', 'A+I+N+N+A'),
    ('e b zwnj i g', 'A+B+Zwnj+A+I+G', 'A+B+I+G'),
    ('s g zwnj a', 'S+G+Zwnj+A+A', 'S+N+N+A'),
    ('m n oe zwnj g e', 'M+N+U+Zwnj+G+Aa', 'M+N+O+I+G+Aa'),
    # the issue's corpus
    ('l a l a zwj zwnj p ue d', 'L+A+L+A+Zwj+Zwnj+P+O+I+O+A', 'L+A+L+A+P+O+O+A'),
    ('t a ng zwj zwnj p u r u m', 'T+A+A+G+Zwj+Zwnj+P+O+R+O+M', 'T+A+A+G+P+O+R+O+M'),
    ('t i l zwj zwnj p u r u g', 'T+I+L+Zwj+Zwnj+P+O+R+O+H', 'T+I+L+P+O+R+O+H'),
    ('s e ng zwj zwnj p u r u m', 'S+A+A+G+Zwj+Zwnj+P+O+R+O+M', 'S+A+A+G+P+O+R+O+M'),
    ('l a l a zwnj p ue d', 'L+A+L+A+Zwnj+P+O+I+O+A', 'L+A+L+A+P+O+O+A'),
    ('t a ng zwnj p u r u m', 'T+A+A+G+Zwnj+P+O+R+O+M', 'T+A+A+G+P+O+R+O+M'),
    ('t i l zwnj p u r u g', 'T+I+L+Zwnj+P+O+R+O+H', 'T+I+L+P+O+R+O+H'),
]


class _Base(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper(locale='MNG')

    def assert_normalization(self, text):
        units = self.s.shape(text)
        normalized = self.s.normalize(text)
        self.assertEqual(self.s.shape(normalized), units)
        self.assertEqual(normalized.count(ZWNJ), text.count(ZWNJ))
        self.assertEqual(self.s.normalize(normalized), normalized)
        self.assertEqual(self.s.normalize_text(text), normalized)
        self.assertEqual(self.s.normalize(text, strict=False), normalized)
        self.assertEqual(self.s.normalize_written_units(units), normalized)
        targets = [[]]
        for unit in units:
            if unit == 'Zwnj':
                targets.append([])
            else:
                targets[-1].append(unit)
        segments = normalized.split(ZWNJ)
        self.assertEqual(len(segments), len(targets))
        for segment, target in zip(segments, targets):
            self.assertEqual(self.s.shape(segment), target)
            isolated = self.s.normalize_written_units(target)
            self.assertEqual(self.s.shape(isolated), target)
            # Isolated Mvs+Aa keeps ordinary bare a; the complete target may
            # select a+FVS2 so a J/W across the break stays J/W (#62).
            if isolated != segment:
                short, long = (isolated, segment) if len(isolated) < len(segment) else (segment, isolated)
                self.assertEqual(long, short + '\u180c')
                self.assertIn('\u180e', short)
                self.assertTrue(short.endswith('\u1820'))
        return normalized

    def assert_table_gap(self, text):
        with self.assertRaises(NormalizationFallbackError) as raised:
            self.s.normalize(text)
        self.assertEqual(raised.exception.text, text)
        self.assertEqual(raised.exception.written_units, tuple(self.s.shape(text)))
        self.assertEqual(self.s.normalize(text, strict=False), text)
        with self.assertRaisesRegex(ValueError, 'no canonical MNG encoding'):
            self.s.normalize_written_units(self.s.shape(text))
        mixed = f'English {text}, 中文'
        with self.assertRaises(NormalizationFallbackError) as raised:
            self.s.normalize_text(mixed)
        self.assertEqual(raised.exception.text, text)
        self.assertEqual(self.s.normalize_text(mixed, strict=False), mixed)


class TestEveryBreakStays(_Base):
    def test_table(self):
        for aliases, with_zwnj, without in TABLE:
            with self.subTest(aliases=aliases):
                text = _mgl(aliases)
                plain = text.replace(ZWNJ, '')
                breaks = text.count(ZWNJ)
                self.assertEqual(self.s.shape_str(text), with_zwnj)
                self.assertEqual(self.s.shape_str(plain), without)
                self.assertEqual(with_zwnj.split('+').count('Zwnj'), breaks)
                self.assertEqual(self.s._shape_raw(text).count('Zwnj'), text.count(ZWNJ))
                self.assertEqual(self.s.trace(text)['shape'], self.s.shape(text))
                self.assert_normalization(text)


class TestIssueCorpus(_Base):
    def test_first_word_keeps_its_marked_ue(self):
        word = '\u182F\u1820\u182F\u1820\u200D\u200C\u182B\u1826\u1833'
        self.assertEqual(self.s._shape_raw(word),
                         ['L', 'A', 'L', 'A', 'Zwj', 'Zwnj', 'P', 'O', 'I', 'Dd'])
        self.assertEqual(self.s.shape(word),
                         ['L', 'A', 'L', 'A', 'Zwj', 'Zwnj', 'P', 'O', 'I', 'O', 'A'])

    def test_other_words(self):
        for word, shape in [
            ('\u1832\u1820\u1829\u200D\u200C\u182B\u1824\u1837\u1824\u182E',
             'T+A+A+G+Zwj+Zwnj+P+O+R+O+M'),
            ('\u1832\u1822\u182F\u200D\u200C\u182B\u1824\u1837\u1824\u182D',
             'T+I+L+Zwj+Zwnj+P+O+R+O+H'),
            ('\u1830\u1821\u1829\u200D\u200C\u182B\u1824\u1837\u1824\u182E',
             'S+A+A+G+Zwj+Zwnj+P+O+R+O+M'),
        ]:
            with self.subTest(word=_hex(word)):
                self.assertEqual(self.s.shape_str(word), shape)


class TestContextCrossesTheBreak(_Base):
    def test_gender_witness(self):
        witness = '\u1828\u1826\u200C\u182C\u180A'
        details = self.s.shape_detailed(witness)
        self.assertEqual([d['position'] for d in details],
                         ['init', 'fina', 'isol', 'init', 'isol'])
        self.assertEqual(details[2]['alias'], 'zwnj')
        self.assertEqual(details[1]['written'], ['Ue'])
        self.assertEqual(details[3]['written'], ['G'])  # feminine from across the break
        self.assertEqual(self.s.shape_str(witness), 'N+Ue+Zwnj+G+Nirugu')
        self.assertEqual(self.s.shape_str('\u1828\u1826\u182C\u180A'), 'N+O+I+G+Nirugu')

    def test_positions_split_at_the_break(self):
        positions = lambda aliases: [d['position'] for d in self.s.shape_detailed(_mgl(aliases))]
        self.assertEqual(positions('t a l zwnj a'), ['init', 'medi', 'fina', 'isol', 'isol'])
        self.assertEqual(positions('a zwj zwnj b'), ['init', 'isol', 'isol', 'isol'])
        self.assertEqual(positions('a zwnj zwj b'), ['isol', 'isol', 'isol', 'fina'])

    def test_fvs_after_a_break_is_the_breaks(self):
        details = self.s.shape_detailed(_mgl('a zwnj fvs1 a'))
        self.assertEqual(len(details), 3)
        self.assertEqual([d['fvs'] for d in details], ['', '+FVS1', ''])
        self.assertTrue(self.s.same_shape(_mgl('a zwnj fvs1 a'), _mgl('a zwnj a')))
        self.assertEqual(self.s.shape_detailed(_mgl('a fvs1 zwnj a'))[0]['fvs'], '+FVS1')
        # GB.A skips the break and resets the marked o, as in the font.
        self.assertEqual(self.s.shape_str(_mgl('t o zwnj')), 'T+O+Zwnj')
        self.assertEqual(self.s.shape_str(_mgl('t o zwnj fvs3')), 'T+U+Zwnj')
        self.assertEqual(self.s.shape_str(_mgl('sh ue zwnj fvs3 nnbsp o')), 'Sh+U+Zwnj+Mvs+A+O')


class TestFollowsTheFont(_Base):
    """Where a ZWNJ meets an FVS, a joiner, a particle or an MVS, the engine shapes as the
    reference font does (the Rust twin is FONT_CASES in tests/zwnj.rs)."""
    CASES = [
        # An FVS a ZWNJ carries counts for the FVS-reading lookups, which skip the ZWNJ.
        ('t o zwnj fvs3', 'T+U+Zwnj'),
        ('t o zwnj', 'T+O+Zwnj'),
        ('d e zwnj fvs3', 'T+A+Zwnj'),
        ('d e zwnj', 'D+A+Zwnj'),
        ('a j nnbsp a zwnj fvs4', 'A+A+J+Mvs+Aa+Zwnj'),
        ('a g mvs zwnj fvs1 a zwnj fvs3', 'A+A+Hx+Mvs+Zwnj+Aa+Zwnj'),
        ('a n mvs zwnj fvs1 a zwnj fvs3', 'A+A+N+Mvs+Zwnj+Aa+Zwnj'),
        ('a j nnbsp a zwnj', 'A+A+I+Mvs+Aa+Zwnj'),
        ('nnbsp ue zwnj fvs3', 'Mvs+A+Ue+Zwnj'),
        ('nnbsp ue zwnj', 'Mvs+U+Zwnj'),
        ('mvs y i n zwnj fvs1', 'Mvs+Y+I+A+Zwnj'),
        ('mvs y i n zwnj', 'Mvs+I+I+A+Zwnj'),
        ('mvs i zwnj zwj fvs2', 'Mvs+A+I+Zwnj'),
        # A particle follows the joining chain: two ZWJs join across a ZWNJ.
        ('u zwnj zwj u', 'A+O+Zwnj+U'),
        ('a r nirugu zwnj ue ue', 'A+A+R+Nirugu+Zwnj+O+U'),
        # post_bowed reads a final or isolated consonant before its FVS applies.
        ('g fvs2 zwnj zwj e', 'G+Zwnj+Zwj+A'),
        ('a b fvs1 zwnj zwj e', 'A+A+B2+Zwnj+Aa'),
        ('a g fvs1 zwnj zwj e', 'A+A+H+Zwnj+Zwj+A'),
        ('a g fvs2 zwnj zwj o', 'A+A+G+Zwnj+Zwj+O'),
        ('t g fvs2 zwnj nirugu o', 'T+G+Zwnj+Nirugu+O'),
        # The lookups that need an MVS beside a letter look through a run that holds a ZWNJ.
        ('a n mvs zwj zwnj a', 'A+A+N+Mvs+Zwnj+Aa'),
        ('a n mvs zwnj zwj a', 'A+A+A+Mvs+Zwnj+Zwj+A'),
        ('e g mvs zwj zwnj e', 'A+H+Mvs+Zwnj+Aa'),
        ('a y a g mvs zwj zwnj a', 'A+A+Y+A+Hx+Mvs+Zwnj+Aa'),
        ('i g sh i n fvs2 mvs zwnj fvs2 zwnj zwnj a', 'A+I+A+A+S+I+A+Mvs+Zwnj+Zwnj+Zwnj+Aa'),
    ]

    def test_cases(self):
        for aliases, expected in self.CASES:
            with self.subTest(aliases=aliases):
                self.assertEqual(self.s.shape_str(_mgl(aliases)), expected)
                self.assert_normalization(_mgl(aliases))


class TestConfluence(_Base):
    GROUPS = [
        # Spellings that differ only in what the shape folds away: a ZWJ beside the
        # break with nothing to join, an FVS the break carries, an NNBSP for the MVS.
        ['n a zwnj', 'n a zwnj zwj', 'n a zwnj fvs1'],
        ['zwnj n a', 'zwj zwnj n a'],
        ['a zwnj a', 'a zwnj fvs1 a'],
        ['mvs zwnj a', 'nnbsp zwnj a'],
    ]

    def test_equivalent_spellings_converge(self):
        for group in self.GROUPS:
            first = _mgl(group[0])
            for aliases in group[1:]:
                with self.subTest(aliases=aliases, reference=group[0]):
                    text = _mgl(aliases)
                    self.assertEqual(self.s.shape(text), self.s.shape(first))
                    self.assertEqual(self.s.normalize(text), self.s.normalize(first))


class TestApis(_Base):
    def test_parsed_shape_is_encoded_independently_across_breaks(self):
        self.assertEqual(self.s.normalize_text('a\u200Cb'), 'a\u200Cb')
        word = _mgl('n ue zwnj h nirugu')
        normalized = self.assert_normalization(word)
        left, right = normalized.split(ZWNJ)
        self.assertEqual(self.s.shape(left), ['N', 'Ue'])
        self.assertEqual(self.s.shape(right), ['G', 'Nirugu'])
        split = ZWNJ.join(self.s.normalize(part) for part in word.split(ZWNJ))
        self.assertNotEqual(normalized, split)
        mixed = f'Hello {word}, 中文 a\u200Cb!\n{word}'
        expected = f'Hello {normalized}, 中文 a\u200Cb!\n{normalized}'
        self.assertEqual(self.s.normalize_text(mixed), expected)
        self.assertEqual(self.s.normalize_text(mixed, strict=False), expected)

    def test_zwnj_segments_use_the_standalone_mvs_suffix_encoding(self):
        encoded_suffix = self.s.normalize_written_units(['I', 'I', 'A'])
        mvs = self.assert_normalization(_mgl('mvs y i n'))
        self.assertEqual(mvs, '\u180E' + encoded_suffix)
        for aliases in ['mvs zwnj y i n', 'mvs zwnj zwnj y i n']:
            normalized = self.assert_normalization(_mgl(aliases))
            self.assertEqual(normalized.rsplit(ZWNJ, 1)[1], encoded_suffix)

    def test_unencodable_segment_never_returns_a_partial_word(self):
        with self.assertRaisesRegex(ValueError, 'no canonical MNG encoding'):
            self.s.normalize_written_units(['O'])
        for units in [['B', 'Zwnj', 'O'], ['O', 'Zwnj', 'B']]:
            with self.subTest(units=units):
                with self.assertRaisesRegex(ValueError, 'no canonical MNG encoding'):
                    self.s.normalize_written_units(units)

    def test_suffix_before_a_break_does_not_depend_on_the_following_segment(self):
        text = _mgl('nnbsp u t zwj fvs1 zwnj zwnj r')
        normalized = self.assert_normalization(text)
        # The old encoding depended on the following segment to keep T instead of D.
        self.assertEqual(self.s.shape_str(normalized.split(ZWNJ)[0]), 'Mvs+A+O+T+Zwj')

    def test_positioned_breaks_and_validation_order(self):
        control = {'unit': 'Zwnj', 'position': 'control'}
        self.assertEqual(self.s.normalize_positioned_written_units([control] * 2), ZWNJ * 2)
        records = [control, {'unit': 'B', 'position': 'init'},
                   {'unit': 'Aa', 'position': 'fina'}, control, control,
                   {'unit': 'N', 'position': 'medi'}, control]
        units = ['Zwnj', 'B', 'Aa', 'Zwnj', 'Zwnj', 'Zwj', 'N', 'Zwj', 'Zwnj']
        self.assertEqual(self.s.normalize_positioned_written_units(records),
                         self.s.normalize_written_units(units))
        bad = {'unit': 'Zwnj', 'position': 'isol'}
        with self.assertRaisesRegex(ValueError, r"positioned_units\[1\] control 'Zwnj' requires position 'control'"):
            self.s.normalize_positioned_written_units([records[1], bad])
        with self.assertRaisesRegex(ValueError, "unsupported positioned control 'Zwj'"):
            self.s.normalize_positioned_written_units([bad, {'unit': 'Zwj', 'position': 'control'}])
        with self.assertRaisesRegex(ValueError, 'at most 1024'):
            self.s.normalize_positioned_written_units([bad] * 1025)

    def test_fvs_on_control_only_input(self):
        for fvs in ['fvs1', 'fvs2', 'fvs3', 'fvs4']:
            self.assertEqual(self.assert_normalization(_mgl(f'zwnj {fvs} zwnj')), ZWNJ * 2)

    def test_error_message(self):
        with self.assertRaisesRegex(ValueError, 'FVS/MVS/NNBSP/Nirugu/ZWJ/ZWNJ'):
            self.s.shape('\u1820x')


class TestFuzz(_Base):
    ALPHABET = [chr(cp) for cp in range(0x1820, 0x1843)] + [
        '\u180B', '\u180C', '\u180D', '\u180E', '\u202F', '\u180A', '\u200D',
        ZWNJ, ZWNJ, ZWNJ, ZWNJ,
    ]

    def test_normalization_and_every_break_with_and_without_fvs4(self):
        for alphabet in [self.ALPHABET, self.ALPHABET + ['\u180F']]:
            rng = random.Random(34)
            normalized_count = 0
            by_shape = {}
            for _ in range(20_000):
                text = ''.join(rng.choice(alphabet) for _ in range(rng.randint(1, 10)))
                with self.subTest(text=_hex(text)):
                    shape = self.s.shape(text)
                    self.assertEqual(self.s._shape_raw(text).count('Zwnj'), text.count(ZWNJ))
                    self.assertEqual(shape.count('Zwnj'), text.count(ZWNJ))
                    self.assertEqual(self.s.trace(text)['shape'], shape)
                    try:
                        self.s.normalize(text)
                    except NormalizationFallbackError:
                        self.assert_table_gap(text)
                    else:
                        normalized_count += 1
                        normalized = self.assert_normalization(text)
                        previous = by_shape.setdefault(tuple(shape), normalized)
                        self.assertEqual(previous, normalized)
            self.assertGreater(normalized_count, 18_000)


if __name__ == '__main__':
    unittest.main()
