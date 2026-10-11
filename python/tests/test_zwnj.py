# -*- coding: utf-8 -*-
"""
ZWNJ (U+200C) as an in-word joining break (issue #34): the letter before it is
written fina/isol, the letter after it init/isol, the context rules look through
it as the reference font does (Kushim-Jiang/mongolian#58), and a break that
changes no glyph is dropped from the public shape by the same written-unit
machinery as a redundant ZWJ. The Rust twin is tests/zwnj.rs; the table there
is the one of the pull request and was cross-checked against HarfBuzz with the
bundled hudum.otf.
ZWNJ(U+200C,#34):打断连写但上下文规则穿过它;不改变字形的 ZWNJ 从公开 shape 中去掉。
"""
import random
import unittest

from mongol_norm import MongolianShaper

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


# (aliases, shape with the ZWNJ(s), shape without them, canonical code points)
TABLE = [
    # word edges
    ('zwnj b a', 'B+Aa', 'B+Aa', '182A 1820 180B'),
    ('b a zwnj', 'B+Aa', 'B+Aa', '182A 1820 180B'),
    ('zwnj n a', 'N+A', 'N+A', '1828 1820 180C'),
    ('n a zwnj', 'N+A', 'N+A', '1828 1820 180C'),
    ('zwnj a', 'A+A', 'A+A', '1820 180B 1820 180C'),
    ('a zwnj', 'A+A', 'A+A', '1820 180B 1820 180C'),
    # control-only input
    ('zwnj', 'Zwnj', '', '200C'),
    ('zwnj zwnj', 'Zwnj', '', '200C'),
    ('zwnj fvs1', 'Zwnj', '', '200C'),
    ('zwnj mvs', 'Mvs', 'Mvs', '180E'),
    ('mvs zwnj', 'Mvs', 'Mvs', '180E'),
    ('zwj zwnj', 'Zwnj', 'Zwj', '200C'),
    ('zwnj zwj', 'Zwnj', 'Zwj', '200C'),
    # repeated breaks
    ('a zwnj zwnj b', 'A+A+Zwnj+B', 'A+A+B', '1820 180B 1820 180C 200C 182A'),
    ('zwnj b zwnj', 'B', 'B', '182A'),
    # between letters
    ('b zwnj b a', 'B+B+Aa', 'B+B+Aa', '182A 182A 1820 180B'),
    ('a o zwnj b a', 'A+A+U+Zwnj+B+Aa', 'A+A+O+B+Aa', '1820 180B 1820 1823 180C 200C 182A 1820 180B'),
    ('b zwnj a', 'B+Zwnj+A+A', 'B+Aa', '182A 200C 1820 180B 1820 180C'),
    ('n zwnj a', 'N+Zwnj+A+A', 'N+A', '1828 200C 1820 180B 1820 180C'),
    ('a zwnj a', 'A+A+Zwnj+A+A', 'A+A+A', '1820 180B 1820 180C 200C 1820 180B 1820 180C'),
    ('a b zwnj b a', 'A+A+B+Zwnj+B+Aa', 'A+A+B+B+Aa', '1820 180B 1820 182A 200C 182A 1820 180B'),
    ('l i zwnj e fvs1', 'L+I+Zwnj+Aa', 'L+G', '182F 1822 200C 1820 180C'),
    ('s zwnj g', 'S+Zwnj+Hx', 'S+G', '1830 200C 182C 180B'),
    ('b zwnj s', 'B+Zwnj+S', 'B+S', '182A 200C 1830'),
    ('t a l zwnj a', 'T+A+L+Zwnj+A+A', 'T+A+L+A', '1832 1820 182F 200C 1820 180B 1820 180C'),
    ('a zwnj b zwnj a', 'A+A+Zwnj+B+Zwnj+A+A', 'A+A+B+Aa', '1820 180B 1820 180C 200C 182A 200C 1820 180B 1820 180C'),
    ('a zwnj n zwnj a', 'A+A+Zwnj+N+Zwnj+A+A', 'A+A+N+A', '1820 180B 1820 180C 200C 1828 200C 1820 180B 1820 180C'),
    # beside an MVS / NNBSP
    ('mvs zwnj a', 'Mvs+Aa', 'Mvs+Aa', '180E 1820'),
    ('zwnj mvs a', 'Mvs+Aa', 'Mvs+Aa', '180E 1820'),
    ('a zwnj mvs a', 'A+A+Mvs+Aa', 'A+A+Mvs+Aa', '1820 180B 1820 180C 180E 1820'),
    ('a zwnj nnbsp a', 'A+A+Mvs+Aa', 'A+A+Mvs+Aa', '1820 180B 1820 180C 180E 1820'),
    ('mvs zwnj i', 'Mvs+I', 'Mvs+I', '180E 1822 180B'),
    ('mvs zwnj u n', 'Mvs+O+A', 'Mvs+O+A', '180E 1824 180B 1820 180C'),
    ('mvs zwnj y i n', 'Mvs+I+I+A', 'Mvs+I+I+A', '180E 1822 180B 1822 180D 1820 180C'),
    ('n zwnj mvs a', 'N+Mvs+Aa', 'N+Mvs+Aa', '1828 180E 1820'),
    ('a n zwnj mvs a', 'A+A+N+Mvs+Aa', 'A+A+N+Mvs+Aa', '1820 180B 1820 1828 180B 180E 1820'),
    ('s g zwnj mvs a', 'S+H+Mvs+Aa', 'S+H+Mvs+Aa', '1830 182C 180E 1820'),
    ('g zwnj mvs a', 'Hx+Mvs+Aa', 'Hx+Mvs+Aa', '182C 180B 180E 1820'),
    ('t a l mvs zwnj a', 'T+A+L+Mvs+Aa', 'T+A+L+Mvs+Aa', '1832 1820 182F 180E 1820'),
    # a break inside a particle ends it
    ('mvs i zwnj y a r', 'Mvs+I+Zwnj+Y+A+R', 'Mvs+I+I+A+R', '180E 1822 180B 200C 1836 180C 1820 1837'),
    ('mvs y zwnj i n', 'Mvs+Y+Zwnj+A+I+A', 'Mvs+I+I+A', '180E 1836 200C 1820 180B 1822 180D 1820 180C'),
    ('mvs u zwnj u', 'Mvs+U+Zwnj+A+O', 'Mvs+O+U', '180E 1824 180B 200C 1820 180B 1823 180B'),
    ('a mvs d u zwnj r', 'A+A+Mvs+D+U+Zwnj+R', 'A+A+Mvs+D+O+R', '1820 180B 1820 180C 180E 1833 180B 1823 180C 200C 1837'),
    # beside a nirugu
    ('n zwnj nirugu', 'N+Nirugu', 'N+Nirugu', '1828 180A'),
    ('zwnj nirugu i e fvs1', 'Nirugu+G', 'Nirugu+G', '180A 182D 180C'),
    ('a zwnj nirugu b', 'A+A+Zwnj+Nirugu+B', 'A+A+Nirugu+B', '1820 180B 1820 180C 200C 180A 182A'),
    ('a nirugu zwnj b', 'A+A+Nirugu+Zwnj+B', 'A+A+Nirugu+B', '1820 180B 1820 180A 200C 182A'),
    ('nirugu zwnj b', 'Nirugu+Zwnj+B', 'Nirugu+B', '180A 200C 182A'),
    # beside a ZWJ, both orders
    ('b zwnj zwj a', 'B+Aa', 'B+Aa', '182A 1820 180B'),
    ('n zwnj zwj a', 'N+A', 'N+A', '1828 1820 180C'),
    ('zwj zwnj n a', 'N+A', 'Zwj+N+A', '1828 1820 180C'),
    ('n a zwnj zwj', 'N+A', 'N+A+Zwj', '1828 1820 180C'),
    ('a zwj zwnj b a', 'A+A+B+Aa', 'A+A+B+Aa', '1820 180B 1820 182A 1820 180B'),
    ('a zwj zwnj b', 'A+A+Zwj+Zwnj+B', 'A+A+B', '1820 180B 1820 200D 200C 182A'),
    ('a zwnj zwj b', 'A+A+Zwnj+Zwj+B', 'A+A+B', '1820 180B 1820 180C 200C 200D 182A'),
    ('b zwj zwnj a', 'B+Zwnj+A+A', 'B+Aa', '182A 200C 1820 180B 1820 180C'),
    # FVS after a break is a stray
    ('a zwnj fvs1 a', 'A+A+Zwnj+A+A', 'A+A', '1820 180B 1820 180C 200C 1820 180B 1820 180C'),
    ('a fvs1 zwnj a', 'A+Zwnj+A+A', 'A+A', '1820 180B 200C 1820 180B 1820 180C'),
    ('b a fvs1 zwnj', 'B+Aa', 'B+Aa', '182A 1820 180B'),
    # gender and harmony cross the break
    ('n ue zwnj h nirugu', 'N+Ue+Zwnj+G+Nirugu', 'N+O+I+G+Nirugu', '1828 1825 180B 200C 182D 180C 180A'),
    ('h zwnj oe', 'H+Zwnj+A+Ue', 'G+Ue', '182C 200C 1820 180B 1825 180B'),
    ('g zwnj e', 'Hx+Zwnj+A', 'G+Aa', '182C 180B 200C 1820 180B'),
    ('a zwnj g', 'A+A+Zwnj+Hx', 'A+A+H', '1820 180B 1820 180C 200C 182C 180B'),
    ('a l zwnj g', 'A+A+L+Zwnj+Hx', 'A+A+L+G', '1820 180B 1820 182F 200C 182C 180B'),
    ('a b zwnj g', 'A+A+B+Zwnj+Hx', 'A+A+B+G', '1820 180B 1820 182A 200C 182C 180B'),
    ('i g zwnj b a', 'A+I+H+Zwnj+B+Aa', 'A+I+A+A+B+Aa', '1820 180B 1822 180D 182C 200C 182A 1820 180B'),
    ('i g zwnj a', 'A+I+G+Zwnj+A+A', 'A+I+N+N+A', '1820 180B 1822 180D 182D 180C 200C 1820 180B 1820 180C'),
    ('e b zwnj i g', 'A+B+Zwnj+A+I+G', 'A+B+I+G', '1820 180B 182A 200C 1820 180B 1822 180D 182D 180C'),
    ('s g zwnj a', 'S+G+Zwnj+A+A', 'S+A+A+A', '1830 182D 180C 200C 1820 180B 1820 180C'),
    ('m n oe zwnj g e', 'M+N+U+Zwnj+G+Aa', 'M+N+O+I+G+Aa', '182E 1828 180B 1823 180C 200C 182D 180C 1821 180B'),
    # the issue's corpus
    ('l a l a zwj zwnj p ue d', 'L+A+L+A+P+O+I+O+A', 'L+A+L+A+P+O+O+A', '182F 1820 182F 1820 182B 1823 1822 180D 1823 1820 180C'),
    ('t a ng zwj zwnj p u r u m', 'T+A+A+G+P+O+R+O+M', 'T+A+A+G+P+O+R+O+M', '1832 1820 1820 182D 180C 182B 1823 1837 1823 182E'),
    ('t i l zwj zwnj p u r u g', 'T+I+L+P+O+R+O+H', 'T+I+L+P+O+R+O+H', '1832 1822 180D 182F 182B 1823 1837 1823 182C'),
    ('s e ng zwj zwnj p u r u m', 'S+A+A+G+P+O+R+O+M', 'S+A+A+G+P+O+R+O+M', '1830 1820 1820 182D 180C 182B 1823 1837 1823 182E'),
    ('l a l a zwnj p ue d', 'L+A+L+A+Zwnj+P+O+I+O+A', 'L+A+L+A+P+O+O+A', '182F 1820 182F 1820 180C 200C 182B 1823 1822 180D 1823 1820 180C'),
    ('t a ng zwnj p u r u m', 'T+A+A+G+Zwnj+P+O+R+O+M', 'T+A+A+G+P+O+R+O+M', '1832 1820 1820 182D 180C 200C 182B 1823 1837 1823 182E'),
    ('t i l zwnj p u r u g', 'T+I+L+Zwnj+P+O+R+O+H', 'T+I+L+P+O+R+O+H', '1832 1822 180D 182F 200C 182B 1823 1837 1823 182C'),
]


