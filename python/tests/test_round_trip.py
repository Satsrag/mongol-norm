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

from mongol_norm import MongolianShaper
from mongol_norm._data import load_rules
from tests._support import fixture_path

# Structural shape tokens (they split a word into letter chains).
# 结构 shape token(把词切成字母 chain)。
_STRUCTURAL_UNITS = ('Mvs', 'Nirugu', 'Zwj')

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
    path = fixture_path('data', name)
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

    def test_public_written_unit_api_covers_all_shape_groups(self):
        representatives = {}
        for word in self._all_words():
            shape = tuple(self.s.shape(word))
            representatives.setdefault(shape, word)

        self.assertEqual(
            len(representatives),
            # 1993 before the collapse merged two pairs of same-word shape groups.
            1991,
            "corpus shape-group coverage drifted",
        )

        compact_failures = []
        for shape in representatives:
            compact = "".join(shape)
            try:
                parsed = tuple(self.s.parse_written_units(compact))
            except ValueError as exc:
                compact_failures.append((shape, compact, str(exc)))
                continue
            if parsed != shape:
                compact_failures.append((shape, compact, parsed))
        self.assertFalse(
            compact_failures,
            f"compact PascalCase parsing failed: {compact_failures[:5]}",
        )

        failures = []
        for shape, word in representatives.items():
            try:
                encoded = self.s.normalize_written_units(shape)
            except Exception as exc:
                failures.append(
                    f"shape={list(shape)!r}: {type(exc).__name__}: {exc}"
                )
                continue
            if encoded != self.s.normalize(word):
                failures.append(
                    f"shape={list(shape)!r}: written-unit API {encoded!r} != "
                    f"normalize {self.s.normalize(word)!r}"
                )
            elif tuple(self.s.shape(encoded)) != shape:
                failures.append(
                    f"shape={list(shape)!r}: output reshaped as "
                    f"{self.s.shape(encoded)!r}"
                )

        if failures:
            self.fail(
                f"{len(failures)} of {len(representatives)} shape groups failed "
                f"the public written-unit API:\n" + "\n".join(failures[:20])
            )

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
            print("First 10 divergences:")
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
                chain_shape = tuple(u for u in in_ctx_shape if u != 'Mvs')
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

    def _check_chain_shape_uniform(self, word_text):
        """
        Verify that the chain after MVS doesn't depend on MVS to render
        correctly: the chain part of normalize, shaped alone, equals the
        chain part of the input's shape.
        校验 MVS 后的 chain 不依赖 MVS 渲染:normalize 去掉首个 MVS 后
        独立 shape 应等于输入 shape 去掉首个 'mvs' 后的部分。

        Returns None on pass / N/A; returns an error description on fail.
        通过或不适用返回 None,失败返回错误描述。
        """
        mvs_char = chr(0x180E)
        with_mvs_shape = self.s.shape(word_text)
        if not with_mvs_shape or with_mvs_shape[0] != 'Mvs':
            return None
        except_shape = with_mvs_shape[1:]  # remove first 'mvs' token

        with_mvs_norm = self.s.normalize(word_text)
        if not with_mvs_norm.startswith(mvs_char):
            return None
        without_mvs_norm = with_mvs_norm[len(mvs_char):]  # remove first MVS char
        without_mvs_shape = self.s.shape(without_mvs_norm)

        if except_shape != without_mvs_shape:
            return (
                f"input         : {word_text!r}\n"
                f"   shape(input)         : {with_mvs_shape}\n"
                f"   expect (strip 1st mvs): {except_shape}\n"
                f"   normalize            : {with_mvs_norm!r}\n"
                f"   strip 1st MVS char   : {without_mvs_norm!r}\n"
                f"   shape of that        : {without_mvs_shape}"
            )
        return None

    def test_particles_from_data(self):
        """
        Data-driven sweep using the FULL particle list from
        mongfontbuilder (the `particles` table of the bundled shaping-rules
        JSON, `mongol_norm._data.load_rules("MNG")`, originally
        `mongfontbuilder/data/particles.json`).

        47 MNG particle patterns include multi-letter chains like
        `mvs y i n`, `mvs d ue r`, `mvs ch ue` etc. For each particle
        starting with 'mvs', apply the user's pseudo-code shape-
        uniformity check.
        数据驱动:用 mongfontbuilder 的完整 particle 列表(47 个 MNG
        条目,含多字母短语)。每个以 mvs 开头的条目都跑伪代码 shape 对比。

        Note: particles.json doesn't include pure `mvs+a` / `mvs+e`,
        so no chachlag exception is needed here.
        注:particles.json 不含纯 `mvs+a`/`mvs+e`,故无需 chachlag 例外。
        """
        mvs_char = chr(0x180E)

        particles = sorted(load_rules('MNG')['particles'].keys())
        failures = []
        checked = 0
        skipped_no_mvs = 0

        for alias_string in particles:
            try:
                word_text = _mgl(alias_string)
            except KeyError as e:
                failures.append(f"unknown alias in {alias_string!r}: {e}")
                continue

            # Particles like `u u`, `ue ue`, `b ue ue` don't start with
            # mvs and don't fit the MVS-uniformity rule. Skip these.
            # `u u`、`ue ue` 这类不带 mvs 的 particle 不适用本规则,跳过。
            if not word_text.startswith(mvs_char):
                skipped_no_mvs += 1
                continue

            fail = self._check_chain_shape_uniform(word_text)
            if fail:
                failures.append(f"particle {alias_string!r}:\n   {fail}")
            checked += 1

        print(f"\nparticle data sweep: {len(particles)} particles total, "
              f"{checked} checked, {skipped_no_mvs} no-mvs skipped")
        if failures:
            for f in failures[:20]:
                print(f)
            if len(failures) > 20:
                print(f"... ({len(failures) - 20} more)")
            self.fail(f"{len(failures)} particles fail shape-uniformity")


