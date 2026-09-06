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

from mongol_norm import MongolianShaper, NormalizationFallbackError
# The fallback tests need a shaper whose normalize table is empty (the
# extension's `testing` hook, see python/tests/_support.py).
# 回退测试需要一个 normalize 表为空的 shaper(扩展的 `testing` 钩子,见 python/tests/_support.py)。
from tests._support import empty_table_shaper, needs_testing_hook

# shape_detailed() reports structural tokens (MVS / Nirugu / ZWJ) with these
# code points; the position tests below look at letters only.
# shape_detailed() 用这些码位报告结构 token;位置测试只看字母。
_STRUCTURAL_CPS = frozenset({"U+180E", "U+180A", "U+200D"})

_ALIAS_TO_CP = {
    'a': '\u1820', 'e': '\u1821', 'i': '\u1822', 'o': '\u1823',
    'u': '\u1824', 'oe': '\u1825', 'ue': '\u1826', 'ee': '\u1827',
    'n': '\u1828', 'ng': '\u1829', 'b': '\u182A', 'p': '\u182B',
    'h': '\u182C', 'g': '\u182D', 'm': '\u182E', 'l': '\u182F',
    's': '\u1830', 'sh': '\u1831', 't': '\u1832', 'd': '\u1833',
    'ch': '\u1834', 'j': '\u1835', 'y': '\u1836', 'r': '\u1837',
    'w': '\u1838', 'f': '\u1839', 'k2': '\u183A', 'k': '\u183B',
    'mvs': '\u180E', 'fvs1': '\u180B', 'fvs2': '\u180C',
    'fvs3': '\u180D', 'fvs4': '\u180F', 'nnbsp': '\u202F',
}