class _Base(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper(locale='MNG')


class TestDropAndKeepTable(_Base):
    def test_table(self):
        for aliases, with_zwnj, without, canonical in TABLE:
            with self.subTest(aliases=aliases):
                text = _mgl(aliases)
                plain = text.replace(ZWNJ, '')
                self.assertEqual(self.s.shape_str(text), with_zwnj)
                self.assertEqual(self.s.shape_str(plain), without)
                kept = 'Zwnj' in with_zwnj.split('+')
                if kept:
                    self.assertNotEqual(self.s.shape(text), self.s.shape(plain))
                norm = self.s.normalize(text)
                self.assertEqual(_hex(norm), canonical)
                self.assertEqual(ZWNJ in norm, kept)
                self.assertEqual(self.s.shape(norm), self.s.shape(text))
                self.assertEqual(self.s.normalize(norm), norm)
                self.assertEqual(self.s._shape_raw(text).count('Zwnj'), text.count(ZWNJ))
                self.assertEqual(self.s.trace(text)['shape'], self.s.shape(text))
                self.assertEqual(self.s.normalize_written_units(self.s.shape(text)), norm)


class TestIssueCorpus(_Base):
    def test_first_word_keeps_its_marked_ue_but_no_control(self):
        word = '\u182F\u1820\u182F\u1820\u200D\u200C\u182B\u1826\u1833'
        self.assertEqual(self.s._shape_raw(word),
                         ['L', 'A', 'L', 'A', 'Zwj', 'Zwnj', 'P', 'O', 'I', 'Dd'])
        self.assertEqual(self.s.shape(word), ['L', 'A', 'L', 'A', 'P', 'O', 'I', 'O', 'A'])
        self.assertEqual(_hex(self.s.normalize(word)),
                         '182F 1820 182F 1820 182B 1823 1822 180D 1823 1820 180C')

    def test_other_words_shape_like_their_plain_spelling(self):
        for word, plain in [
            ('\u1832\u1820\u1829\u200D\u200C\u182B\u1824\u1837\u1824\u182E', 't a ng p u r u m'),
            ('\u1832\u1822\u182F\u200D\u200C\u182B\u1824\u1837\u1824\u182D', 't i l p u r u g'),
            ('\u1830\u1821\u1829\u200D\u200C\u182B\u1824\u1837\u1824\u182E', 's e ng p u r u m'),
        ]:
            with self.subTest(word=_hex(word)):
                self.assertTrue(self.s.same_shape(word, _mgl(plain)))
                self.assertEqual(self.s.normalize(word), self.s.normalize(_mgl(plain)))


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

    def test_fvs_after_a_break_is_a_stray(self):
        details = self.s.shape_detailed(_mgl('a zwnj fvs1 a'))
        self.assertEqual(len(details), 3)
        self.assertEqual([d['fvs'] for d in details], ['', '', ''])
        self.assertTrue(self.s.same_shape(_mgl('a zwnj fvs1 a'), _mgl('a zwnj a')))
        self.assertEqual(self.s.shape_detailed(_mgl('a fvs1 zwnj a'))[0]['fvs'], '+FVS1')


class TestConfluence(_Base):
    GROUPS = [
        ['n a', 'zwnj n a', 'n a zwnj', 'zwj zwnj n a', 'n a zwnj zwj', 'n zwnj zwj a',
         'zwnj zwnj n a zwnj'],
        ['b a', 'zwnj b a', 'b a zwnj', 'b zwnj zwj a', 'b a fvs1 zwnj', 'zwj b a zwnj'],
        ['b b a', 'b zwnj b a', 'b zwj zwnj b a', 'zwnj b zwnj b a'],
        ['a b a', 'a zwj zwnj b a'],
        ['mvs a', 'mvs zwnj a', 'zwnj mvs a', 'mvs zwnj zwnj a', 'nnbsp zwnj a'],
        ['s g mvs a', 's g zwnj mvs a', 's g zwnj zwj mvs a'],
        ['l a l a zwj zwnj p ue d', 'l a l a p o i fvs3 o a fvs2'],
    ]

    def test_equivalent_spellings_converge(self):
        for group in self.GROUPS:
            first = _mgl(group[0])
            for aliases in group[1:]:
                with self.subTest(aliases=aliases, reference=group[0]):
                    text = _mgl(aliases)
                    self.assertEqual(self.s.shape(text), self.s.shape(first))
                    self.assertEqual(self.s.normalize(text), self.s.normalize(first))

    def test_prefix_before_a_kept_break_is_stable(self):
        for left, right in [('b', 'a'), ('n ue', 'h nirugu'), ('t a l', 'a'), ('a', 'a'),
                            ('a o', 'b a')]:
            with self.subTest(left=left, right=right):
                norm = self.s.normalize(_mgl(f'{left} zwnj {right}'))
                self.assertTrue(norm.startswith(self.s.normalize(_mgl(left)) + ZWNJ))


class TestApis(_Base):
    def test_normalize_text_treats_zwnj_as_a_word_character(self):
        word = _mgl('b zwnj a')
        self.assertEqual(self.s.normalize_text(f'x {word} y'),
                         f'x {self.s.normalize(word)} y')
        self.assertEqual(self.s.normalize_text('a\u200Cb'), 'a\u200Cb')
        self.assertEqual(self.s.normalize_text(ZWNJ), ZWNJ)
        self.assertEqual(self.s.normalize_text(_mgl('zwnj n a')),
                         self.s.normalize(_mgl('n a')))

    def test_written_unit_apis(self):
        self.assertIn('Zwnj', self.s._known_units())
        self.assertEqual(self.s.parse_written_units('B+Zwnj+A+A'), ['B', 'Zwnj', 'A', 'A'])
        self.assertEqual(_hex(self.s.normalize_written_units(['B', 'Zwnj', 'A', 'A'])),
                         '182A 200C 1820 180B 1820 180C')
        self.assertEqual(self.s.normalize_written_units(['Zwnj', 'B', 'Aa']),
                         self.s.normalize(_mgl('b a')))
        self.assertEqual(self.s.normalize_written_units(['Zwnj']), ZWNJ)
        self.assertEqual(self.s.normalize_written_units(['Zwnj', 'Zwnj']), ZWNJ)

    def test_positioned_api_takes_zwnj_as_a_control(self):
        encoded = self.s.normalize_positioned_written_units([
            {'unit': 'B', 'position': 'init'},
            {'unit': 'Zwnj', 'position': 'control'},
            {'unit': 'A', 'position': 'isol'},
        ])
        self.assertIn(ZWNJ, encoded)
        self.assertEqual(self.s.shape(encoded), ['B', 'Zwnj', 'A'])
        with self.assertRaisesRegex(ValueError, r"control 'Zwnj' requires position 'control'"):
            self.s.normalize_positioned_written_units([{'unit': 'Zwnj', 'position': 'isol'}])
        self.assertEqual(
            self.s.normalize_positioned_written_units([{'unit': 'Zwnj', 'position': 'control'}]),
            ZWNJ)
        encoded = self.s.normalize_positioned_written_units([
            {'unit': 'A', 'position': 'isol'},
            {'unit': 'Zwnj', 'position': 'control'},
            {'unit': 'N', 'position': 'medi'},
        ])
        self.assertEqual(self.s.shape(encoded), ['A', 'Zwnj', 'Zwj', 'N', 'Zwj'])

    def test_canonical_version_and_error_message(self):
        self.assertEqual(self.s.canonical_version, 'mng-canonical/4')
        with self.assertRaisesRegex(ValueError, 'FVS/MVS/NNBSP/Nirugu/ZWJ/ZWNJ'):
            self.s.shape('\u1820x')


class TestFuzz(_Base):
    ALPHABET = [chr(cp) for cp in range(0x1820, 0x1843)] + [
        '\u180B', '\u180C', '\u180D', '\u180E', '\u202F', '\u180A', '\u200D',
        ZWNJ, ZWNJ, ZWNJ, ZWNJ,
    ]

    def test_round_trip_and_idempotence(self):
        rng = random.Random(34)
        normalized = 0
        for _ in range(5000):
            text = ''.join(rng.choice(self.ALPHABET) for _ in range(rng.randint(1, 10)))
            with self.subTest(text=_hex(text)):
                shape = self.s.shape(text)
                self.assertEqual(self.s._shape_raw(text).count('Zwnj'), text.count(ZWNJ))
                self.assertNotIn(('Zwnj', 'Zwnj'), list(zip(shape, shape[1:])))
                self.assertEqual(self.s.trace(text)['shape'], shape)
                try:
                    norm = self.s.normalize(text)
                except ValueError:
                    continue
                normalized += 1
                self.assertEqual(self.s.shape(norm), shape)
                self.assertEqual(self.s.normalize(norm), norm)
                self.assertEqual(self.s.normalize_written_units(shape), norm)
        self.assertGreater(normalized, 3500)


if __name__ == '__main__':
    unittest.main()