class TestNormalizeFast(unittest.TestCase, _RoundTripBase):
    """
    Long chains must stay fast. An 18-unit single morpheme used to take
    ~9s via the budget-ladder DFS; the per-unit table path brings it to ~ms.
    长 chain 必须快。18-unit 单语素曾经 ~9s,逐单元表路径降到毫秒级。
    """

    @classmethod
    def setUpClass(cls):
        _RoundTripBase.setUpClass.__func__(cls)

    def test_long_chain_is_fast(self):
        import time
        chain = ('A', 'O', 'I', 'I', 'L', 'A', 'D', 'O', 'L', 'G',
                 'A', 'J', 'I', 'G', 'O', 'L', 'G', 'O')
        t = time.time()
        enc = self.s.normalize_written_units(list(chain))
        elapsed = time.time() - t
        self.assertEqual(tuple(self.s.shape(enc)), chain,
                         "long-chain encoding must round-trip")
        self.assertLess(elapsed, 0.2,
                        f"18-unit chain took {elapsed*1000:.0f}ms (want <200ms)")

    def test_normalize_text_throughput(self):
        """A paragraph of long compound words normalizes well under the old
        ~10-20s: < 5s on a fresh shaper, and each word round-trips.
        长复合词段落必须远快于旧的 ~10-20s:新建 shaper < 5s,且每词保形。"""
        import time
        words = [
            'ᠦᠢᠯᠡᠳᠦᠯᠭᠡᠵᠢᠭᠦᠯᠬᠦ', 'ᠳᠡᠭᠡᠭᠰᠢᠯᠡᠭᠦᠯᠬᠦ',
            'ᠲᠡᠷᠭᠡᠯᠰᠢᠭᠦᠯᠬᠦ', 'ᠪᠦᠯᠬᠦᠮᠳᠡᠰᠦᠭᠡᠢ',
        ]
        t = time.time()
        for w in words:
            norm = self.s.normalize(w)
            self.assertEqual(self.s.shape(norm), self.s.shape(w),
                             f"{w!r} did not round-trip")
        elapsed = time.time() - t
        self.assertLess(elapsed, 5.0,
                        f"long-word batch took {elapsed:.1f}s (want <5s)")


def _split_letters(text):
    """Split a Unicode string into per-letter chunks: each Mongolian letter
    plus any trailing FVS marks; MVS/nirugu/ZWJ are their own chunk.
    把字符串拆成逐字母块:每个蒙古字母 + 其后的 FVS;MVS/nirugu 各自成块。"""
    FVS = {0x180B, 0x180C, 0x180D, 0x180F}
    chunks = []
    for ch in text:
        cp = ord(ch)
        if cp in FVS and chunks:
            chunks[-1] += ch
        else:
            chunks.append(ch)
    return chunks