def _mgl(s):
    """Build Mongolian text from space-separated aliases, e.g. _mgl("t a l mvs a")."""
    return ''.join(_ALIAS_TO_CP[a] for a in s.split())


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

    # ══════════════════════════════════════════════════════════
    # Step 1 · Chachlag — MVS-triggered suffix forms
    # ══════════════════════════════════════════════════════════

    # 1-1  a/e after MVS → chachlag
    def test_step1_chachlag_tala(self):
        # ᠲᠠᠯ᠎ᠠ  tal + MVS + a → chachlag "Aa"
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs a")),
            ["T", "A", "L", "Mvs", "Aa"],
        )

    def test_step1_chachlag_talayin(self):
        # ᠲᠠᠯ᠎ᠠ᠎ᠶᠢᠨ  two MVS: chachlag on a, particle on y
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs a mvs y i n")),
            ["T", "A", "L", "Mvs", "Aa", "Mvs", "I", "I", "A"],
        )

    # 1-2  a/e after MVS + FVS → default (no chachlag)
    def test_step1_chachlag_mvs_a_fvs_default(self):
        # tal + MVS + a+FVS1 → a gets default (not chachlag)
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs a fvs1")),
            ["T", "A", "L", "Mvs", "A"],
        )

    # ══════════════════════════════════════════════════════════
    # Step 2 · Syllabic — consonant/vowel context rules
    # ══════════════════════════════════════════════════════════

    # 2-1  o/u/oe/ue after initial consonant → marked

    def test_step2_vowel_marked(self):
        # o/u/oe/ue after initial consonant → marked
        self.assertEqual(self.s.shape(_mgl("ch u")),   ["Ch", "O"])       # ᠴᠤ
        self.assertEqual(self.s.shape(_mgl("s ue")),   ["S", "Ue"])       # ᠰᠦ
        self.assertEqual(self.s.shape(_mgl("d u")),    ["D", "O"])        # ᠳᠤ
        self.assertEqual(self.s.shape(_mgl("d ue")),   ["D", "Ue"])       # ᠳᠦ
        self.assertEqual(self.s.shape(_mgl("t ue l")), ["T", "O", "I", "L"])  # ᠲᠦᠯ

    # 2-1b  o/u/oe/ue precedes/follows FVS → default
    def test_step2_vowel_fvs_default(self):
        # FVS on vowel itself → default
        self.assertEqual(self.s.shape(_mgl("h u fvs2")),  ["H", "U"])    # ᠬᠤ᠌
        self.assertEqual(self.s.shape(_mgl("r ue fvs3")), ["R", "U"])    # ᠷᠦ᠍
        # FVS on preceding letter → vowel also default
        self.assertEqual(self.s.shape(_mgl("d fvs1 ue")), ["D", "U"])    # ᠳ᠋ᠦ
        self.assertEqual(self.s.shape(_mgl("d fvs1 u")),  ["D", "U"])    # ᠳ᠋ᠤ

    # 2-1c  oe/ue.fina preceded by h/g(INIT)+FVS2/FVS4 → marked (exception)
    # Per GB/T 25914-2023 表 E.3 (U+182C) / 表 E.4 (U+182D): this rule applies
    # ONLY when h/g is at initial position. UTN #57 and the mongfontbuilder
    # docs (web/docs/hudum.mdx) describe this rule without the init constraint
    # and with reversed "precedes" wording — both are inaccurate.
    # See: https://github.com/Kushim-Jiang/mongfontbuilder/issues/47
    # 仅当 h/g 处于词首位置时触发；UTN #57 与 mongfontbuilder 文档均有误。
    def test_step2_oe_ue_fina_hg_init_fvs_marked(self):
        self.assertEqual(self.s.shape(_mgl("h fvs2 ue")), ["G", "Ue"])   # ᠬ᠌ᠦ
        self.assertEqual(self.s.shape(_mgl("g fvs4 ue")), ["Gx", "Ue"])  # ᠭ᠏ᠦ

    # 2-1d  oe/ue.fina preceded by h/g(MEDI)+FVS2/FVS4 → default (NOT marked)
    # Regression guard: confirms the init-position constraint added per GB/T
    # 25914-2023 表 E.3/E.4. ᠡᠨᠡᠬ᠌ᠦ (enehüu, "this") has h at .medi — it must
    # render the default O form, matching real font output.
    # 反向回归：词中的 h/g + FVS2/FVS4 + ue 不触发 marked，必须返回默认 O 形。
    def test_step2_oe_ue_fina_hg_medi_fvs_default(self):
        self.assertEqual(self.s.shape(_mgl("e n e h fvs2 ue")), ["A", "N", "A", "G", "O"])   # ᠡᠨᠡᠬ᠌ᠦ
        self.assertEqual(self.s.shape(_mgl("e n e g fvs4 ue")), ["A", "N", "A", "Gx", "O"])  # ᠡᠨᠡᠭ᠏ᠦ

    # 2-2  oe/ue medial after consonant cluster from init → marked
    def test_step2_oe_marked_after_cc(self):
        # ᠮᠨᠥᠭᠡ (mnöge) — oe after init m + medi n → marked
        details = self.s.shape_detailed(_mgl("m n oe g e"))
        oe_tok = [d for d in details if d["alias"] == "oe"][0]
        self.assertEqual(oe_tok["condition"], "marked")
        self.assertEqual(self.s.shape(_mgl("m n oe g e")), ["M", "N", "O", "I", "G", "Aa"])  # ᠮᠨᠥᠭᠡ

    # 2-3  d.init before final vowel (no FVS) → marked (Twelve Syllabaries)
    def test_step2_d_marked(self):
        # ᠳᠠ (da) — d.init + a.fina (no FVS) → marked "D".
        # Without the marked rule, d.init default would be "T" (onset).
        self.assertEqual(self.s.shape(_mgl("d a")), ["D", "A"])
        # ᠳᠦ (du) — d.init + u.fina → marked "D"; u.fina renders as "O"
        # because the post-bowed/marked context selects its bowed form.
        self.assertEqual(self.s.shape(_mgl("d u")), ["D", "O"])

    # 2-3a  GB exception: an FVS adjacent to d or to the following vowel
    #       cancels the d.marked rule (cf. mongolian/.fea III.eac.d.marked
    #       `ignore sub @d-hud.init' @hud.vowel @fvs`).
    def test_step2_d_marked_gb_fvs_cancels(self):
        # FVS on the vowel → marked bails → d.init falls to default "T"
        self.assertEqual(self.s.shape(_mgl("d a fvs1")), ["T", "Aa"])

    # 2-3b  d.medi between vowels is NOT covered by d.marked (init-only);
    #       iii2e takes it instead and assigns `onset` → "D" (tooth form).
    #       Locks in the init-gate fix in `_iii2a_d_marked_at`.
    def test_step2_d_medi_intervocalic_onset(self):
        # ᠣᠳᠣ (odu) — o init + d medi (between vowels) + u fina
        # d.medi gets onset → "D", not the devsger default "Dd".
        self.assertEqual(self.s.shape(_mgl("o d u")), ["A", "O", "D", "U"])
        # d.fina (no following vowel) keeps its devsger default "Dd".
        # Raw: the rule tests below pin WHICH UTN #57 rule fired, and for `Dd` and medial
        # `H` / `Hx` the only evidence is the engine's own unit — the public shape() folds
        # all three into `O A` / `A A` / `N N` (see TestDuplicateEncodings at the end of
        # this module, and `_shape_raw`'s docstring).
        # 原始序列:这些规则测试考察的是"哪条规则触发",而 `Dd`/词中 `H`/词中 `Hx` 只在
        # 引擎自身的单元里可见,公开的 shape() 会把它们折叠掉。
        self.assertEqual(self.s._shape_raw(_mgl("o d")), ["A", "O", "Dd"])

    # 2-4  n/j/w before MVS + isolated a/e → chachlag_onset
    def test_step2_chachlag_onset_n(self):
        # ᠰᠠᠢᠨ᠎ᠠ — n before MVS+a → chachlag_onset "N"
        self.assertEqual(
            self.s.shape(_mgl("s a i n mvs a")),
            ["S", "A", "I", "I", "N", "Mvs", "Aa"],
        )
        # ᠬᠤᠷᠸ᠎ᠠ - w before MVS+a → chachlag_onset "U"
        self.assertEqual(
            self.s.shape(_mgl("h o r w mvs a")),
            ["H", "O", "R", "U", "Mvs", "Aa"],
        )
        # ᠵ᠎ᠠ - j before MVS+a → chachlag_onset "I"
        self.assertEqual(
            self.s.shape(_mgl("j mvs a")),
            ["I", "Mvs", "Aa"],
        )
        # ᠡᠵ᠎ᠡ - j.fina before MVS+e.iso → chachlag_onset "I"
        self.assertEqual(
            self.s.shape(_mgl("e j mvs e")),
            ["A", "I", "Mvs", "Aa"],
        )

    # 2-5  h/g before MVS + isolated a → chachlag_onset
    def test_step2_chachlag_onset_g_a(self):
        # ᠶᠠᠪᠤᠭ᠎ᠠ — g before MVS+a → chachlag_onset
        self.assertEqual(
            self.s.shape(_mgl("y a b u g mvs a")),
            ["Y", "A", "B","O", "Hx", "Mvs", "Aa"],
        )
        # ᠬᠠᠪᠬ᠎ᠠ — h before MVS+a → chachlag_onset
        self.assertEqual(
            self.s.shape(_mgl("h a b h mvs a")),
            ["H", "A", "B", "H", "Mvs", "Aa"],
        )

    # 2-6  g before MVS + isolated e → chachlag_onset
    def test_step2_chachlag_onset_g_e(self):
        # ᠡᠭ᠎ᠡ — g before MVS+e → chachlag_onset
        self.assertEqual(
            self.s.shape(_mgl("e g mvs e")),
            ["A", "H", "Mvs", "Aa"],
        )

    # 2-7  n/d before vowel → onset; n/t/d after vowel or before consonant → devsger
    #      (t has no observable onset variant in MNG.json — iii2e still tags it
    #       but the resolver falls back to default, so onset is a visual no-op for t.)
    def test_step2_n_onset(self):
        # ᠠᠨᠠᠷ - n before vowel → onset "N"
        self.assertEqual(
            self.s.shape(_mgl("a n a r"))[2], "N",
        )
        # ᠳᠠᠯᠠ - d.init before vowel → onset "T"
        self.assertEqual(
            self.s.shape(_mgl("d a l a"))[0], "T",
        )
        # ᠠᠨᠳᠠ - d.medi before vowel → onset "D"; n.medi after vowel and before consonant d → devsger "A"
        out = self.s.shape(_mgl("a n d a"))
        self.assertEqual(out[3], "D")  # d.medi onset
        self.assertEqual(out[2], "A")  # n.medi devsger (next is consonant d)

        # ᠪᠠᠨ - n.fina after vowel → devsger "A"
        self.assertEqual(self.s.shape(_mgl("b a n"))[2], "A")

        # ᠳᠠᠳᠭ᠎ᠠ - d.medi after vowel and before consonant g → devsger "Dd" (raw: `Dd` is
        # `O A` in the public shape, which would also shift the indices below)
        out = self.s._shape_raw(_mgl("d a d g mvs a"))
        self.assertEqual(out[2], "Dd")  # d.medi devsger

        # ᠠᠲᠳ - t.medi after vowel a and before consonant d → devsger "T"; d.fina default → devsger "Dd"
        out = self.s._shape_raw(_mgl("a t d"))
        self.assertEqual(out[2], "T")   # t.medi devsger
        self.assertEqual(out[3], "Dd")  # d.fina devsger (default)

    # 2-8  h/g: masculine/feminine context chain
    def test_step2_h_masculine_onset(self):
        # ᠬᠠᠰ (has) — h(QA) before masculine vowel a → masculine_onset "H"
        self.assertEqual(
            self.s.shape(_mgl("h a s")),
            ["H", "A", "S"],
        )

    def test_step2_g_masculine_onset(self):
        # ᠭᠠᠷ (gar) — g(GA) before masculine vowel a → masculine_onset "Hx"
        self.assertEqual(
            self.s.shape(_mgl("g a r")),
            ["Hx", "A", "R"],
        )

    def test_step2_g_feminine(self):
        # ᠭᠡᠷ (ger) — g(GA) before feminine vowel e → feminine "G"
        self.assertEqual(
            self.s.shape(_mgl("g e r")),
            ["G", "A", "R"],
        )

    def test_step2_g_masculine_devsger(self):
        # ᠠᠭ (ag) — g(GA) after masculine vowel a → masculine_devsger "H"
        self.assertEqual(
            self.s.shape(_mgl("a g")),
            ["A", "A", "H"],
        )

    def test_step2_g_feminine_after_fem(self):
        # ᠥᠭ (oeg) — g(GA) after feminine vowel oe → feminine "G"
        self.assertEqual(
            self.s.shape(_mgl("oe g")),
            ["A", "O", "I", "G"],
        )

    # 2-8a  g.medi adjacent to fem vowel → feminine via iii2f main rule
    # (NOT remote — oe sits immediately before g)
    def test_step2_oegn_adjacent_feminine(self):
        # ᠥᠭᠨ - oe (fem) IMMEDIATELY before g → check 4 fires → feminine "G"
        self.assertEqual(self.s.shape(_mgl("oe g n")), ["A", "O", "I", "G", "A"])

    # 2-8c  i + g/h with reachable MASC marker → masculine_devsger "H"
    # Mirrors iii.py's `III.g_h.onset_and_devsger_and_gender.A.MNG` pattern 5
    # (`i + g/h + MASC`). Per preprocessing.A/B/C, MASC ends up immediately
    # after every h/g letter that is preceded by a masc vowel (init/medi)
    # with no fem vowel in between. What follows the g/h does NOT matter —
    # any letters after g/h have their MASC stripped by preprocessing.C,
    # but g/h's own trailing MASC is preserved. All expected values below
    # verified against `DraftNew-Regular.otf` via `hb-shape`.
    def test_step2_g_i_with_marker_masc(self):
        # ᠠᠢᠭ - g.fina, prev=i, masc a reaches g → "H"
        self.assertEqual(self.s.shape(_mgl("a i g")), ["A", "A", "I", "I", "H"])
        # ᠣᠯᠢᠭ - marker threads through l + i → "H"
        self.assertEqual(self.s.shape(_mgl("o l i g")), ["A", "O", "L", "I", "H"])
        # The medial cases below are raw: `H:medi` is `A A` in the public shape.
        # ᠠᠢᠭᠷ - g.medi (r follows); g/h being last is NOT required
        self.assertEqual(self.s._shape_raw(_mgl("a i g r")), ["A", "A", "I", "I", "H", "R"])
        # ᠠᠯᠢᠭᠷ - marker through l + i, then g.medi with r after
        self.assertEqual(self.s._shape_raw(_mgl("a l i g r")), ["A", "A", "L", "I", "H", "R"])
        # ᠠᠢᠭᠷᠡ - even a fem e *after* g doesn't block (e blocks only when
        #          it sits between masc source and g/h, not downstream)
        self.assertEqual(self.s._shape_raw(_mgl("a i g r e")), ["A", "A", "I", "I", "H", "R", "A"])

    # 2-8d  i + g/h with marker BLOCKED or NOT REACHING → pattern 6 → feminine
    def test_step2_g_i_marker_blocked(self):
        # ᠠᠡᠢᠭ - fem e between masc a and g blocks marker → pattern 6 → "G"
        self.assertEqual(self.s.shape(_mgl("a e i g")), ["A", "A", "A", "I", "I", "G"])
        # ᠢᠭ - prev=i but no masc vowel anywhere → pattern 6 → "G"
        self.assertEqual(self.s.shape(_mgl("i g")), ["A", "I", "G"])

    # 2-8c-h  Same scenarios as 2-8c but with h instead of g. h has different
    # observability: h.fina default == "H" == masc_devsger glyph, so h.fina
    # cases CANNOT distinguish "rule fired" from "fell to default" — they
    # all show H either way. The only observable position is h.medi, where
    # default = "G" but masc_devsger = "H". All values verified against
    # `DraftNew-Regular.otf` via hb-shape.
    def test_step2_h_i_with_marker_masc(self):
        # ᠠᠢᠬᠷ - h.medi, prev=i, marker reaches → masc_devsger "H"
        # (Without pattern 5 firing, h.medi would default to "G" — so this
        # test genuinely proves the rule fired.)
        # Raw: `H:medi` is `A A` in the public shape, which cannot tell H from a vowel pair.
        self.assertEqual(self.s._shape_raw(_mgl("a i h r")), ["A", "A", "I", "I", "H", "R"])
        # ᠠᠢᠬ - h.fina, prev=i, marker reaches → masc_devsger "H" (same as default)
        self.assertEqual(self.s.shape(_mgl("a i h")), ["A", "A", "I", "I", "H"])

    # 2-8d-h  h with prev=i but blocked/no masc — KEY DIFFERENCE from g:
    # iii2f.A pattern 6 (i+g → feminine) covers ONLY g, NOT h. So when
    # the marker doesn't reach h, h falls through to its DEFAULT, not to
    # feminine. h.fina default = "H" (looks identical to masc); h.medi
    # default = "G" (which is what feminine would be anyway, but for a
    # different reason — fall-through, not pattern 6).
    def test_step2_h_i_marker_blocked(self):
        # ᠠᠡᠢᠬᠷ - fem e blocks marker; h.medi falls to default "G"
        self.assertEqual(self.s.shape(_mgl("a e i h r")), ["A", "A", "A", "I", "I", "G", "R"])
        # ᠠᠡᠢᠬ - h.fina default = "H" (no pattern 6 for h, no feminine variant)
        # Contrast: same shape with g would give "G" (pattern 6 fires for g).
        self.assertEqual(self.s.shape(_mgl("a e i h")), ["A", "A", "A", "I", "I", "H"])
        # ᠢᠬ - no masc anywhere; h.fina default = "H" (NOT G like `i g`)
        self.assertEqual(self.s.shape(_mgl("i h")), ["A", "I", "H"])

    # 2-8e-h  h with prev≠i — same logic as g but observable on h.medi.
    def test_step2_h_non_i_prev_no_remote(self):
        # ᠣᠯᠬ - h.fina default = "H" (looks like masc, but no rule fired)
        self.assertEqual(self.s.shape(_mgl("o l h")), ["A", "O", "L", "H"])
        # ᠣᠯᠬᠷ - h.medi default = "G" (genuinely shows no rule fired)
        self.assertEqual(self.s.shape(_mgl("o l h r")), ["A", "O", "L", "G", "R"])

    # 2-8e  prev letter is NOT i (e.g. consonant) — iii2f.A doesn't fire even
    # if a masc vowel is reachable. This is the strict prev=i gate that
    # distinguishes our impl from the broader "remote harmony" prose in
    # the web docs (`mongfontbuilder/web/docs/hudum.mdx:189-220`).
    def test_step2_g_non_i_prev_no_remote(self):
        # ᠣᠯᠭ - prev=l → default "G" (NOT H, despite remote masc o)
        self.assertEqual(self.s.shape(_mgl("o l g")), ["A", "O", "L", "G"])
        # ᠠᠯᠭᠷ - prev=l, g.medi with r after → default "G"
        self.assertEqual(self.s.shape(_mgl("a l g r")), ["A", "A", "L", "G", "R"])

    # 2-8f  Web docs (mongfontbuilder/web/docs/hudum.mdx:189-220) describe two
    # additional remote-harmony rules that are NOT implemented in iii.py and
    # NOT exhibited by DraftNew-Regular.otf. Document this divergence here:
    #
    #   Doc rule (5): "g/h remotely follows fem vowel without blocking masc
    #                  → feminine"
    #     Not implementable observably — g.fina/g.medi/h.medi default glyph
    #     is already G (the feminine form), and h.fina has no feminine
    #     variant (falls back to default H). Our default-fallthrough yields
    #     the same glyph as the doc-prescribed feminine, so this divergence
    #     is invisible in output. Recorded here for future reference.
    #
    #   Doc rule (7): "g/h remotely precedes masc vowel without blocking fem
    #                  → masculine_devsger"
    #     OBSERVABLY divergent from the font: `g r a` should be `Hx` per
    #     docs (g.init.masculine_onset/devsger), but DraftNew renders G.
    #     Reason: iii2f.B fires first (g.init + consonant → feminine),
    #     and the implementation has no rule that "looks ahead remotely"
    #     for a masc vowel after the consonant context. We mirror the
    #     implementation, not the docs.
    def test_step2_gh_remote_doc_rules_NOT_implemented(self):
        # Doc rule (5): remotely follows fem — output is G either way (default
        # equals feminine glyph), so this is just a tripwire confirming the
        # default form is what we expect.
        self.assertEqual(self.s.shape(_mgl("e l g")), ["A", "L", "G"])
        self.assertEqual(self.s.shape(_mgl("e l g r")), ["A", "L", "G", "R"])
        # Doc rule (7): remotely precedes masc — iii2f.B forces feminine "G",
        # NOT the masculine_onset "Hx" that web docs describe. Verified
        # against DraftNew-Regular.otf via hb-shape:
        #   `g r a`  → u182D.G.init  (NOT u182D.Hx.init)
        #   `h r a`  → u182C.G.init  (NOT u182C.H.init)
        # Contrast `g a` (adjacent masc vowel) which correctly fires
        # masculine_onset → "Hx".
        self.assertEqual(self.s.shape(_mgl("g r a")), ["G", "R", "A"])
        self.assertEqual(self.s.shape(_mgl("h r a")), ["G", "R", "A"])
        # Adjacency control: g + masc a (no consonant between) → "Hx"
        self.assertEqual(self.s.shape(_mgl("g a")), ["Hx", "A"])

    # 2-8g  g.init / h.init + consonant → feminine (iii2f.B). Last in the
    # 2-8 series — this is the simplest "no adjacent vowel, init position"
    # fallback rule, kept here as a clean trailing case after all the
    # marker-propagation and doc-divergence subtleties above.
    def test_step2_gh_init_before_consonant_feminine(self):
        # g.init + r → iii2f.B → feminine "G"
        self.assertEqual(self.s.shape(_mgl("g r")), ["G", "R"])
        # h.init + r → iii2f.B → feminine "G"
        self.assertEqual(self.s.shape(_mgl("h r")), ["G", "R"])

    # 2-9  t before ee or consonant → devsger
    def test_step2_t_devsger_before_ee(self):
        # ᠠᠲᠧᠨ (ateen) — t.medi before ee → iii2g.t.devsger fires → "T"
        # (NOT iii2e onset — see iii2e block comment for the carve-out.)
        # Verified against DraftNew-Regular.otf: u1832.T.medi (devsger T).
        details = self.s.shape_detailed(_mgl("a t ee n"))
        t_tok = [d for d in details if d["alias"] == "t"][0]
        self.assertEqual(t_tok["condition"], "devsger")
        self.assertEqual(t_tok["written"], ["T"])
        # Full shape regression: D form would be the buggy output
        self.assertEqual(self.s.shape(_mgl("a t ee n")), ["A", "A", "T", "W", "A"])
        # Control: t + non-ee vowel still goes onset → falls to default "D"
        # (iii2e fires; iii2g doesn't)
        self.assertEqual(self.s.shape(_mgl("a t i n")), ["A", "A", "D", "I", "A"])
        # Control: t + consonant → iii2g.t.devsger fires (consonant branch)
        # → "T". Here iii2e also yields devsger via prev=vowel a, so iii2g
        # is technically redundant; the test still locks in T as the output.
        self.assertEqual(self.s.shape(_mgl("a t r")), ["A", "A", "T", "R"])
        # Control: d + ee still goes iii2e onset (carve-out is t-only)
        # d.medi.onset = D (same as default) — visually identical
        self.assertEqual(self.s.shape(_mgl("a d ee n")), ["A", "A", "D", "W", "A"])

    # 2-10  sh: dotless. iii2g.sh.dotless has two clauses (iii.py:754-763):
    #   (1) sh.init + i.medi          → dotless
    #   (2) sh.medi + i.{medi, fina}  → dotless
    # All expected values verified against `DraftNew-Regular.otf` via
    # `hb-shape --unicodes=1831,...` (note: U+1831 is sh, not U+1830 s —
    # the font uses `u1831.Sh.init` for the dotted default and
    # `u1831.S.init` for the dotless variant).
    def test_step2_sh_dotless(self):
        # ᠰᠢᠮ - sh.init + i.medi → clause 1 → "S"
        self.assertEqual(self.s.shape(_mgl("sh i m")), ["S", "I", "M"])
        # ᠰᠢᠰᠢ - one input exercises BOTH clauses:
        #   1st sh.init + i.medi (followed by 2nd sh) → clause 1 → "S"
        #   2nd sh.medi + i.fina (i is last)          → clause 2 → "S"
        self.assertEqual(self.s.shape(_mgl("sh i sh i")), ["S", "I", "S", "I"])
        # ᠠᠰᠢ - sh.medi + i.fina → clause 2 → "S"
        self.assertEqual(self.s.shape(_mgl("a sh i")), ["A", "A", "S", "I"])
        # Negative controls — neither clause fires, sh stays default "Sh":
        # ᠰᠢ - sh.init + i.FINA (only 2 letters, so i is fina not medi)
        self.assertEqual(self.s.shape(_mgl("sh i")), ["Sh", "I"])
        # ᠰᠠ - sh.init + non-i vowel
        self.assertEqual(self.s.shape(_mgl("sh a")), ["Sh", "A"])
        # ᠰᠧ - sh.init + ee (ee is not in the rule's input set)
        self.assertEqual(self.s.shape(_mgl("sh ee")), ["Sh", "W"])

    # 2-11  g: dotless. iii2g.g.dotless (iii.py:764-776) has two precise
    # sub-rules — both require `s/d` before g — and OVERRIDES whatever
    # condition iii2f.h_g.harmony or iii2c.chachlag_onset set earlier
    # (the OpenType class-membership trick). All values verified against
    # `DraftNew-Regular.otf` via `hb-shape --unicodes=...`.
    #
    # Rule (1):  s/d + g.medi + masc vowel  → dotless "H"
    #            (override target: iii2f masculine_onset "Hx")
    # Rule (2):  s/d + g.fina + MVS + chachlag a.isol  → dotless "H"
    #            (override target: iii2c chachlag_onset "Hx")
    def test_step2_g_dotless(self):
        # Rule 1 — g.medi + masc vowel (raw: the dotless "H" is medial, so the public
        # shape folds it to `A A`):
        self.assertEqual(self.s._shape_raw(_mgl("s g a")), ["S", "H", "A"])
        self.assertEqual(self.s._shape_raw(_mgl("d g a")), ["T", "H", "A"])
        self.assertEqual(self.s._shape_raw(_mgl("a s g a")), ["A", "A", "S", "H", "A"])
        # Rule 2 — g.fina + MVS + chachlag a:
        self.assertEqual(self.s.shape(_mgl("s g mvs a")), ["S", "H", "Mvs", "Aa"])
        self.assertEqual(self.s.shape(_mgl("d g mvs a")), ["T", "H", "Mvs", "Aa"])

        # Negative: rule 1 needs masc vowel — fem/neut/consonant don't fire
        self.assertEqual(self.s.shape(_mgl("s g i")), ["S", "G", "I"])   # neut i
        self.assertEqual(self.s.shape(_mgl("s g e")), ["S", "G", "Aa"])  # fem e (iii2f → feminine = G)
        self.assertEqual(self.s.shape(_mgl("s g n")), ["S", "G", "A"])   # consonant n
        # Negative: rule 2 needs MVS + chachlag a — bare g.fina doesn't fire
        self.assertEqual(self.s.shape(_mgl("s g")), ["S", "G"])
        # Negative: prev letter must be s or d
        self.assertEqual(self.s._shape_raw(_mgl("a g a")), ["A", "A", "Hx", "A"])    # iii2f masc_onset (`Hx:medi`)
        self.assertEqual(self.s.shape(_mgl("n g mvs a")), ["N", "Hx", "Mvs", "Aa"])  # iii2c chachlag_onset

    # ══════════════════════════════════════════════════════════
    # Step 3 · Particle — MVS particle dictionary lookup
    # ══════════════════════════════════════════════════════════
    #
    # Per `_PARTICLE_TARGET_ALIASES` in rules.py, only the following 7
    # letters can receive the `particle` condition (in this order):
    #   a, e, i, u, ue, d, y
    # Tests are organized by which TARGET letter they exercise. A test
    # that triggers particle on multiple targets (e.g., d+u or i+y) is
    # listed under the target that comes FIRST in the order above.

    # 3-1a  TARGET = a
    def test_step3_particle_acha(self):
        # tal + MVS + acha — a.init at idx 1 → particle (e, d, y not targeted here)
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs a ch a")),
            ["T", "A", "L", "Mvs", "A", "Ch", "A"],
        )

    # 3-1b  TARGET = e
    # The particle dict has NO entry where e sits at a particle index —
    # e appears in some particles (e.g. "mvs i y e n", "mvs d e g e n")
    # but always at indices 3+ (non-particle positions). The rule never
    # observably tags e as `particle`. Listed here for completeness;
    # the iyen/iyer/degen tests below double as negative coverage for e.

    # 3-1c  TARGET = i
    def test_step3_particle_i(self):
        # tal + MVS + i — i.isol at idx 1 → particle
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs i")),
            ["T", "A", "L", "Mvs", "I"],
        )

    def test_step3_particle_iyar(self):
        # tal + MVS + iyar — i + y at idx 1, 2 → both particle (masc vowel)
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs i y a r")),
            ["T", "A", "L", "Mvs", "I", "I", "A", "R"],
        )

    def test_step3_particle_iyer(self):
        # tal + MVS + iyer — fem-vowel pair of iyar; identical shape because
        # e.medi default = "A" same as a.medi default
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs i y e r")),
            ["T", "A", "L", "Mvs", "I", "I", "A", "R"],
        )

    def test_step3_particle_iyen(self):
        # tal + MVS + iyen — i,y particles; e at idx 3 stays default (not in [1,2])
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs i y e n")),
            ["T", "A", "L", "Mvs", "I", "I", "A", "A"],
        )

    # 3-1d  TARGET = u
    def test_step3_particle_u(self):
        # tal + MVS + u — u.isol at idx 1 → particle
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs u")),
            ["T", "A", "L", "Mvs", "U"],
        )

    def test_step3_particle_du(self):
        # tal + MVS + du — d at idx 1 AND u at idx 2 both particles
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs d u")),
            ["T", "A", "L", "Mvs", "D", "U"],
        )

    # 3-1e  TARGET = ue
    def test_step3_particle_ue(self):
        # tal + MVS + ue — ue.isol at idx 1 → particle "U"
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs ue")),
            ["T", "A", "L", "Mvs", "U"],
        )

    def test_step3_particle_uen(self):
        # tal + MVS + ue+n — ue.init particle "O", n.fina devsger "A"
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs ue n")),
            ["T", "A", "L", "Mvs", "O", "A"],
        )

    def test_step3_particle_ued(self):
        # tal + MVS + ue+d — ue.init particle "O", d.fina devsger "Dd"
        # (raw: the public shape is `T A L Mvs O O A`)
        self.assertEqual(
            self.s._shape_raw(_mgl("t a l mvs ue d")),
            ["T", "A", "L", "Mvs", "O", "Dd"],
        )

    def test_step3_particle_duer(self):
        # tal + MVS + duer — d at idx 1 AND ue at idx 2 both particles
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs d ue r")),
            ["T", "A", "L", "Mvs", "D", "O", "R"],
        )

    # 3-1f  TARGET = d
    def test_step3_particle_dagan(self):
        # tal + MVS + dagan — d at idx 1 → particle (masc vowel harmony)
        # (raw: `Hx:medi` is `N N` in the public shape)
        self.assertEqual(
            self.s._shape_raw(_mgl("t a l mvs d a g a n")),
            ["T", "A", "L", "Mvs", "D", "A", "Hx", "A", "A"],
        )

    def test_step3_particle_degen(self):
        # tal + MVS + degen — fem-vowel pair of dagan. g.medi here gets
        # "G" (iii2f feminine) instead of "Hx" (masc_onset in dagan)
        # because of the surrounding fem vowel e.
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs d e g e n")),
            ["T", "A", "L", "Mvs", "D", "A", "G", "A", "A"],
        )

    # 3-1g  TARGET = y
    def test_step3_particle_yi(self):
        # tal + MVS + yi — y.init at idx 1 → particle "I"
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs y i")),
            ["T", "A", "L", "Mvs", "I", "I"],
        )

    def test_step3_particle_yin(self):
        # tal + MVS + yin — y.init at idx 1 → particle "I"
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs y i n")),
            ["T", "A", "L", "Mvs", "I", "I", "A"],
        )

    # 3-2  u/ue particles WITHOUT MVS (word-internal: "u u", "ue ue")
    def test_step3_particle_uu(self):
        # ᠤᠤ — "u u" in dict → u.init at idx 0 → particle "O"
        self.assertEqual(self.s.shape(_mgl("u u")), ["O", "U"])

    def test_step3_particle_ueue(self):
        # ᠦᠦ — "ue ue" in dict → ue.init at idx 0 → particle "O"
        self.assertEqual(self.s.shape(_mgl("ue ue")), ["O", "U"])

    # 3-3  Negative: MVS-headed segment NOT in particle dict → no particle
    def test_step3_no_particle_match(self):
        # "mvs l e" — not a dict entry → l and e stay default
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs l e")),
            ["T", "A", "L", "Mvs", "L", "A"],
        )
        # "mvs r" — not a dict entry → r stays default
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs r")),
            ["T", "A", "L", "Mvs", "R"],
        )

    # ══════════════════════════════════════════════════════════
    # Step 4 · Devsger — i.medi after vowel → vowel_devsger (double tooth)
    # ══════════════════════════════════════════════════════════
    #
    # Rule (iii.py iii4): i.medi gets `vowel_devsger` IF the immediately
    # preceding vowel's WRITTEN form (post-FVS substitution) does NOT end
    # with the unit "I". The "consider FVS" wording in the docs is just
    # making explicit that the check uses the FVS-resolved form — same
    # mechanism, one rule. Verified against `DraftNew-Regular.otf` via
    # hb-shape.

    # 4-1  prev vowel does NOT end with "I" → devsger fires
    def test_step4_devsger_ail(self):
        # ᠠᠢᠯ - a.init = "AA" (ends with A) → fires → i.medi.vowel_devsger = "II"
        self.assertEqual(
            self.s.shape(_mgl("a i l")),
            ["A", "A", "I", "I", "L"],
        )

    # 4-2  prev vowel ENDS with "I" (default form) → devsger does NOT fire
    def test_step4_no_devsger_tueil(self):
        # ᠲᠦᠢᠯ - ue.medi default = "OI" (ends with I) → no devsger → i.medi default "I"
        self.assertEqual(
            self.s.shape(_mgl("t ue i l")),
            ["T", "O", "I", "I", "L"],
        )

    # 4-3  prev vowel + FVS — FVS-resolved form STILL ends with "I" → no devsger.
    # Demonstrates the rule looks at the post-FVS form: ue.medi.fvs1 = "OI"
    # still ends with I, so devsger doesn't fire (same outcome as 4-2 but
    # arrived at via the FVS path).
    def test_step4_no_devsger_aueil_fvs1(self):
        # ᠠᠦ᠋ᠢᠯ - a.init "AA" + ue.medi+fvs1 "OI" + i.medi + l.fina
        self.assertEqual(
            self.s.shape(_mgl("a ue fvs1 i l")),
            ["A", "A", "O", "I", "I", "L"],
        )

    # 4-4  FVS on i ITSELF → rule bows out (i's FVS forces a specific variant)
    def test_step4_no_devsger_naima_fvs3(self):
        # ᠨᠠᠢ᠍ᠮᠠ - n + a + i.medi+fvs3 + m + a
        # i has explicit FVS3 → devsger rule skipped → i.medi.fvs3 = "I" (default)
        # Without the FVS3, this would be naima → i.medi devsger = "II".
        self.assertEqual(
            self.s.shape(_mgl("n a i fvs3 m a")),
            ["N", "A", "I", "M", "A"],
        )

    # ══════════════════════════════════════════════════════════
    # Step 5 · Post-bowed — vowels after bowed consonants
    # ══════════════════════════════════════════════════════════
    #
    # Rule (iii.py iii5): vowel.FINA gets `post_bowed` if the preceding
    # consonant renders as a "bowed" written unit. The bowed units are:
    #   bowedB = B, P, F   (letters b, p, f)
    #   bowedK = K, K2     (letters k, k2)
    #   bowedG = G, Gx     (g/h after feminine harmony — `g`/`h` only
    #                       become G/Gx via iii2f.feminine; their default
    #                       Hx/H are NOT bowed)
    # Per-input-set restrictions:
    #   bowedB / bowedK accept: a, e, o, u, oe, ue  (all fina vowels)
    #   bowedG accepts only:    e                   (a after bowedG is
    #                                                 explicitly excluded)
    # All values verified against `DraftNew-Regular.otf` via hb-shape.

    # 5-1  bowedB (b/p/f) + vowel.fina → post_bowed
    def test_step5_post_bowed_after_b(self):
        # ᠪᠠ - a.fina post_bowed → "Aa"
        self.assertEqual(self.s.shape(_mgl("b a")), ["B", "Aa"])
        # ᠪᠡ - e.fina post_bowed → "Aa"
        self.assertEqual(self.s.shape(_mgl("b e")), ["B", "Aa"])
        # ᠪᠣ - o.fina post_bowed → "O" (also iii2a.marked applies; same glyph)
        self.assertEqual(self.s.shape(_mgl("b o")), ["B", "O"])
        # ᠫᠠ - p (bowedB) + a → "Aa"
        self.assertEqual(self.s.shape(_mgl("p a")), ["P", "Aa"])
        # ᠹᠢ - i.fina after F: i is NOT in the post_bowed input set → "I"
        self.assertEqual(self.s.shape(_mgl("f i")), ["F", "I"])

    # 5-2  bowedK (k/k2) + vowel.fina → post_bowed
    def test_step5_post_bowed_after_k(self):
        # ᠻᠠ - k + a.fina → "Aa"
        self.assertEqual(self.s.shape(_mgl("k a")), ["K", "Aa"])
        # ᠺᠡ - k2 + e.fina → "Aa"
        self.assertEqual(self.s.shape(_mgl("k2 e")), ["K2", "Aa"])

    # 5-3  bowedG (g/h with feminine form G/Gx) + e.fina → post_bowed
    # G/Gx only appears when g/h gets `feminine` from iii2f. With masc
    # vowel context (g+a), g becomes Hx instead — NOT bowed.
    def test_step5_post_bowed_after_g(self):
        # ᠭᠡ - g.init + e: g→feminine "G", e.fina post_bowed → "Aa"
        self.assertEqual(self.s.shape(_mgl("g e")), ["G", "Aa"])
        # ᠥᠭᠡ - oe.init + g.medi + e.fina: g→feminine "G", e post_bowed
        self.assertEqual(
            self.s.shape(_mgl("oe g e")),
            ["A", "O", "I", "G", "Aa"],
        )
        # ᠪᠣᠭᠡ - composite: b+o+g.medi+e.fina
        self.assertEqual(
            self.s.shape(_mgl("b o g e")),
            ["B", "O", "G", "Aa"],
        )

    # 5-4  g.init + a (masc): g becomes Hx (masc_onset), NOT G — so a does
    # NOT get post_bowed. Shows the harmony rule's interaction.
    def test_step5_no_post_bowed_g_init_plus_a(self):
        # ᠭᠠ - g.init + masc a → g masc_onset "Hx", a default "A"
        self.assertEqual(self.s.shape(_mgl("g a")), ["Hx", "A"])

    # 5-5  vowel at MEDI position after bowed → no post_bowed
    # post_bowed only applies to .fina vowels. data has no post_bowed
    # variant for medi vowels, so even though our rule may set the
    # condition, the resolver falls back to default (same glyph).
    def test_step5_no_post_bowed_medi(self):
        # ᠪᠠᠯ - a.medi after b → default "A" (NOT post_bowed)
        self.assertEqual(self.s.shape(_mgl("b a l")), ["B", "A", "L"])
        # ᠪᠡᠷ - e.medi after b → default "A"
        self.assertEqual(self.s.shape(_mgl("b e r")), ["B", "A", "R"])
        # ᠭᠡᠷ - g + e.medi + r → e.medi default (NOT post_bowed Aa)
        self.assertEqual(self.s.shape(_mgl("g e r")), ["G", "A", "R"])

    # ══════════════════════════════════════════════════════════
    # Position assignment (observed through shape_detailed(); structural
    # tokens are filtered out so only the letters' positions remain)
    # 位置分配(通过 shape_detailed() 观察;过滤掉结构 token,只看字母)
    # ══════════════════════════════════════════════════════════

    def _letter_positions(self, aliases):
        return [
            detail["position"]
            for detail in self.s.shape_detailed(_mgl(aliases))
            if detail["cp"] not in _STRUCTURAL_CPS
        ]

    def test_position_single_isol(self):
        self.assertEqual(self._letter_positions("n"), ["isol"])

    def test_position_init_fina(self):
        self.assertEqual(self._letter_positions("a b"), ["init", "fina"])

    def test_position_init_medi_fina(self):
        self.assertEqual(self._letter_positions("t a l"), ["init", "medi", "fina"])

    def test_position_mvs_breaks_chain(self):
        # t a l MVS a → [init,medi,fina] + [isol]
        self.assertEqual(
            self._letter_positions("t a l mvs a"),
            ["init", "medi", "fina", "isol"],
        )

    def test_position_double_mvs(self):
        # t a l MVS a MVS y i n → 3 segments
        self.assertEqual(
            self._letter_positions("t a l mvs a mvs y i n"),
            ["init", "medi", "fina", "isol", "init", "medi", "fina"],
        )

    def test_position_of_every_token_including_mvs(self):
        # The MVS token itself is reported too (as an isolated token).
        # MVS token 本身也会被报告(isol)。
        details = self.s.shape_detailed(_mgl("t a l mvs a"))
        self.assertEqual(
            [(d["cp"], d["position"]) for d in details],
            [("U+1832", "init"), ("U+1820", "medi"), ("U+182F", "fina"),
             ("U+180E", "isol"), ("U+1820", "isol")],
        )

    # ══════════════════════════════════════════════════════════
    # NNBSP → MVS normalization
    # ══════════════════════════════════════════════════════════

    def test_nnbsp_produces_same_shape(self):
        mvs_text = _mgl("t a l mvs a mvs y i n")
        nnbsp_text = _mgl("t a l nnbsp a nnbsp y i n")
        self.assertEqual(self.s.shape(nnbsp_text), self.s.shape(mvs_text))

    # ══════════════════════════════════════════════════════════
    # UTN-vs-EAC divergences  (UTN57 ↔ GB/T 25914 EAC 分歧)
    # ══════════════════════════════════════════════════════════
    #
    # GB/T 25914-2023 (EAC) and the UTN57 model disagree on a small
    # number of edge cases. mongfontbuilder follows UTN57 and marks the
    # EAC counter-examples as `pytest.mark.xfail`
    # (mongfontbuilder/tests/test_font.py:42-69); we follow UTN too and
    # excludethe matching rows from `test_eac_hud.py` (see
    # `_UTN_XFAIL_CASES` there). The cases below pin down the
    # UTN-correct shaping that the EAC suite cannot.
    #
    # GB/T 25914-2023 (EAC) 与 UTN57 模型在少数边缘情况上有分歧。
    # mongfontbuilder 遵循 UTN57 并在自己的测试里把这几条 EAC 反例
    # 标为 `pytest.mark.xfail`；我们也遵循 UTN, 并在
    # `test_eac_hud.py` 的 `_UTN_XFAIL_CASES` 里跳过对应行。下面这些
    # 测试用例钉死了 EAC 套件无法表达的 UTN 正确行为。

    # ── A. FVS attached to pre-MVS letter blocks chachlag_onset ────
    # iii.py iii2c (mongfontbuilder): user FVS on h/g/n/j/w
    # immediately before `MVS + a/e.isol` SUPPRESSES the chachlag_onset
    # substitution, so the letter keeps its user-chosen FVS form.
    # iii.py iii2c (mongfontbuilder): h/g/n/j/w 紧邻 `MVS + a/e.isol`
    # 前面若挂了用户 FVS, 则抑制 chachlag_onset 替换, 字母保持 FVS
    # 指定的形态。
    # (counter-example: EAC XIM11-1012 expects `Hx` here.)

    def test_utn_g_fvs2_blocks_chachlag_onset(self):
        # `b a g fvs2 mvs a` — UTN: g.fvs2=G (feminine, user wins).
        # `b a g fvs2 mvs a` — UTN: g.fvs2=G (阴性变体, 用户胜)。
        # EAC counter-example: expects Hx (chachlag_onset).
        self.assertEqual(
            self.s.shape(_mgl("b a g fvs2 mvs a")),
            ["B", "A", "G", "Mvs", "Aa"],
        )

    def test_utn_g_fvs3_picks_chachlag_onset(self):
        # `b a g fvs3 mvs a` — FVS3 IS the chachlag_onset slot for
        # g.fina (variants: fvs3→Hx, conditions=[chachlag_onset]). User
        # and rule agree → Hx fires the UTN-correct way.
        # `b a g fvs3 mvs a` — FVS3 正好是 g.fina chachlag_onset 槽位,
        # 用户和规则一致 → Hx 按 UTN 正确路径触发。
        self.assertEqual(
            self.s.shape(_mgl("b a g fvs3 mvs a")),
            ["B", "A", "Hx", "Mvs", "Aa"],
        )

    # ── B. NNBSP is equivalent to MVS (UTN: "old NNBSP function") ──
    # iii.py keeps NNBSP in the `mvs` glyph class so chachlag /
    # particle / mvs.narrow / mvs.wide all fire as if it were MVS. The
    # EAC spec wants NNBSP to disable every shaping feature; UTN
    # explicitly rejects that (mongfontbuilder xfail reason:
    # "the old functionality of NNBSP should be retained").
    # iii.py 把 NNBSP 放在 `mvs` glyph 类里, chachlag/particle/
    # mvs.narrow/mvs.wide 全部按 MVS 触发。EAC 期望 NNBSP 禁用所有整形
    # 特性, 但 UTN 明文拒绝 ("the old functionality of NNBSP should
    # be retained")。
    # (counter-examples: EAC XIM11-38/39/40/41.)

    def test_utn_nnbsp_alone_renders_as_mvs(self):
        # Standalone NNBSP — UTN renders the MVS slot; EAC wants empty.
        # 单独的 NNBSP — UTN 渲染 MVS 槽位; EAC 期望空。
        self.assertEqual(self.s.shape(_mgl("nnbsp")), ["Mvs"])

    def test_utn_nnbsp_triggers_chachlag(self):
        # `b a g nnbsp a` — UTN treats NNBSP as MVS, so the trailing
        # a.isol triggers chachlag (Aa) and g.fina takes chachlag_onset
        # (Hx). EAC XIM11-39 wants `B A H A A` (no chachlag).
        # `b a g nnbsp a` — UTN 把 NNBSP 当 MVS, 尾部 a.isol 触发
        # chachlag (Aa), g.fina 取 chachlag_onset (Hx)。EAC XIM11-39
        # 期望 `B A H A A` (不触发 chachlag)。
        self.assertEqual(
            self.s.shape(_mgl("b a g nnbsp a")),
            ["B", "A", "Hx", "Mvs", "Aa"],
        )

    def test_utn_nnbsp_triggers_particle(self):
        # `a b u nnbsp y i n` — UTN: particle dict matches `mvs y i n`
        # (NNBSP ≡ MVS) → y.init = I (particle.fvs1 form).
        # EAC XIM11-40 wants `A A B O Y I A` (no particle).
        # `a b u nnbsp y i n` — UTN: particle 词典匹配 `mvs y i n`
        # (NNBSP ≡ MVS) → y.init = I (particle.fvs1 形态)。
        # EAC XIM11-40 期望 `A A B O Y I A` (不触发 particle)。
        self.assertEqual(
            self.s.shape(_mgl("a b u nnbsp y i n")),
            ["A", "A", "B", "O", "Mvs", "I", "I", "A"],
        )

    def test_utn_nnbsp_renders_mvs_token(self):
        # `a b u nnbsp e j i` — UTN emits the `mvs` separator between
        # the two words; EAC XIM11-41 wants no separator.
        # `a b u nnbsp e j i` — UTN 在两个词之间发出 `mvs` 分隔符;
        # EAC XIM11-41 期望无分隔符。
        self.assertEqual(
            self.s.shape(_mgl("a b u nnbsp e j i")),
            ["A", "A", "B", "O", "Mvs", "A", "J", "I"],
        )

    # ══════════════════════════════════════════════════════════
    # Edge cases
    # ══════════════════════════════════════════════════════════

    def test_single_vowel(self):
        self.assertEqual(self.s.shape(_mgl("a")), ["A", "A"])

    def test_empty_string(self):
        self.assertEqual(self.s.shape(""), [])

    def test_oron(self):
        # ᠣᠷᠣᠨ (oron)
        self.assertEqual(
            self.s.shape(_mgl("o r o n")),
            ["A", "O", "R", "O", "A"],
        )

    def test_mori(self):
        # ᠮᠣᠷᠢ (mori)
        self.assertEqual(
            self.s.shape(_mgl("m o r i")),
            ["M", "O", "R", "I"],
        )


