#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Data-driven regression test against mongfontbuilder's core-hud.tsv.
跑 mongfontbuilder 的 core-hud.tsv 数据驱动回归测试。

Each row in `tests/data/core-hud.tsv` is `<test-id> <aliases> <expected>`,
tab-separated, where:
  - `aliases`  is a space-separated input (our `_mgl()` alias scheme)
  - `expected` is a space-separated list of written units

The TSV is vendored from
`mongfontbuilder/tests/data/core-hud.tsv` (commit shipped in the
mongfontbuilder source tree). Re-sync when upgrading mongfontbuilder.

mongfontbuilder's own test harness (`tests/test_font.py`) shapes the
input through HarfBuzz on a built OTF and compares to `expected`. Here
we shape through `MongolianShaper` (the Rust engine's Python binding)
and do the same comparison, after one normalization step: mongfontbuilder distinguishes
narrow (`_`) vs wide (`-`) MVS in its expected output, while our
`shape()` produces the generic `Mvs` token — we collapse both to
`Mvs` before comparing.
"""
import csv
import unittest

from mongol_norm import MongolianShaper
from tests._support import fixture_path

# Complete alias → codepoint map (superset of test_shaper.py's _ALIAS_TO_CP;
# adds letters used by core-hud.tsv but not by the hand-written tests:
# c, z, hh, rh, lh, zr, cr, and the literal `space` token).
_ALIAS_TO_CP = {
    # Vowels
    'a': 'ᠠ', 'e': 'ᠡ', 'i': 'ᠢ', 'o': 'ᠣ',
    'u': 'ᠤ', 'oe': 'ᠥ', 'ue': 'ᠦ', 'ee': 'ᠧ',
    # Consonants
    'n': 'ᠨ', 'ng': 'ᠩ', 'b': 'ᠪ', 'p': 'ᠫ',
    'h': 'ᠬ', 'g': 'ᠭ', 'm': 'ᠮ', 'l': 'ᠯ',
    's': 'ᠰ', 'sh': 'ᠱ', 't': 'ᠲ', 'd': 'ᠳ',
    'ch': 'ᠴ', 'j': 'ᠵ', 'y': 'ᠶ', 'r': 'ᠷ',
    'w': 'ᠸ', 'f': 'ᠹ', 'k2': 'ᠺ', 'k': 'ᠻ',
    'c': 'ᠼ', 'z': 'ᠽ', 'hh': 'ᠾ', 'rh': 'ᠿ',
    'lh': 'ᡀ', 'zr': 'ᡁ', 'cr': 'ᡂ',
    # Format controls
    'mvs': '᠎', 'fvs1': '᠋', 'fvs2': '᠌',
    'fvs3': '᠍', 'fvs4': '᠏', 'nnbsp': ' ',
    # NB: `space` is a *word boundary* in mongfontbuilder's test format.
    # We handle it by SPLITTING the input on `space`, shaping each word
    # independently, and concatenating outputs with `Mvs` between them.
    # See `_shape_aliases` below. (Hence no mapping for `space` here —
    # it should never reach `_aliases_to_text`.)
}


def _aliases_to_text(aliases: str) -> str:
    """Convert space-separated alias string to Unicode text (no `space` token)."""
    return ''.join(_ALIAS_TO_CP[a] for a in aliases.split())


def _shape_aliases(shaper, aliases: str) -> list:
    """
    Shape an alias string, treating `space` as a word boundary.

    mongfontbuilder's `space` (U+0020) is a hard word break — it produces
    a wide-MVS-shaped glyph but does NOT participate in particle-dict
    matching or marker propagation across the boundary. We model this by
    splitting the input on `space`, shaping each word independently, and
    joining the results with `Mvs` between them.
    """
    tokens = aliases.split()
    # Split into words on `space`
    words = [[]]
    for t in tokens:
        if t == 'space':
            words.append([])
        else:
            words[-1].append(t)
    out = []
    for i, word_tokens in enumerate(words):
        if i > 0:
            out.append('Mvs')
        if not word_tokens:
            continue
        text = ''.join(_ALIAS_TO_CP[a] for a in word_tokens)
        out.extend(shaper.shape(text))
    return out


def _normalize_expected(expected: str) -> list:
    """
    Normalize mongfontbuilder's expected-output tokens to match our
    shaper's output format.

    mongfontbuilder uses `_` for narrow MVS (chachlag context) and `-`
    for wide MVS (particle context). Our shaper produces the generic
    `Mvs` token regardless. Collapse both to `Mvs` for comparison.
    """
    out = []
    for tok in expected.split():
        if tok in ('_', '-'):
            out.append('Mvs')
        else:
            out.append(tok)
    return out


def _load_tsv() -> list:
    """Load all valid (non-comment, non-empty) rows from core-hud.tsv."""
    path = fixture_path('data', 'core-hud.tsv')
    rows = []
    with path.open(encoding='utf-8') as f:
        reader = csv.reader(f, delimiter='\t')
        for row in reader:
            if not row or row[0].startswith('#'):
                continue
            if len(row) < 3:
                continue
            rows.append(tuple(row[:3]))
    return rows


class TestCoreHud(unittest.TestCase):
    """
    Run every test case from core-hud.tsv through our shaper.
    把 core-hud.tsv 里每个测试 case 都跑一遍。
    """

    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper(locale='MNG')
        cls.rows = _load_tsv()

    def test_core_hud_all(self):
        failures = []
        for index, aliases, expected in self.rows:
            with self.subTest(index=index, input=aliases):
                actual = _shape_aliases(self.s, aliases)
                expected_norm = _normalize_expected(expected)
                if actual != expected_norm:
                    failures.append(
                        f"{index:10}  input={aliases!r}\n"
                        f"            got     {actual}\n"
                        f"            expect  {expected_norm}"
                    )
        # Print a summary at the end
        if failures:
            print(f"\n\n{len(failures)} / {len(self.rows)} failed:")
            for f in failures:
                print(f)
            self.fail(f"{len(failures)} of {len(self.rows)} core-hud cases failed (see above)")
        else:
            print(f"\n\nAll {len(self.rows)} core-hud cases passed.")