class TestPrefixStability(unittest.TestCase, _RoundTripBase):
    """
    Prefix stability (user requirement): if word A = word B + suffix and
    their shapes agree on the shared prefix, normalize must encode that
    shared prefix IDENTICALLY — except the single boundary letter whose
    position changes (fina in B -> medi in A).
    前缀稳定:A = B + 后缀 且共享前缀 shape 相同时,normalize 的共享前缀
    编码必须逐字母相同,只有位置翻转的边界字母(B 的 fina -> A 的 medi)可不同。
    """

    @classmethod
    def setUpClass(cls):
        _RoundTripBase.setUpClass.__func__(cls)

    def test_AB_shared_prefix_identical(self):
        # A is the 18-unit compound; B is the 9-unit word that is A's
        # shape-prefix. B's last letter is the boundary (fina in B, medi in
        # A) and is excluded; every earlier letter must match.
        A = ('A', 'O', 'I', 'I', 'L', 'A', 'D', 'O', 'L', 'G',
             'A', 'J', 'I', 'G', 'O', 'L', 'G', 'O')
        B = ('A', 'O', 'I', 'I', 'L', 'A', 'D', 'O', 'L')
        encA = self.s.normalize_written_units(list(A))
        encB = self.s.normalize_written_units(list(B))
        lettersA = _split_letters(encA)
        lettersB = _split_letters(encB)
        # shared region = all of B's letters except its last (the boundary)
        shared = len(lettersB) - 1
        self.assertEqual(
            lettersA[:shared], lettersB[:shared],
            f"shared prefix diverges:\n"
            f"  A={encA!r} letters={lettersA}\n"
            f"  B={encB!r} letters={lettersB}\n"
            f"  A[:{shared}]={lettersA[:shared]}\n"
            f"  B[:{shared}]={lettersB[:shared]}"
        )

    def test_corpus_real_pair_stability(self):
        """
        Over REAL corpus word pairs where shape(B) is a shape-prefix of
        shape(A), the shared region (all of B's letters except the boundary)
        must encode identically. We require >=99% (a handful of deep
        partition/pinning edge cases remain — units like 'Y'/'Sh' whose
        encoding flips fina->medi as the word grows).
        真实语料词对(shape(B) 是 shape(A) 前缀)的共享区编码必须一致,>=99%。
        """
        # Only pure-letter chains: structural tokens (mvs/nirugu/zwj) split
        # words into chains, so feeding them as one chain is meaningless.
        # 只取纯字母 chain:结构 token 会切分 chain,整词直接喂无意义。
        shapes = {}
        for _, aliases in INLINE_CASES:
            for w in _aliases_to_words(aliases):
                if w:
                    sh = tuple(self.s.shape(w))
                    if not any(t in _STRUCTURAL_UNITS for t in sh):
                        shapes[sh] = self.s.normalize_written_units(list(sh))
        for fn in ('core-hud.tsv', 'eac-hud.tsv'):
            for _, aliases, _exp in _load_tsv(fn):
                toks = aliases.split()
                if any(t != 'space' and t not in _ALIAS_TO_CP for t in toks):
                    continue
                for w in _aliases_to_words(aliases):
                    if w:
                        sh = tuple(self.s.shape(w))
                        if not any(t in _STRUCTURAL_UNITS for t in sh):
                            shapes[sh] = self.s.normalize_written_units(list(sh))
        shapeset = set(shapes)
        pairs = viol = 0
        examples = []
        for A in shapeset:
            for m in range(1, len(A)):
                B = A[:m]
                if B not in shapeset:
                    continue
                pairs += 1
                lF = _split_letters(shapes[A])
                lP = _split_letters(shapes[B])
                shared = len(lP) - 1
                if shared > 0 and lF[:shared] != lP[:shared]:
                    viol += 1
                    if len(examples) < 5:
                        examples.append((list(A), list(B), lF, lP))
        rate = (pairs - viol) / pairs if pairs else 1.0
        print(f"\nprefix-stability (real corpus pairs): "
              f"{pairs - viol}/{pairs} = {rate*100:.2f}%")
        if rate < 0.99:
            for A, B, lF, lP in examples:
                print(f"  VIOL A={A} B={B}\n    full={lF}\n    pre ={lP}")
            self.fail(f"prefix-stability {rate*100:.2f}% < 99%")


if __name__ == '__main__':
    unittest.main(verbosity=2)