class TestDuplicateEncodings(unittest.TestCase):
    """
    The other side of every `_shape_raw` assertion in TestShape: what the public
    shape() says about the nine written units that render as exactly the same ink as a
    sequence of other units.
    TestShape 中每个 `_shape_raw` 断言的另一面:公开的 shape() 如何处理那九个
    与另一串单元墨迹完全相同的书写单元。
    """

    # (what the engine calls it, spelling A, spelling B) — the two spellings render the
    # same ink, so the public shape must be one sequence and normalize one text.
    PAIRS = [
        ("Dd:fina = O A",  "ᠠᠷᠠᠳ",       "ᠠᠷᠠᠤᠠ"),
        ("Dd:medi = O A",  "ᠣᠳᠪᠣ",       "ᠠ᠋ᠣᠣᠠᠪᠣ᠋"),
        ("H:medi  = A A",  "ᠪᠠᠭᠰᠢ",      "ᠪᠠᠠᠠᠰᠢ"),
        ("Hx:medi = N N",  "ᠠᠷᠭᠠᠯ",      "ᠠ᠋ᠠᠷᠨ᠋ᠨ᠋ᠠᠯ"),
        ("Cr:init = O O",  "ᡂ᠊",         "ᠤ᠋ᠤ᠊"),
        ("A:isol  = A Aa", "ᠡ",          "ᠡᠠ᠋"),
        ("Aa:fina = A Aa", "ᠪᠠ",         "ᠪᠠᠠ᠋"),
        ("B2:fina = O Aa", "᠊ᠪ᠋",        "᠊ᠤᠠ᠋"),
        ("G:fina  = I Aa", "᠊ᠭ",         "᠊ᠢᠠ᠋"),
    ]

    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper(locale="MNG")

    def test_every_duplicate_pair_unifies(self):
        for name, a, b in self.PAIRS:
            with self.subTest(pair=name):
                self.assertEqual(self.s.shape(a), self.s.shape(b))
                self.assertTrue(self.s.same_shape(a, b))
                self.assertEqual(self.s.normalize(a), self.s.normalize(b))
                # The engine still tells the two apart — that is what the conformance
                # suites check, and it is why `_shape_raw` exists.
                self.assertNotEqual(self.s._shape_raw(a), self.s._shape_raw(b))

    def test_expanded_duplicates(self):
        # The single unit becomes the pair.
        self.assertEqual(self.s.shape("ᠠᠷᠠᠳ"), ["A", "A", "R", "A", "O", "A"])   # Dd:fina
        self.assertEqual(self.s.shape("ᠣᠳᠪᠣ"), ["A", "O", "O", "A", "B", "O"])   # Dd:medi
        self.assertEqual(self.s.shape("ᠪᠠᠭᠰᠢ"), ["B", "A", "A", "A", "S", "I"])  # H:medi
        self.assertEqual(self.s.shape("ᠠᠷᠭᠠᠯ"),
                         ["A", "A", "R", "N", "N", "A", "L"])                     # Hx:medi
        self.assertEqual(self.s.shape("ᡂ᠊"), ["O", "O", "Nirugu"])               # Cr:init

    def test_contracted_duplicates(self):
        # Choose the shorter form only in its verified position and bowed written unit context.
        self.assertEqual(self.s.shape("ᠡᠠ᠋"), ["A"])                # A:isol
        self.assertEqual(self.s.shape("ᠪᠠᠠ᠋"), ["B", "Aa"])         # Aa:fina
        self.assertEqual(self.s.shape("᠊ᠤᠠ᠋"), ["Nirugu", "B2"])    # B2:fina
        self.assertEqual(self.s.shape("᠊ᠢᠠ᠋"), ["Nirugu", "G"])     # G:fina

    def test_aa_contraction_requires_immediate_bowed_unit(self):
        for bowed_unit in ("B", "P", "F", "G", "Gx", "K", "K2"):
            for units, expected in (
                ([bowed_unit, "A", "Aa"], [bowed_unit, "Aa"]),
                ([bowed_unit, "A", "A", "Aa"], [bowed_unit, "A", "A", "Aa"]),
                ([bowed_unit, "H", "Aa"], [bowed_unit, "A", "A", "Aa"]),
            ):
                with self.subTest(units=units):
                    text = self.s.normalize_written_units(units)
                    self.assertEqual(self.s.shape(text), expected)
                    self.assertEqual(self.s.normalize(text), text)
        for aliases, expected in (
            ("n a a fvs1", ["N", "A", "Aa"]),
            ("b a a a fvs1", ["B", "A", "A", "Aa"]),
        ):
            with self.subTest(aliases=aliases):
                self.assertEqual(self.s.shape(_mgl(aliases)), expected)
        self.assertFalse(self.s.same_shape(_mgl("b a"), _mgl("b a a a fvs1")))

    def test_forms_outside_a_verified_pair_are_left_alone(self):
        self.assertEqual(self.s.shape(_mgl("h o d a")), ["H", "O", "D", "A"])      # H:init
        self.assertEqual(self.s.shape(_mgl("a i g")), ["A", "A", "I", "I", "H"])   # H:fina
        self.assertEqual(self.s.shape(_mgl("n g mvs a")),
                         ["N", "Hx", "Mvs", "Aa"])                     # Hx:fina, Aa:isol
        self.assertEqual(self.s.shape("ᡂ"), ["Cr"])       # Cr:isol, not the verified init
        self.assertEqual(self.s.shape(_mgl("a")), ["A", "A"])  # a lone `A` chain is canonical

    def test_dd_never_reaches_a_public_shape(self):
        # `Dd` is a duplicate in every position it has, so it can never appear.
        for aliases in ("o d", "o d b o", "t a l mvs ue d", "a r a d", "d a d h u"):
            with self.subTest(aliases=aliases):
                self.assertNotIn("Dd", self.s.shape(_mgl(aliases)))
                self.assertIn("Dd", self.s._shape_raw(_mgl(aliases)))

    def test_expansion_does_not_license_a_non_bowed_unit_contraction(self):
        # Dd expands to O A; O is not a bowed written unit, so A Aa must remain intact.
        word = _mgl("t e d fvs2 e fvs1")
        self.assertEqual(self.s._shape_raw(word), ["T", "A", "Dd", "Aa"])
        self.assertEqual(self.s.shape(word), ["T", "A", "O", "A", "Aa"])
        # Nothing oscillates: re-encoding the public shape and reshaping is a fixed point.
        units = self.s.shape(word)
        self.assertEqual(self.s.shape(self.s.normalize_written_units(units)), units)

    def test_collapse_is_a_fixed_point_for_every_corpus_shape(self):
        # Idempotence where it matters: the public shape of a real word, fed back through
        # the written-unit encoder, reshapes to itself.
        from tests.test_canonical_golden import _all_corpus_words
        for word in dict.fromkeys(_all_corpus_words()):
            units = self.s.shape(word)
            self.assertEqual(
                self.s.shape(self.s.normalize_written_units(units)), units,
                f"collapse is not a fixed point for {word!r}",
            )


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
        self.assertFalse(self.s.same_shape("ᠰᠠᠢᠨ", "ᠨᠠᠢ᠍ᠮᠠ"))

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

    # Canonical "sain" under the FVS-pinned per-unit encoder: all five
    # encodings shape to ['S','A','I','I','A'] and converge to ONE output.
    # The encoder pins each unit to a context-independent (letter, fvs):
    # s, a, i+fvs3, i+fvs3, a — so prefixes encode stably regardless of
    # what follows (medial 'I' needs fvs3 to be context-independent; that
    # FVS clutter is the deliberate price of prefix-stability).
    # 每个单元钉死为 context 无关编码,故前缀稳定;中位 I 需 fvs3 才 context
    # 无关,FVS 杂讯是前缀稳定的代价。
    CANONICAL_SAIN = "ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌"

    @needs_testing_hook
    def test_non_strict_mode_preserves_input_when_canonicalization_falls_back(self):
        word = "ᠰᠠᠢᠨ"
        shaper = empty_table_shaper()
        self.assertEqual(shaper.normalize(word, strict=False), word)

    @needs_testing_hook
    def test_default_mode_raises_when_canonicalization_falls_back(self):
        word = "ᠰᠠᠢᠨ"
        shaper = empty_table_shaper()
        with self.assertRaisesRegex(
                NormalizationFallbackError,
                "no canonical encoding for written units S\\+A\\+I\\+I\\+A") as raised:
            shaper.normalize(word)

        self.assertEqual(raised.exception.text, word)
        self.assertEqual(raised.exception.written_units, ("S", "A", "I", "I", "A"))

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
        for word in ["ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ", "ᠨᠠᠢ᠍ᠮᠠ", "ᠣᠷᠣᠨ"]:
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

    # Canonical "sain" under the FVS-pinned per-unit encoder (see
    # TestNormalize for the rationale). All variants converge to one output.
    CANONICAL_SAIN = "ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌"

    @needs_testing_hook
    def test_default_mode_reports_a_fallback_inside_mixed_text(self):
        text = "Hello ᠰᠠᠢᠨ world"
        shaper = empty_table_shaper()
        with self.assertRaises(NormalizationFallbackError) as raised:
            shaper.normalize_text(text)
        self.assertEqual(raised.exception.text, "ᠰᠠᠢᠨ")

    @needs_testing_hook
    def test_non_strict_mode_preserves_a_fallback_inside_mixed_text(self):
        text = "Hello ᠰᠠᠢᠨ world"
        shaper = empty_table_shaper()
        self.assertEqual(shaper.normalize_text(text, strict=False), text)

    def test_single_word_matches_normalize(self):
        # normalize_text on a single word should match normalize
        # 单词的 normalize_text 应与 normalize 一致
        for word in ["ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ", "ᠨᠠᠢ᠍ᠮᠠ", "ᠣᠷᠣᠨ"]:
            self.assertEqual(self.s.normalize_text(word), self.s.normalize(word))

    def test_two_words_space_separated(self):
        # Two Mongolian words separated by a space
        # 空格分隔的两个蒙古文词
        text = "ᠰᠡᠢᠨ ᠨᠠᠢ᠍ᠮᠠ"
        result = self.s.normalize_text(text)
        expected = self.s.normalize("ᠰᠡᠢᠨ") + " " + self.s.normalize("ᠨᠠᠢ᠍ᠮᠠ")
        self.assertEqual(result, expected)

    def test_space_preserved(self):
        # Spaces must be preserved exactly
        # 空格必须精确保留
        text = "ᠰᠠᠢᠨ  ᠨᠠᠢ᠍ᠮᠠ"  # double space
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
        text = "ᠰᠡᠢᠨ, ᠨᠠᠢ᠍ᠮᠠ!"
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
        text = "ᠰᠡᠢᠨ ᠨᠠᠢ᠍ᠮᠠ"
        n1 = self.s.normalize_text(text)
        n2 = self.s.normalize_text(n1)
        self.assertEqual(n1, n2)

    def test_numbers_preserved(self):
        # Numbers mixed with Mongolian text stay unchanged
        # 数字与蒙古文混合时保持不变
        text = "ᠰᠡᠢᠨ 123 ᠨᠠᠢ᠍ᠮᠠ"
        result = self.s.normalize_text(text)
        self.assertIn("123", result)
        self.assertEqual(
            result,
            self.s.normalize("ᠰᠡᠢᠨ") + " 123 " + self.s.normalize("ᠨᠠᠢ᠍ᠮᠠ"),
        )

    def test_symbols_preserved(self):
        # Symbols (@, #, etc.) mixed with Mongolian text stay unchanged
        # 符号与蒙古文混合时保持不变
        text = "#ᠰᠡᠢᠨ @world"
        result = self.s.normalize_text(text)
        self.assertTrue(result.startswith("#"))
        self.assertIn("@world", result)

    def test_multiword_each_word_independent(self):
        # Each word should be normalized independently — verify by checking
        # that multi-word normalize_text matches word-by-word normalize
        # 每个词应独立规范化——通过检查多词结果与逐词结果一致来验证
        words = ["ᠰᠡᠢᠨ", "ᠣᠷᠣᠨ", "ᠨᠠᠢ᠍ᠮᠠ"]
        text = " ".join(words)
        result = self.s.normalize_text(text)
        expected = " ".join(self.s.normalize(w) for w in words)
        self.assertEqual(result, expected)


