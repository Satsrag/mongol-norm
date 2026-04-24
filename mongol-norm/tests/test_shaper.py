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
from mongol_norm import MongolianShaper

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
            ["T", "A", "L", "mvs", "Aa"],
        )

    def test_step1_chachlag_talayin(self):
        # ᠲᠠᠯ᠎ᠠ᠎ᠶᠢᠨ  two MVS: chachlag on a, particle on y
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs a mvs y i n")),
            ["T", "A", "L", "mvs", "Aa", "mvs", "I", "I", "A"],
        )

    # 1-2  a/e after MVS + FVS → default (no chachlag)
    def test_step1_chachlag_mvs_a_fvs_default(self):
        # tal + MVS + a+FVS1 → a gets default (not chachlag)
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs a fvs1")),
            ["T", "A", "L", "mvs", "A"],
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
        # d.fina (no following vowel) keeps its devsger default "Dd"
        self.assertEqual(self.s.shape(_mgl("o d")), ["A", "O", "Dd"])

    # 2-4  n/j/w before MVS + isolated a/e → chachlag_onset
    def test_step2_chachlag_onset_n(self):
        # ᠰᠠᠢᠨ᠎ᠠ — n before MVS+a → chachlag_onset "N"
        self.assertEqual(
            self.s.shape(_mgl("s a i n mvs a")),
            ["S", "A", "I", "I", "N", "mvs", "Aa"],
        )
        # ᠬᠤᠷᠸ᠎ᠠ - w before MVS+a → chachlag_onset "U"
        self.assertEqual(
            self.s.shape(_mgl("h o r w mvs a")),
            ["H", "O", "R", "U", "mvs", "Aa"],
        )
        # ᠵ᠎ᠠ - j before MVS+a → chachlag_onset "I"
        self.assertEqual(
            self.s.shape(_mgl("j mvs a")),
            ["I", "mvs", "Aa"],
        )
        # ᠡᠵ᠎ᠡ - j.fina before MVS+e.iso → chachlag_onset "I"
        self.assertEqual(
            self.s.shape(_mgl("e j mvs e")),
            ["A", "I", "mvs", "Aa"],
        )

    # 2-5  h/g before MVS + isolated a → chachlag_onset
    def test_step2_chachlag_onset_g_a(self):
        # ᠶᠠᠪᠤᠭ᠎ᠠ — g before MVS+a → chachlag_onset
        self.assertEqual(
            self.s.shape(_mgl("y a b u g mvs a")),
            ["Y", "A", "B","O", "Hx", "mvs", "Aa"],
        )
        # ᠬᠠᠪᠬ᠎ᠠ — h before MVS+a → chachlag_onset
        self.assertEqual(
            self.s.shape(_mgl("h a b h mvs a")),
            ["H", "A", "B", "H", "mvs", "Aa"],
        )

    # 2-6  g before MVS + isolated e → chachlag_onset
    def test_step2_chachlag_onset_g_e(self):
        # ᠡᠭ᠎ᠡ — g before MVS+e → chachlag_onset
        self.assertEqual(
            self.s.shape(_mgl("e g mvs e")),
            ["A", "H", "mvs", "Aa"],
        )

    # 2-7  n/t/d before vowel → onset; after vowel → devsger
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

        # ᠳᠠᠳᠭ᠎ᠠ - d.medi after vowel and before consonant g → devsger "Dd"
        out = self.s.shape(_mgl("d a d g mvs a"))
        self.assertEqual(out[2], "Dd")  # d.medi devsger

        # ᠠᠲᠳ - t.medi after vowel a and before consonant d → devsger "T"; d.fina default → devsger "Dd"
        out = self.s.shape(_mgl("a t d"))
        self.assertEqual(out[2], "T")   # t.medi devsger
        self.assertEqual(out[3], "Dd")  # d.fina devsger (default)

    def test_step2_n_devsger(self):
        # ᠰᠠᠢᠨ (sain) — n after vowel → devsger → fina "A"
        self.assertEqual(
            self.s.shape(_mgl("s a i n"))[-1], "A",
        )

    def test_step2_t_onset(self):
        # ᠲᠣᠯᠢ (toli) — t before vowel → onset "T"
        self.assertEqual(
            self.s.shape(_mgl("t o l i"))[0], "T",
        )

    def test_step2_t_devsger_before_consonant(self):
        # ᠠᠲᠨ (atn) — t before consonant n → devsger "T"
        self.assertEqual(
            self.s.shape(_mgl("a t n")),
            ["A", "A", "T", "A"],
        )

    # 2-8  h/g: masculine/feminine context chain
    def test_step2_h_masculine_onset(self):
        # ᠬᠠᠷ (har) — h(QA) before masculine vowel a → masculine_onset "H"
        self.assertEqual(
            self.s.shape(_mgl("h a r")),
            ["H", "A", "R"],
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

    def test_step2_hg_remote_masculine(self):
        # ᠣᠭᠨ (ogn) — g remote: o is masculine → masculine_devsger "H"
        self.assertEqual(
            self.s.shape(_mgl("o g n")),
            ["A", "O", "H", "A"],
        )

    def test_step2_hg_remote_feminine(self):
        # ᠥᠭᠨ (oegn) — g remote: oe is feminine → feminine "G"
        self.assertEqual(
            self.s.shape(_mgl("oe g n")),
            ["A", "O", "I", "G", "A"],
        )

    # 2-9  t before ee or consonant → devsger
    def test_step2_t_devsger_before_ee(self):
        # ᠠᠲᠧᠨ (ateen) — t before ee → devsger "D"
        details = self.s.shape_detailed(_mgl("a t ee n"))
        t_tok = [d for d in details if d["alias"] == "t"][0]
        self.assertEqual(t_tok["condition"], "onset")

    # 2-10  sh: dotless before i
    def test_step2_sh_dotless(self):
        # ᠰᠢᠮ (shim) — sh.init before i.medi → dotless "S"
        self.assertEqual(
            self.s.shape(_mgl("sh i m")),
            ["S", "I", "M"],
        )

    # ══════════════════════════════════════════════════════════
    # Step 3 · Particle — MVS particle dictionary lookup
    # ══════════════════════════════════════════════════════════

    # 3-1  MVS + particle dict entries
    def test_step3_particle_yi(self):
        # tal + MVS + yi → y particle "I"
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs y i")),
            ["T", "A", "L", "mvs", "I", "I"],
        )

    def test_step3_particle_yin(self):
        # tal + MVS + yin → y particle "I"
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs y i n")),
            ["T", "A", "L", "mvs", "I", "I", "A"],
        )

    def test_step3_particle_du(self):
        # tal + MVS + du → d,u particle
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs d u")),
            ["T", "A", "L", "mvs", "D", "U"],
        )

    def test_step3_particle_i(self):
        # tal + MVS + i → i particle
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs i")),
            ["T", "A", "L", "mvs", "I"],
        )

    def test_step3_particle_u(self):
        # tal + MVS + u → u particle
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs u")),
            ["T", "A", "L", "mvs", "U"],
        )

    def test_step3_particle_iyar(self):
        # tal + MVS + iyar → i,y particle
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs i y a r")),
            ["T", "A", "L", "mvs", "I", "I", "A", "R"],
        )

    def test_step3_particle_dagan(self):
        # tal + MVS + dagan → d particle
        self.assertEqual(
            self.s.shape(_mgl("t a l mvs d a g a n")),
            ["T", "A", "L", "mvs", "D", "A", "Hx", "A", "A"],
        )

    # 3-2  u/ue particle without MVS
    def test_step3_particle_uu(self):
        # ᠤᠤ (uu) — "u u" in particle dict → u.init particle "O"
        self.assertEqual(
            self.s.shape(_mgl("u u")),
            ["O", "U"],
        )

    def test_step3_particle_ueue(self):
        # ᠦᠦ (ueue) — "ue ue" in particle dict → ue.init particle "O"
        self.assertEqual(
            self.s.shape(_mgl("ue ue")),
            ["O", "U"],
        )

    # ══════════════════════════════════════════════════════════
    # Step 4 · Devsger — i double tooth after vowel
    # ══════════════════════════════════════════════════════════

    def test_step4_devsger_ail(self):
        # ᠠᠢᠯ (ail) — i.medi after vowel a → vowel_devsger I,I
        self.assertEqual(
            self.s.shape(_mgl("a i l")),
            ["A", "A", "I", "I", "L"],
        )

    def test_step4_no_devsger_final(self):
        # ᠠᠢ (ai) — i.fina: devsger does NOT apply at final
        self.assertEqual(
            self.s.shape(_mgl("a i")),
            ["A", "A", "I"],
        )

    def test_step4_devsger_naima(self):
        # ᠨᠠᠢᠮᠠ (naima) — i after a → vowel_devsger I,I
        self.assertEqual(
            self.s.shape(_mgl("n a i m a")),
            ["N", "A", "I", "I", "M", "A"],
        )

    # ══════════════════════════════════════════════════════════
    # Step 5 · Post-bowed — vowels after bowed consonants
    # ══════════════════════════════════════════════════════════

    # 5-1  a/e after bowed → post_bowed
    def test_step5_post_bowed_uge(self):
        # ᠥᠭᠡ (üge) — e after G(bowed) → post_bowed "Aa"
        self.assertEqual(
            self.s.shape(_mgl("oe g e")),
            ["A", "O", "I", "G", "Aa"],
        )

    def test_step5_post_bowed_ger(self):
        # ᠭᠡᠷ (ger) — e after G(bowed) → post_bowed "A"
        self.assertEqual(
            self.s.shape(_mgl("g e r")),
            ["G", "A", "R"],
        )

    def test_step5_post_bowed_boge(self):
        # ᠪᠣᠭᠡ (boge) — e after G(bowed) → post_bowed "Aa"
        self.assertEqual(
            self.s.shape(_mgl("b o g e")),
            ["B", "O", "G", "Aa"],
        )

    # ══════════════════════════════════════════════════════════
    # Position assignment
    # ══════════════════════════════════════════════════════════

    def test_position_single_isol(self):
        tokens = self.s.tokenize(_mgl("n"))
        self.s.assign_positions(tokens)
        ltoks = [t for t in tokens if t.is_letter]
        self.assertEqual(ltoks[0].position, "isol")

    def test_position_init_fina(self):
        tokens = self.s.tokenize(_mgl("a b"))
        self.s.assign_positions(tokens)
        ltoks = [t for t in tokens if t.is_letter]
        self.assertEqual(ltoks[0].position, "init")
        self.assertEqual(ltoks[1].position, "fina")

    def test_position_init_medi_fina(self):
        tokens = self.s.tokenize(_mgl("t a l"))
        self.s.assign_positions(tokens)
        ltoks = [t for t in tokens if t.is_letter]
        self.assertEqual(ltoks[0].position, "init")
        self.assertEqual(ltoks[1].position, "medi")
        self.assertEqual(ltoks[2].position, "fina")

    def test_position_mvs_breaks_chain(self):
        # t a l MVS a → [init,medi,fina] + [isol]
        tokens = self.s.tokenize(_mgl("t a l mvs a"))
        self.s.assign_positions(tokens)
        ltoks = [t for t in tokens if t.is_letter]
        positions = [t.position for t in ltoks]
        self.assertEqual(positions, ["init", "medi", "fina", "isol"])

    def test_position_double_mvs(self):
        # t a l MVS a MVS y i n → 3 segments
        tokens = self.s.tokenize(_mgl("t a l mvs a mvs y i n"))
        self.s.assign_positions(tokens)
        ltoks = [t for t in tokens if t.is_letter]
        positions = [t.position for t in ltoks]
        self.assertEqual(positions, ["init", "medi", "fina", "isol", "init", "medi", "fina"])

    # ══════════════════════════════════════════════════════════
    # NNBSP → MVS normalization
    # ══════════════════════════════════════════════════════════

    def test_nnbsp_produces_same_shape(self):
        mvs_text = _mgl("t a l mvs a mvs y i n")
        nnbsp_text = _mgl("t a l nnbsp a nnbsp y i n")
        self.assertEqual(self.s.shape(nnbsp_text), self.s.shape(mvs_text))

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
        self.assertFalse(self.s.same_shape("ᠰᠠᠢᠨ", "ᠨᠠᠢᠮᠠ"))

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

    CANONICAL_SAIN = "ᠰᠠᠢᠨ"

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
        for word in ["ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ", "ᠨᠠᠢᠮᠠ", "ᠣᠷᠣᠨ"]:
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

    CANONICAL_SAIN = "ᠰᠠᠢᠨ"

    def test_single_word_matches_normalize(self):
        # normalize_text on a single word should match normalize
        # 单词的 normalize_text 应与 normalize 一致
        for word in ["ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ", "ᠨᠠᠢᠮᠠ", "ᠣᠷᠣᠨ"]:
            self.assertEqual(self.s.normalize_text(word), self.s.normalize(word))

    def test_two_words_space_separated(self):
        # Two Mongolian words separated by a space
        # 空格分隔的两个蒙古文词
        text = "ᠰᠡᠢᠨ ᠨᠠᠢᠮᠠ"
        result = self.s.normalize_text(text)
        expected = self.s.normalize("ᠰᠡᠢᠨ") + " " + self.s.normalize("ᠨᠠᠢᠮᠠ")
        self.assertEqual(result, expected)

    def test_space_preserved(self):
        # Spaces must be preserved exactly
        # 空格必须精确保留
        text = "ᠰᠠᠢᠨ  ᠨᠠᠢᠮᠠ"  # double space
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
        text = "ᠰᠡᠢᠨ, ᠨᠠᠢᠮᠠ!"
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
        text = "ᠰᠡᠢᠨ ᠨᠠᠢᠮᠠ"
        n1 = self.s.normalize_text(text)
        n2 = self.s.normalize_text(n1)
        self.assertEqual(n1, n2)

    def test_numbers_preserved(self):
        # Numbers mixed with Mongolian text stay unchanged
        # 数字与蒙古文混合时保持不变
        text = "ᠰᠡᠢᠨ 123 ᠨᠠᠢᠮᠠ"
        result = self.s.normalize_text(text)
        self.assertIn("123", result)
        self.assertEqual(
            result,
            self.s.normalize("ᠰᠡᠢᠨ") + " 123 " + self.s.normalize("ᠨᠠᠢᠮᠠ"),
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
        words = ["ᠰᠡᠢᠨ", "ᠣᠷᠣᠨ", "ᠨᠠᠢᠮᠠ"]
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

    def test_nnbsp_survives_tokenization(self):
        # NNBSP between two Mongolian letters must not be dropped;
        # it is normalized to MVS (U+180E) during tokenization.
        # NNBSP 在两个蒙古文字母之间不应被丢弃；
        # 在分词阶段被规范化为 MVS (U+180E)。
        text = "ᠰᠠᠢᠨ" + self.NNBSP + "ᠠ"
        tokens = self.s.tokenize(text)
        cps = [tok.cp for tok in tokens]
        self.assertIn(0x180E, cps, "NNBSP must be normalized to MVS during tokenization")
        self.assertNotIn(0x202F, cps, "NNBSP codepoint must not survive tokenization")

    def test_nnbsp_token_is_mvs_flag(self):
        # NNBSP input must produce a token with is_mvs=True and cp==MVS
        # NNBSP 输入必须产生 is_mvs=True 且 cp==MVS 的标记
        text = "ᠰ" + self.NNBSP + "ᠠ"
        tokens = self.s.tokenize(text)
        mvs_toks = [t for t in tokens if t.cp == 0x180E]
        self.assertEqual(len(mvs_toks), 1)
        self.assertTrue(mvs_toks[0].is_mvs)

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
        from mongol_norm import rules
        tokens = self.s.tokenize(text)
        self.s.assign_positions(tokens)
        rules.run_rules(self.s._shaping_rules, tokens, self.s)
        # The suffix 'a' after NNBSP should get chachlag condition
        a_tokens = [t for t in tokens if t.alias == "a" and t.condition == "chachlag"]
        self.assertGreater(len(a_tokens), 0,
                           "NNBSP should trigger chachlag on following a/e")

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
        word2 = "ᠨᠠᠢᠮᠠ"
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
        text = "ᠰᠠᠢᠨ" + self.NNBSP + "ᠠ" + " " + "ᠨᠠᠢᠮᠠ"
        n1 = self.s.normalize_text(text)
        n2 = self.s.normalize_text(n1)
        self.assertEqual(n1, n2, "normalize_text() with NNBSP must be idempotent")


if __name__ == "__main__":
    unittest.main(verbosity=2)
