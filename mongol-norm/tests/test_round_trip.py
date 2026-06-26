#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Round-trip test for normalize: shape(input) == shape(normalize(input)).
normalize 往返测试：shape(输入) == shape(normalize(输入))。

The defining property of a correct normalize is that it preserves shape.
正确 normalize 的根本性质是：保形(reshape 后结果不变)。
The exact Unicode form normalize emits does not matter for this test —
what matters is that the new form reshapes to the SAME written-unit
sequence as the original.
normalize 输出的具体 Unicode 形式不重要 —— 关键是新形式重新 shape 后
仍能产生与原始输入相同的 written-unit 序列。

Test corpus = the three shape test suites:
测试语料 = 三套 shape 测试:
  1. Hand-written representative cases (this file, in INLINE_CASES)
     手写代表性用例 (本文件 INLINE_CASES)
  2. mongfontbuilder core-hud.tsv (~280 cases)
  3. GB/T 25914-2023 eac-hud.tsv (~3500 cases)

All cases that pass shape() must round-trip cleanly through normalize().
所有 shape() 通过的用例都必须能干净地通过 normalize() 往返。
"""
import csv
import unittest
from pathlib import Path

from mongol_norm import MongolianShaper

_ALIAS_TO_CP = {
    'a': 'ᠠ', 'e': 'ᠡ', 'i': 'ᠢ', 'o': 'ᠣ',
    'u': 'ᠤ', 'oe': 'ᠥ', 'ue': 'ᠦ', 'ee': 'ᠧ',
    'n': 'ᠨ', 'ng': 'ᠩ', 'b': 'ᠪ', 'p': 'ᠫ',
    'h': 'ᠬ', 'g': 'ᠭ', 'm': 'ᠮ', 'l': 'ᠯ',
    's': 'ᠰ', 'sh': 'ᠱ', 't': 'ᠲ', 'd': 'ᠳ',
    'ch': 'ᠴ', 'j': 'ᠵ', 'y': 'ᠶ', 'r': 'ᠷ',
    'w': 'ᠸ', 'f': 'ᠹ', 'k2': 'ᠺ', 'k': 'ᠻ',
    'c': 'ᠼ', 'z': 'ᠽ', 'hh': 'ᠾ', 'rh': 'ᠿ',
    'lh': 'ᡀ', 'zr': 'ᡁ', 'cr': 'ᡂ',
    'mvs': '᠎', 'fvs1': '᠋', 'fvs2': '᠌',
    'fvs3': '᠍', 'fvs4': '᠏', 'nnbsp': ' ',
    'nirugu': '᠊', 'zwj': '‍',
}


def _mgl(aliases: str) -> str:
    """Build a Unicode Mongolian word from space-separated aliases.
    用空格分隔的别名字符串构建蒙古文 Unicode 字符串。
    Example: _mgl('s a i n') → 'ᠰᠠᠢᠨ'.
    """
    return ''.join(_ALIAS_TO_CP[a] for a in aliases.split())


def _aliases_to_words(aliases: str):
    """Split alias string on `space` boundary, return list of Unicode words."""
    tokens = aliases.split()
    words = [[]]
    for t in tokens:
        if t == 'space':
            words.append([])
        else:
            words[-1].append(t)
    return [''.join(_ALIAS_TO_CP[a] for a in w) for w in words]


def _round_trip(shaper, word_text):
    """Return (shape_of_input, normalize_output, shape_of_normalize)."""
    s1 = shaper.shape(word_text)
    norm = shaper.normalize(word_text)
    s2 = shaper.shape(norm)
    return s1, norm, s2


def _hex(text):
    return ' '.join(f'{ord(c):04X}' for c in text)


# ────────────────────────────────────────────────────────────────────
# Hand-written representative cases — every encoding here is a known
# real-world stressor for normalize. shape() of each must equal
# shape(normalize(.)) of each. Aliases are space-separated; _mgl()
# materializes the Mongolian text at test time (mirrors test_shaper.py
# and the core/eac TSVs).
# 手写代表性用例 —— 每个编码都是已知的 normalize 真实场景压测点。
# 别名以空格分隔,运行时由 _mgl() 转为蒙古文(与 test_shaper.py 及
# core/eac TSV 一致)。
# ────────────────────────────────────────────────────────────────────
INLINE_CASES = [
    # The five sain encodings — should all round-trip to the same shape.
    # sain 五种编码 —— 应全部往返到相同 shape。
    ('sain.base',            's a i n'),
    ('sain.e_variant',       's e i n'),
    ('sain.na_fvs2',         's n fvs2 i i n'),
    ('sain.ya_fvs1_i',       's a y fvs1 i n'),
    ('sain.ya_fvs1_ya_fvs1', 's a y fvs1 y fvs1 n'),

    # Bayan-Öndör (the case the user just walked through) — n+fvs2.onset
    # and ue+fvs2 with the three-unit written tuple A O I.
    # Bayan-Öndör (用户刚走过的例子) —— n+fvs2.onset 和 ue+fvs2 三段 written A O I。
    ('bayan_ondor',          'b a y a n fvs2 ue fvs2 n d ue r'),

    # UTN-correct shapings pinned in test_shaper.py — these MUST round-trip
    # because EAC disagrees but our shape is the source of truth.
    # UTN 正确形(test_shaper.py 已固定) —— 必须往返,因为我们以 UTN 为准。
    ('utn.g_fvs2_blocks',    'b a g fvs2 mvs a'),
    ('utn.g_fvs3_chachlag',  'b a g fvs3 mvs a'),
    ('utn.nnbsp_alone',      'nnbsp'),
    ('utn.nnbsp_chachlag',   'b a g nnbsp a'),
    ('utn.nnbsp_particle',   'a b u nnbsp y i n'),
    ('utn.nnbsp_mvs_token',  'a b u nnbsp e j i'),

    # MVS + chachlag/particle — wide/narrow MVS behavior.
    # MVS + chachlag/particle —— 宽/窄 MVS 行为。
    ('chachlag.tala',        't a l mvs a'),
    ('particle.talayin',     't a l mvs a mvs y i n'),

    # Pure-default cases (nothing fancy) — should be the easy wins.
    # 纯默认情况(无任何花样) —— 应是最容易通过的。
    ('plain.morin',          'm o r i n'),
    ('plain.nom',            'n o m'),
    ('plain.tngri',          't ng r i'),

    ('paticle.i',            'mvs i'),
    ('paticle.i.iso',         'i fvs1'),
]


def _load_tsv(name):
    path = Path(__file__).parent / 'data' / name
    rows = []
    with path.open(encoding='utf-8') as f:
        for row in csv.reader(f, delimiter='\t'):
            if not row or row[0].startswith('#'):
                continue
            if len(row) < 3:
                continue
            rows.append(tuple(row[:3]))
    return rows


class _RoundTripBase:
    """Mixin: drives round-trip assertions and prints a compact summary."""

    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper(locale='MNG')

    def _check_word(self, label, word_text, failures):
        try:
            s1, norm, s2 = _round_trip(self.s, word_text)
        except Exception as e:
            failures.append(f"{label:30}  CRASH  {type(e).__name__}: {e}")
            return
        if s1 != s2:
            failures.append(
                f"{label:30}\n"
                f"   input  : {word_text!r}  ({_hex(word_text)})\n"
                f"   shape1 : {s1}\n"
                f"   norm   : {norm!r}  ({_hex(norm)})\n"
                f"   shape2 : {s2}"
            )

    def _check_aliases(self, label, aliases, failures):
        for word_text in _aliases_to_words(aliases):
            if not word_text:
                continue
            self._check_word(label, word_text, failures)


class TestRoundTripInline(unittest.TestCase, _RoundTripBase):
    """Round-trip on the hand-written representative cases."""

    @classmethod
    def setUpClass(cls):
        _RoundTripBase.setUpClass.__func__(cls)

    def test_round_trip_inline(self):
        failures = []
        for label, aliases in INLINE_CASES:
            # INLINE_CASES are alias strings; pass through _aliases_to_words
            # so `space` (multi-word) and bare aliases both work uniformly.
            # INLINE_CASES 是别名字符串,统一走 _aliases_to_words 以同时
            # 处理 `space` 多词和单词两种情况。
            self._check_aliases(label, aliases, failures)
        total = len(INLINE_CASES)
        passed = total - len(failures)
        print(f"\nINLINE: {passed} / {total} round-tripped")
        if failures:
            for f in failures:
                print(f)
            self.fail(f"{len(failures)} of {total} inline round-trips failed")


class TestRoundTripCoreHud(unittest.TestCase, _RoundTripBase):
    """Round-trip every core-hud.tsv input through normalize."""

    @classmethod
    def setUpClass(cls):
        _RoundTripBase.setUpClass.__func__(cls)
        cls.rows = _load_tsv('core-hud.tsv')

    def test_round_trip_core_hud(self):
        failures = []
        word_count = 0
        for index, aliases, _expected in self.rows:
            for word_text in _aliases_to_words(aliases):
                if not word_text:
                    continue
                word_count += 1
                self._check_word(f"{index}", word_text, failures)
        passed = word_count - len(failures)
        pct = 100 * passed / word_count if word_count else 0
        print(f"\nCORE-HUD: {passed} / {word_count} round-tripped ({pct:.1f}%)")
        if failures:
            print("First 30 failures:")
            for f in failures[:30]:
                print(f)
            if len(failures) > 30:
                print(f"... ({len(failures) - 30} more)")
            self.fail(f"{len(failures)} of {word_count} core-hud round-trips failed")


class TestShapeCanonicity(unittest.TestCase, _RoundTripBase):
    """
    Stronger property: same shape ⟹ same normalize output.
    更强性质:同 shape ⟹ normalize 输出相同。

    Pull all inputs from all three corpora, group them by their shape
    output, then assert that within each group every input produces the
    IDENTICAL normalize result. If two encodings render visually the same
    they must canonicalize identically; that is the user-facing meaning
    of normalize.
    汇总三套语料的所有输入,按 shape 分组,断言同组内所有输入的 normalize
    输出完全一致。视觉相同必须 Unicode 相同 —— 这才是 normalize 的用户语义。
    """

    @classmethod
    def setUpClass(cls):
        _RoundTripBase.setUpClass.__func__(cls)
        cls.core_rows = _load_tsv('core-hud.tsv')
        cls.eac_rows = _load_tsv('eac-hud.tsv')

    def _all_words(self):
        for _, aliases in INLINE_CASES:
            for w in _aliases_to_words(aliases):
                if w:
                    yield w
        for index, aliases, _exp in self.core_rows:
            for w in _aliases_to_words(aliases):
                if w:
                    yield w
        for index, aliases, _exp in self.eac_rows:
            toks = aliases.split()
            if any(t != 'space' and t not in _ALIAS_TO_CP for t in toks):
                continue
            for w in _aliases_to_words(aliases):
                if w:
                    yield w

    def test_same_shape_same_normalize(self):
        # shape_tuple → {normalize_output: [example_inputs...]}
        # If any group's inner dict has >1 key, the property failed.
        groups = {}
        for word in self._all_words():
            sh = tuple(self.s.shape(word))
            nm = self.s.normalize(word)
            groups.setdefault(sh, {}).setdefault(nm, []).append(word)

        divergences = []
        for sh, by_norm in groups.items():
            if len(by_norm) > 1:
                divergences.append((sh, by_norm))

        total_groups = len(groups)
        diverged = len(divergences)
        converged = total_groups - diverged
        print(f"\nSHAPE-CANONICITY: {converged} / {total_groups} shape-groups "
              f"converge to one normalize")
        if divergences:
            print(f"First 10 divergences:")
            for sh, by_norm in divergences[:10]:
                print(f"  shape={list(sh)}")
                for nm, examples in by_norm.items():
                    print(f"    normalize → {nm!r}  from {examples[:3]}{'...' if len(examples) > 3 else ''}")
            self.fail(
                f"{diverged} of {total_groups} shape-groups produced "
                f">1 distinct normalize output — same shape must map to same Unicode."
            )


class TestRoundTripEacHud(unittest.TestCase, _RoundTripBase):
    """Round-trip every eac-hud.tsv input through normalize."""

    @classmethod
    def setUpClass(cls):
        _RoundTripBase.setUpClass.__func__(cls)
        cls.rows = _load_tsv('eac-hud.tsv')

    def test_round_trip_eac_hud(self):
        failures = []
        word_count = 0
        for index, aliases, _expected in self.rows:
            tokens = aliases.split()
            unknown = [t for t in tokens if t != 'space' and t not in _ALIAS_TO_CP]
            if unknown:
                continue
            for word_text in _aliases_to_words(aliases):
                if not word_text:
                    continue
                word_count += 1
                self._check_word(f"{index}", word_text, failures)
        passed = word_count - len(failures)
        pct = 100 * passed / word_count if word_count else 0
        print(f"\nEAC-HUD: {passed} / {word_count} round-tripped ({pct:.1f}%)")
        if failures:
            print("First 30 failures:")
            for f in failures[:30]:
                print(f)
            if len(failures) > 30:
                print(f"... ({len(failures) - 30} more)")
            self.fail(f"{len(failures)} of {word_count} eac-hud round-trips failed")


# ────────────────────────────────────────────────────────────────────
# Particle test cases — verify the user-requested rules:
#   1. `I` at iso always normalizes to `i+fvs1` (not bare `j`)
#   2. `mvs + particle` and `particle alone` give the same encoding
#      (the chain portion after MVS doesn't rely on MVS to render)
# Exception: chachlag (chain shape ('Aa',) after MVS) keeps `mvs + bare a/e`.
# Also: normalize output must not contain nirugu (U+180A).
# 粒子用例 —— 验证用户规则:I iso 总归 i+fvs1;mvs+particle 与 particle
# 编码一致(不依赖 MVS 渲染)。chachlag 例外。normalize 不应含 nirugu。
# ────────────────────────────────────────────────────────────────────
PARTICLE_CASES = [
    # Single-letter chains after MVS — vowel particles
    ('mvs+i',                'mvs i'),
    ('mvs+u (masc)',         'mvs u'),
    ('mvs+ue (fem)',         'mvs ue'),
    # Chachlag — special case, keeps bare a/e
    ('mvs+a chachlag',       'mvs a'),
    ('mvs+e chachlag',       'mvs e'),
    # Multi-letter particle chains after MVS
    ('mvs+yin (genitive)',   'mvs y i n'),
    ('mvs+un (u variant)',   'mvs u n'),
    ('mvs+un (ue variant)',  'mvs ue n'),
    ('mvs+du (u dative)',    'mvs d u'),
    ('mvs+du (ue dative)',   'mvs d ue'),
    ('mvs+tu (t-dative)',    'mvs t u'),
    ('mvs+ban (reflexive)',  'mvs b a n'),
    ('mvs+ben (reflexive)',  'mvs b e n'),
    ('mvs+daga (locative)',  'mvs d a g a'),
    ('mvs+yi (acc)',         'mvs y i'),
    ('mvs+iyer (instr)',     'mvs i y e r'),
    # Single-letter I at iso — rule 1
    ('I iso (j alone)',      'j'),
    ('I iso (i+fvs1)',       'i fvs1'),
]

# Groups of inputs whose shape is identical and therefore whose
# normalize output must also be identical (shape-canonicity).
# 同 shape 必同 normalize 的输入组(canonicity 检查)。
PARTICLE_EQUIVALENCE_GROUPS = [
    ('mvs+u vs mvs+ue',      ['mvs u', 'mvs ue']),
    ('mvs+a vs mvs+e (chachlag)', ['mvs a', 'mvs e']),
    ('mvs+un (u/ue)',        ['mvs u n', 'mvs ue n']),
    ('mvs+du (u/ue)',        ['mvs d u', 'mvs d ue']),
    ('mvs+ban vs mvs+ben',   ['mvs b a n', 'mvs b e n']),
    ('I iso (j vs i+fvs1)',  ['j', 'i fvs1']),
]


class TestParticleUniform(unittest.TestCase, _RoundTripBase):
    """
    Particle rules: chain encoding after MVS shouldn't rely on MVS to
    render correctly (Rule 2). Single-letter `I` at iso always uses the
    vowel particle `i+fvs1` (Rule 1). Chachlag (`mvs + bare a/e`) is the
    explicit exception. Normalize output must not contain nirugu.
    粒子规则:MVS 后的 chain 编码不依赖 MVS 渲染;`I` iso 总用 `i+fvs1`;
    chachlag 例外;normalize 输出不应含 nirugu。
    """

    @classmethod
    def setUpClass(cls):
        _RoundTripBase.setUpClass.__func__(cls)

    def test_particles_round_trip(self):
        """Each particle case must round-trip (shape preserved)."""
        failures = []
        for label, aliases in PARTICLE_CASES:
            for word_text in _aliases_to_words(aliases):
                if not word_text:
                    continue
                self._check_word(label, word_text, failures)
        if failures:
            for f in failures:
                print(f)
            self.fail(f"{len(failures)} of {len(PARTICLE_CASES)} particle round-trips failed")

    def test_mvs_uniform_no_mvs_dependency(self):
        """
        For chain after MVS (except chachlag), stripping the MVS prefix
        from normalize() output must give a chain that, when shaped
        ALONE, equals the chain portion of the MVS-context shape.
        I.e., the chain encoding doesn't depend on MVS to render correctly.
        MVS 后的 chain(chachlag 除外):去 MVS 前缀后独立 shape 应等于
        其在 MVS 上下文中的 shape。
        """
        mvs_ch = chr(0x180E)
        chachlag_chain = ('Aa',)
        failures = []
        for label, aliases in PARTICLE_CASES:
            for word_text in _aliases_to_words(aliases):
                if not word_text or not word_text.startswith(mvs_ch):
                    continue
                norm = self.s.normalize(word_text)
                if not norm.startswith(mvs_ch):
                    failures.append(f"{label}: normalize lost MVS prefix: {norm!r}")
                    continue
                in_ctx_shape = self.s.shape(word_text)
                chain_shape = tuple(u for u in in_ctx_shape if u != 'mvs')
                if chain_shape == chachlag_chain:
                    continue
                chain_text = norm[len(mvs_ch):]
                alone_shape = tuple(self.s.shape(chain_text))
                if alone_shape != chain_shape:
                    failures.append(
                        f"{label}: chain after MVS depends on MVS to render\n"
                        f"   input        : {word_text!r}\n"
                        f"   normalize    : {norm!r}\n"
                        f"   shape(input) : {list(in_ctx_shape)}\n"
                        f"   chain alone  : {list(alone_shape)}\n"
                        f"   chain in ctx : {list(chain_shape)}"
                    )
        if failures:
            for f in failures:
                print(f)
            self.fail(f"{len(failures)} particle cases depend on MVS for chain rendering")

    def test_i_iso_always_i_fvs1(self):
        """Rule 1: shape ['I'] at iso → `i+fvs1` (not bare `j`)."""
        i_cp = chr(0x1822)
        fvs1_cp = chr(0x180B)
        expected = i_cp + fvs1_cp
        failures = []
        for aliases in ('j', 'i fvs1'):
            word = _mgl(aliases)
            norm = self.s.normalize(word)
            if norm != expected:
                failures.append(
                    f"input={word!r} ({aliases}) normalize→{norm!r}, expected {expected!r}"
                )
        if failures:
            for f in failures:
                print(f)
            self.fail("`I` iso normalize must be `i+fvs1`")

    def test_no_nirugu_in_normalize_output(self):
        """
        Normalize output for particle cases must not contain nirugu
        (U+180A). Canonical should prefer explicit FVS encoding instead
        of nirugu wraps.
        normalize 输出中不应有 nirugu。
        """
        nirugu = chr(0x180A)
        failures = []
        for label, aliases in PARTICLE_CASES:
            for word_text in _aliases_to_words(aliases):
                if not word_text:
                    continue
                norm = self.s.normalize(word_text)
                if nirugu in norm:
                    failures.append(f"{label}: normalize({word_text!r}) = {norm!r} contains nirugu")
        if failures:
            for f in failures:
                print(f)
            self.fail(f"{len(failures)} particle normalize outputs contain nirugu")

    def test_equivalence_groups_converge(self):
        """
        Each equivalence group of same-shape inputs must converge to
        ONE normalize output (shape-canonicity for these specific cases).
        每个等价组内所有输入的 normalize 必须完全一致。
        """
        failures = []
        for label, alias_list in PARTICLE_EQUIVALENCE_GROUPS:
            outputs = set()
            shapes = set()
            for aliases in alias_list:
                for word_text in _aliases_to_words(aliases):
                    if not word_text:
                        continue
                    shapes.add(tuple(self.s.shape(word_text)))
                    outputs.add(self.s.normalize(word_text))
            if len(shapes) != 1:
                failures.append(
                    f"{label}: inputs have DIFFERENT shapes — not a valid equivalence group\n"
                    f"   shapes: {shapes}"
                )
                continue
            if len(outputs) != 1:
                failures.append(
                    f"{label}: inputs {alias_list} produced {len(outputs)} distinct normalize:\n"
                    f"   {outputs}"
                )
        if failures:
            for f in failures:
                print(f)
            self.fail(f"{len(failures)} equivalence groups diverged")

    def test_chain_part_matches_standalone(self):
        """
        User-requested property: with or without MVS, the chain part's
        normalize encoding is identical.
        用户要求:有没有 MVS,这个 chain 的 normalize 编码必须一致。

        Concretely, for every input `mvs + X` (non-chachlag), let
        chain_text = normalize(mvs + X)[len(mvs):]. Then
        normalize(chain_text) must equal chain_text itself — i.e., the
        chain text is already in canonical form as a standalone input.

        Examples:
          normalize('mvs+i')  = 'mvs + i+fvs1'   →  chain='i+fvs1'
          normalize('i+fvs1') = 'i+fvs1'           ✓ same

          normalize('mvs+u')  = 'mvs + u+fvs1'   →  chain='u+fvs1'
          normalize('u+fvs1') = 'u+fvs1'           ✓ same

          normalize('mvs+yin')= 'mvs + j+i+a'    →  chain='j+i+a'
          normalize('j+i+a')  = 'j+i+a'            ✓ same
        """
        mvs = chr(0x180E)
        chachlag_chain = ('Aa',)
        failures = []
        for label, aliases in PARTICLE_CASES:
            for word_text in _aliases_to_words(aliases):
                if not word_text or not word_text.startswith(mvs):
                    continue
                norm = self.s.normalize(word_text)
                if not norm.startswith(mvs):
                    continue
                in_ctx_shape = self.s.shape(word_text)
                chain_shape = tuple(u for u in in_ctx_shape if u != 'mvs')
                if chain_shape == chachlag_chain:
                    continue  # chachlag is the documented exception
                chain_text = norm[len(mvs):]
                chain_normalize = self.s.normalize(chain_text)
                if chain_normalize != chain_text:
                    failures.append(
                        f"{label}:\n"
                        f"   input            : {word_text!r}\n"
                        f"   normalize        : {norm!r}\n"
                        f"   chain (strip mvs): {chain_text!r}\n"
                        f"   normalize(chain) : {chain_normalize!r}\n"
                        f"   — chain text isn't its own normalize, so encoding differs with/without MVS"
                    )
        if failures:
            for f in failures:
                print(f)
            self.fail(f"{len(failures)} cases: chain encoding differs with/without MVS")


if __name__ == '__main__':
    unittest.main(verbosity=2)
