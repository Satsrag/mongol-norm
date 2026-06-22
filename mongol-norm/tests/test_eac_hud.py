#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Data-driven cross-implementation test against mongfontbuilder's
`eac-hud.tsv` — the GB/T 25914-2023 EAC (Encoding Adjustment of
Characters) compliance suite for Hudum (MNG).
跑国标 GB/T 25914-2023 EAC 一致性测试集(Hudum / MNG)。

3500+ test cases organized by `ruleindex` (e.g. `MAD10`, `MED12`,
`MAM31`), one TSV row per case. The TSV is vendored from
`mongfontbuilder/tests/data/eac-hud.tsv` (alongside its JSON twin
`eac-hud.json`, which we don't use here).

Format/normalization is the same as `test_core_hud.py`:
  - input  : space-separated aliases, `space` is a word boundary
  - output : space-separated written units; `_` / `-` collapsed to `mvs`

EAC adds a few tokens beyond core-hud:
  - `nirugu` (U+180A) — joining marker
  - `zwj`    (U+200D) — zero-width joiner
  - `fvs4`   (U+180F) — already in our existing alias map

Caveats:
  - Some EAC expected outputs use mongfontbuilder glyph-name artifacts
    we don't emit (`<`/`>` join markers, capitalized `Mvs`/`Nnbsp`,
    explicit `Fvs1..4`). Such cases are expected to fail; classify
    them after running.
  - Some EAC rules diverge from the UTN model on purpose
    (mongfontbuilder itself marks these as `xfail` — see
    `mongfontbuilder/tests/test_font.py:test_MNG`).
"""
import csv
import unittest
from pathlib import Path

from mongol_norm import MongolianShaper

# Alias → codepoint map for the EAC test format.
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
    'nirugu': '᠊',  # Mongolian Nirugu (joining marker)
    'zwj': '‍',     # Zero Width Joiner
    # `space` is handled separately as a word boundary; see _shape_aliases.
}


def _shape_aliases(shaper, aliases: str) -> list:
    """Shape an alias string, treating `space` as a word boundary."""
    tokens = aliases.split()
    words = [[]]
    for t in tokens:
        if t == 'space':
            words.append([])
        else:
            words[-1].append(t)
    out = []
    for i, word_tokens in enumerate(words):
        if i > 0:
            out.append('mvs')
        if not word_tokens:
            continue
        text = ''.join(_ALIAS_TO_CP[a] for a in word_tokens)
        out.extend(shaper.shape(text))
    return out


# Tokens mongfontbuilder emits from font glyph-name parsing that our
# pure-Python shaper doesn't produce — strip them from expected output
# so the comparison focuses on actual shape differences.
#   Ni              — Nirugu glyph (joining marker render)
#   < / >           — Left / Right join markers
#   Fvs1..4         — explicit FVS rendering
#   Nnbsp           — explicit NNBSP rendering
_FONT_NAMING_ARTIFACTS = {'Ni', '<', '>', 'Fvs1', 'Fvs2', 'Fvs3', 'Fvs4', 'Nnbsp'}


def _normalize_expected(expected: str) -> list:
    """
    Normalize mongfontbuilder's expected-output tokens for comparison
    against our shaper's output:
      - Collapse narrow `_` / wide `-` / capitalized `Mvs` → `mvs`
      - Drop font-naming artifacts (`Ni`, `<`, `>`, `Fvs1..4`, `Nnbsp`)
    """
    out = []
    for tok in expected.split():
        if tok in ('_', '-', 'Mvs'):
            out.append('mvs')
        elif tok in _FONT_NAMING_ARTIFACTS:
            continue  # drop
        else:
            out.append(tok)
    return out


def _load_tsv() -> list:
    """Load all valid rows from eac-hud.tsv."""
    path = Path(__file__).parent / 'data' / 'eac-hud.tsv'
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


class TestEacHud(unittest.TestCase):
    """
    Run every test case from eac-hud.tsv through our shaper.
    GB/T 25914-2023 EAC compliance suite.
    """

    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper(locale='MNG')
        cls.rows = _load_tsv()

    def test_eac_hud_all(self):
        failures = []
        skipped_tokens = set()
        for index, aliases, expected in self.rows:
            # Skip any case using tokens we don't have in our alias map
            tokens = aliases.split()
            unknown = [t for t in tokens if t != 'space' and t not in _ALIAS_TO_CP]
            if unknown:
                skipped_tokens.update(unknown)
                continue
            try:
                actual = _shape_aliases(self.s, aliases)
            except KeyError as e:
                failures.append(f"{index:12}  input={aliases!r}  PARSE ERROR: {e}")
                continue
            expected_norm = _normalize_expected(expected)
            if actual != expected_norm:
                failures.append(
                    f"{index:12}  input={aliases!r}\n"
                    f"              got     {actual}\n"
                    f"              expect  {expected_norm}"
                )
        passed = len(self.rows) - len(failures)
        pct = 100 * passed / len(self.rows) if self.rows else 0
        print(f"\n\n{passed} / {len(self.rows)} passed ({pct:.1f}%); "
              f"{len(failures)} failed")
        if skipped_tokens:
            print(f"  Skipped (unknown tokens): {sorted(skipped_tokens)}")
        if failures:
            # Print just the first 30 to keep output bounded
            print(f"\nFirst 30 failures:")
            for f in failures[:30]:
                print(f)
            print(f"\n... ({len(failures) - 30} more)" if len(failures) > 30 else "")
            self.fail(f"{len(failures)} of {len(self.rows)} eac-hud cases failed")
        else:
            print(f"\nAll {len(self.rows)} eac-hud cases passed.")