class TestNNBSP(unittest.TestCase):
    """
    U+202F (Narrow No-Break Space) handling.
    U+202F（窄不换行空格）处理。

    NNBSP appears in older Mongolian text in the same role as MVS (U+180E).
    The shaping engine normalizes NNBSP → MVS at the earliest preprocessing
    point (tokenization), so all downstream processing sees only MVS.
    NNBSP 出现在较旧的蒙古文文本中，与 MVS (U+180E) 角色相同。
    字形引擎在最早的预处理点（分词）将 NNBSP 统一转换为 MVS，
    后续所有处理仅看到 MVS。
    """

    NNBSP = "\u202F"
    MVS = "\u180E"

    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper(locale="MNG")

    # ── Tokenization: NNBSP survives, not silently dropped ─────────
    # (observed through shape_detailed(), which reports one record per token)
    # (通过 shape_detailed() 观察,它逐 token 报告)

    def test_nnbsp_survives_tokenization(self):
        # NNBSP between two Mongolian letters must not be dropped;
        # it is normalized to MVS (U+180E) during tokenization.
        # NNBSP 在两个蒙古文字母之间不应被丢弃；
        # 在分词阶段被规范化为 MVS (U+180E)。
        text = "ᠰᠠᠢᠨ" + self.NNBSP + "ᠠ"
        cps = [detail["cp"] for detail in self.s.shape_detailed(text)]
        self.assertIn("U+180E", cps, "NNBSP must be normalized to MVS during tokenization")
        self.assertNotIn("U+202F", cps, "NNBSP codepoint must not survive tokenization")

    def test_nnbsp_token_is_mvs_flag(self):
        # NNBSP input must produce exactly one MVS token (cp U+180E, alias "mvs")
        # NNBSP 输入必须恰好产生一个 MVS 标记(cp U+180E,alias "mvs")
        text = "ᠰ" + self.NNBSP + "ᠠ"
        mvs_toks = [d for d in self.s.shape_detailed(text) if d["cp"] == "U+180E"]
        self.assertEqual(len(mvs_toks), 1)
        self.assertEqual(mvs_toks[0]["alias"], "mvs")

    # ── Shaping: NNBSP triggers same behavior as MVS ──────────────

    def test_nnbsp_same_shape_as_mvs(self):
        # Word with NNBSP should shape identically to same word with MVS
        # 使用 NNBSP 的词应与使用 MVS 的相同词产生相同字形
        stem = "ᠰᠠᠢᠨ"
        suffix = "ᠠ"
        with_mvs = stem + self.MVS + suffix
        with_nnbsp = stem + self.NNBSP + suffix
        self.assertEqual(self.s.shape(with_mvs), self.s.shape(with_nnbsp))

    def test_nnbsp_chachlag_trigger(self):
        # NNBSP before a/e should trigger chachlag condition (like MVS)
        stem = "ᠰᠠᠢᠨ"
        suffix = "ᠠ"
        text = stem + self.NNBSP + suffix
        details = self.s.shape_detailed(text)
        # The suffix 'a' after NNBSP should get chachlag condition
        a_tokens = [d for d in details
                    if d["alias"] == "a" and d["condition"] == "chachlag"]
        self.assertGreater(len(a_tokens), 0,
                           "NNBSP should trigger chachlag on following a/e")
        self.assertEqual(a_tokens[-1]["written"], ["Aa"])

    # ── Normalization: NNBSP converted to MVS ───────────────────────

    def test_nnbsp_converted_to_mvs_in_normalize(self):
        # normalize() must convert NNBSP → MVS in output
        # normalize() 必须在输出中将 NNBSP 转换为 MVS
        stem = "ᠰᠠᠢᠨ"
        suffix = "ᠠ"
        text = stem + self.NNBSP + suffix
        result = self.s.normalize(text)
        self.assertIn(self.MVS, result,
                      "NNBSP must be normalized to MVS in output")
        self.assertNotIn(self.NNBSP, result,
                         "NNBSP must not survive normalization")

    def test_mvs_stays_mvs_in_normalize(self):
        # Regression: MVS input must remain MVS in output
        # 回归测试：MVS 输入在输出中仍为 MVS
        stem = "ᠰᠠᠢᠨ"
        suffix = "ᠠ"
        text = stem + self.MVS + suffix
        result = self.s.normalize(text)
        self.assertIn(self.MVS, result)
        self.assertNotIn(self.NNBSP, result)

    # ── normalize_text(): NNBSP in Mongolian runs ─────────────────

    def test_nnbsp_in_mongolian_run(self):
        # NNBSP should be part of Mongolian text runs, not break them;
        # output contains MVS (NNBSP was normalized at tokenization).
        # NNBSP 应属于蒙古文段，不应打断文本段；
        # 输出中包含 MVS（NNBSP 在分词阶段已被规范化）。
        stem = "ᠰᠠᠢᠨ"
        suffix = "ᠠ"
        text = stem + self.NNBSP + suffix
        result = self.s.normalize_text(text)
        self.assertIn(self.MVS, result,
                      "NNBSP must be normalized to MVS in text output")
        self.assertNotIn(self.NNBSP, result,
                         "NNBSP must not survive normalize_text()")

    def test_nnbsp_normalize_text_matches_normalize(self):
        # For a single word+suffix with NNBSP, normalize_text should
        # produce the same result as normalize
        stem = "ᠰᠠᠢᠨ"
        suffix = "ᠠ"
        text = stem + self.NNBSP + suffix
        self.assertEqual(self.s.normalize_text(text), self.s.normalize(text))

    def test_nnbsp_mixed_with_spaces(self):
        # NNBSP inside Mongolian word (normalized to MVS), regular space between words
        # NNBSP 在蒙古文词内（规范化为 MVS），普通空格在词间
        word1 = "ᠰᠠᠢᠨ" + self.NNBSP + "ᠠ"
        word2 = "ᠨᠠᠢ᠍ᠮᠠ"
        text = word1 + " " + word2
        result = self.s.normalize_text(text)
        self.assertIn(self.MVS, result, "NNBSP must become MVS in output")
        self.assertNotIn(self.NNBSP, result, "NNBSP must not survive")
        self.assertIn(" ", result)

    # ── Idempotence with NNBSP ────────────────────────────────────

    def test_nnbsp_normalize_idempotent(self):
        stem = "ᠰᠠᠢᠨ"
        suffix = "ᠠ"
        text = stem + self.NNBSP + suffix
        n1 = self.s.normalize(text)
        n2 = self.s.normalize(n1)
        self.assertEqual(n1, n2, "normalize() with NNBSP must be idempotent")

    def test_nnbsp_normalize_text_idempotent(self):
        text = "ᠰᠠᠢᠨ" + self.NNBSP + "ᠠ" + " " + "ᠨᠠᠢ᠍ᠮᠠ"
        n1 = self.s.normalize_text(text)
        n2 = self.s.normalize_text(n1)
        self.assertEqual(n1, n2, "normalize_text() with NNBSP must be idempotent")


if __name__ == "__main__":
    unittest.main(verbosity=2)
