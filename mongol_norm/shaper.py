#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
UTN57 Mongolian Shaping Engine (Hudum)
UTN57 蒙古文字形引擎（回鹘式蒙古文）

Implements the COMPLETE shaping process from:
实现完整的字形处理流程，依据：
  https://mongfontbuilder.pages.dev/hudum/
  UTN #57 v4 (Encoding and Shaping of the Mongolian Script)

Why this engine exists / 为什么需要这个引擎:
  Traditional Mongolian has extreme encoding ambiguity — the SAME visual glyph
  can be produced by many different Unicode sequences (e.g. five encodings of
  "sain" all render identically). This makes text search, deduplication, and
  NLP unreliable without a shaping-aware normalizer.
  传统蒙古文存在严重的编码歧义——同一个视觉字形可以由多种不同的 Unicode 序列产生
  （例如"sain"一词有五种编码方式，但渲染结果完全相同）。如果没有感知字形的规范化器，
  文本搜索、去重和自然语言处理都无法可靠运行。

5-step Mongolian-specific shaping phase / 蒙古文5步字形处理阶段:
  1. Chachlag（附加形式）  — MVS-triggered suffix forms for a/e
                             MVS 触发的 a/e 词缀形式
  2. Syllabic（音节规则）  — consonant/vowel context rules
                             辅音/元音上下文规则（onset起始/devsger连接/marked标记等）
  3. Particle（助词处理）  — MVS particle dictionary lookup
                             MVS 助词词典查找
  4. Devsger（连接齿形）   — i vowel_devsger (double tooth after vowel)
                             元音后的 i 取双齿形
  5. Post-bowed（弓形后续）— vowel forms after bowed consonants (G, Gx, K, K2, B, P, F)
                             弓形辅音后的元音形态

Data source / 数据来源: the bundled JSON in `mongol_norm/data/` (flat
pre-processed rules), generated from mongfontbuilder + UTN #57 by the
dev-only scripts in `scripts/`. See docs/data-format.md.
"""
from typing import Dict

from ._data import load_rules
from . import rules as _rules

# Positional forms: a Mongolian letter takes different shapes depending on
# where it appears in the word. This is analogous to Arabic initial/medial/final forms.
# 位置形式：蒙古文字母根据在词中的位置取不同字形，类似阿拉伯文的首/中/尾形式。
POSITIONS = ["isol", "init", "medi", "fina"]

# Free Variation Selectors (FVS): Unicode codepoints that follow a letter to
# select a non-default glyph variant. They are the root cause of encoding ambiguity —
# the same visual result can often be achieved with or without FVS.
# 自由变体选择符（FVS）：跟在字母后面用于选择非默认字形变体的 Unicode 码位。
# 它们是编码歧义的根本原因——同一视觉结果往往可以用或不用 FVS 来实现。
FVS_CPS = {0x180B, 0x180C, 0x180D, 0x180F}
FVS_INT_TO_CP = {0: None, 1: 0x180B, 2: 0x180C, 3: 0x180D, 4: 0x180F}
FVS_CP_TO_INT = {v: k for k, v in FVS_INT_TO_CP.items() if v is not None}

# MVS (Mongolian Vowel Separator): signals suffix boundaries. Triggers special
# glyph forms (chachlag) and particle matching.
# MVS（蒙古文元音分隔符）：标示词缀边界，触发特殊字形（chachlag 附加形式）和助词匹配。
MVS_CP = 0x180E
NIRUGU_CP = 0x180A
ZWJ_CP = 0x200D
ZWNJ_CP = 0x200C
NNBSP_CP = 0x202F

MONGOLIAN_BLOCK = range(0x1800, 0x18B0)

# Bowed written units: consonants whose final stroke curves ("bows") rightward.
# Vowels following bowed consonants take special "post-bowed" forms (step 5).
# 弓形书写单元：末笔向右弯曲的辅音。弓形辅音后的元音需要取特殊的"弓形后续"形态（第5步）。
BOWED_UNITS = {"G", "Gx", "K", "K2", "B", "P", "F"}


def is_mongolian_letter(cp):
    """
    Is cp a Mongolian letter (not FVS/MVS/punct/digit)?
    判断码位是否为蒙古文字母（排除 FVS/MVS/标点/数字）。
    """
    if cp in FVS_CPS or cp == MVS_CP or cp == NIRUGU_CP:
        return False
    if cp in range(0x1800, 0x180A):  # punctuation
        return False
    if cp in range(0x1810, 0x181A):  # digits
        return False
    return cp in MONGOLIAN_BLOCK


def is_mongolian_word_char(cp):
    """Return whether *cp* participates in a Mongolian word run.

    Strict word validation and mixed-text segmentation must share this
    classification. Splitting at Nirugu or ZWJ changes neighbouring letters'
    isol/init/medi/fina positions and therefore changes normalization.
    """
    return (is_mongolian_letter(cp)
            or cp in FVS_CPS
            or cp in (MVS_CP, NNBSP_CP, NIRUGU_CP, ZWJ_CP))


def _check_word_chars(text):
    """
    Raise ValueError if `text` contains any char that is not a Mongolian
    letter, FVS, MVS, NNBSP, Nirugu, or ZWJ. Used by `shape()` and
    `normalize()` to refuse mixed-script input early — silently dropping
    spaces / Latin / Chinese / etc. would mask user errors. For free-form
    mixed text use `normalize_text()` instead.
    若 `text` 含非蒙古字母 / FVS / MVS / NNBSP / Nirugu / ZWJ 之外的字符,
    抛 ValueError。`shape()` 与 `normalize()` 借此拒绝混合文本输入 ——
    静默吞掉空格 / 英文 / 中文等会掩盖用户错误。处理自由文本请用 `normalize_text()`。
    """
    for i, ch in enumerate(text):
        cp = ord(ch)
        if is_mongolian_word_char(cp):
            continue
        # Build a helpful error with the offending char's position and id.
        # 报错信息含越位字符的位置和码位。
        raise ValueError(
            f"non-Mongolian character {ch!r} (U+{cp:04X}) at index {i}: "
            f"shape() / normalize() accept only Mongolian letters + FVS/MVS/"
            f"NNBSP/Nirugu/ZWJ. For mixed-script input use normalize_text()."
        )


# ── Token ───────────────────────────────────────────────────────

class Token:
    """
    A token in the shaping pipeline. Each Mongolian letter becomes one Token.
    字形处理管线中的标记。每个蒙古文字母对应一个 Token。

    Key fields / 关键属性:
      cp        — Unicode codepoint of the letter / 字母的 Unicode 码位
      fvs_cp    — optional FVS codepoint following the letter / 可选的后续 FVS 码位
      position  — positional form: isol/init/medi/fina / 位置形式：独立/首/中/尾
      condition — shaping condition assigned by the 5-step pipeline / 5步管线赋予的字形条件
      written   — resolved glyph unit(s), e.g. ('I', 'I') for devsger / 解析后的字形单元
      alias     — human-readable name like 'A', 'S', 'n' / 人类可读的简称
    """
    __slots__ = [
        'cp', 'fvs_cp', 'fvs_cps', 'position', 'condition', 'written',
        'alias', 'is_mvs', 'is_letter', 'is_nirugu', 'index',
    ]

    def __init__(self, cp, fvs_cp=None, index=0):
        self.cp = cp
        self.fvs_cp = fvs_cp
        # Full stack of FVS codepoints attached to this letter (in stream
        # order). `fvs_cp` mirrors `fvs_cps[0]` for the common
        # single-FVS case; the resolver consults the full tuple to
        # implement first-VALID-FVS-wins fallback (e.g. `sh fvs3 fvs1`
        # → sh.init has no fvs3 variant, so fvs1 takes effect).
        self.fvs_cps = (fvs_cp,) if fvs_cp is not None else ()
        self.position = "isol"
        self.condition = None
        self.written = None  # Lazily resolved — set during _resolve_token_written()
                             # 延迟解析——在 _resolve_token_written() 中设置
        self.alias = ""
        self.is_mvs = (cp == MVS_CP or cp == NNBSP_CP)
        self.is_letter = is_mongolian_letter(cp)
        self.is_nirugu = (cp == NIRUGU_CP)
        self.index = index
    
    def __repr__(self):
        fvs = f"+FVS{FVS_CP_TO_INT.get(self.fvs_cp, '?')}" if self.fvs_cp else ""
        cond = f" [{self.condition}]" if self.condition else ""
        return f"<{self.alias or f'U+{self.cp:04X}'}{fvs} @{self.position}{cond}>"


# ── Shaper ──────────────────────────────────────────────────────

class MongolianShaper:
    """
    Full UTN57 Hudum shaping engine.
    完整的 UTN57 回鹘式蒙古文字形引擎。

    The engine loads glyph variant data from the mongfontbuilder package, then
    provides three core operations:
    引擎从 mongfontbuilder 包加载字形变体数据，然后提供三个核心操作：

      shape(text)      — Forward shaping: text → visual glyph sequence
                         正向字形处理：文本 → 视觉字形序列
      same_shape(a, b) — Visual identity comparison (do a and b look the same?)
                         视觉一致性比较（a 和 b 看起来一样吗？）
      normalize(text)  — Canonical encoding: many-to-one mapping via shape+harmony
                         规范化编码：通过字形+元音和谐实现多对一映射

    Usage / 用法:
        shaper = MongolianShaper(locale="MNG")
        shape = shaper.shape("ᠰᠠᠢᠨ")  # → ['S', 'A', 'I', 'I', 'A']
    """

    def __init__(self, locale="MNG"):
        self.locale = locale
        self._load_data()
        self._build_lookups()
        self._shaping_rules = _rules.get_rules_for_locale(self.locale)

    # ── Data loading ────────────────────────────────────────────

    def _load_data(self):
        """
        Load the flat shape-rules JSON for the selected locale.
        加载当前 locale 的扁平 shape-rules JSON。

        Data comes from the bundled JSON in `mongol_norm/data/`, pre-processed
        from mongfontbuilder + UTN #57. No runtime reference resolution, no
        cross-locale merging — the JSON is language-agnostic and the same file
        can be consumed by JS/Dart/etc. ports.

        数据来自 bundled JSON(`mongol_norm/data/`),由 mongfontbuilder + UTN #57
        预处理生成。运行时无需解析交叉引用或跨 locale 合并——该 JSON 语言无关,
        JS/Dart 等移植版本可共享同一份数据。
        """
        self._rules = load_rules(self.locale)

        # Build a compat nested dict so the rest of the engine (which still walks
        # self.variants in {char_name: {pos: {fvs_str: vdata}}} shape) keeps working.
        # Each vdata here already has locale-specific fields inlined under self.locale,
        # since the preprocess step filtered to the target locale.
        self.variants: Dict[str, Dict[str, Dict[str, Dict]]] = {}
        for letter in self._rules["letters"]:
            char_name = letter["name"]
            for v in letter["variants"]:
                pos, fvs_str = v["position"], str(v["fvs"])
                vdata = {
                    "written": v["written"],
                    "default": v["default"],
                    "locales": {
                        self.locale: {
                            "written": v["written"],
                            "conditions": v["conditions"],
                            "archaic": v["archaic"],
                            "unrecommended": v["unrecommended"],
                        }
                    },
                }
                self.variants.setdefault(char_name, {}).setdefault(pos, {})[fvs_str] = vdata

        # Aliases for this locale, flattened to {char_name: alias}.
        self.aliases: Dict[str, str] = {
            letter["name"]: letter["alias"]
            for letter in self._rules["letters"]
            if letter.get("alias")
        }

        # Locale-level metadata (categories + particles).
        self.locales_data = {self.locale: {"categories": self._rules["categories"]}}
        self.particles_data = {self.locale: self._rules["particles"]}
    
    def _build_lookups(self):
        """
        Build fast-access lookup tables from the loaded JSON data.
        从加载的 JSON 数据构建快速访问查找表。

        These indices avoid repeated JSON traversal during shaping. The most
        critical one is variant_lookup: given (codepoint, position, FVS) it
        returns the variant data dict in O(1), driving the entire pipeline.
        这些索引避免了字形处理过程中重复遍历 JSON。最关键的是 variant_lookup：
        给定（码位, 位置, FVS）在 O(1) 时间内返回变体数据字典，驱动整个管线。
        """
        # Character name ↔ codepoint / 字符名 ↔ 码位
        self.cp_to_name = {}
        self.name_to_cp = {}
        for char_name in self.variants:
            cp = self._get_cp(char_name)
            if cp is not None:
                self.cp_to_name[cp] = char_name
                self.name_to_cp[char_name] = cp
        
        # Alias: cp → alias string for this locale / 别名：码位 → 本区域的别名字符串
        # Aliases are locale-dependent because the same codepoint may represent
        # different phonemes in different scripts (MNG vs TOD vs SIB).
        # 别名是区域相关的，因为同一码位在不同文字中可能代表不同音素。
        self.cp_to_alias = {}
        self.alias_to_cp = {}
        for char_name, alias_data in self.aliases.items():
            cp = self._get_cp(char_name)
            if cp is None:
                continue
            alias = None
            if isinstance(alias_data, str):
                alias = alias_data
            elif isinstance(alias_data, dict):
                # Try locale namespace: MNG → SIB etc
                ns = self.locale[:3] if len(self.locale) > 3 else self.locale
                alias = alias_data.get(self.locale) or alias_data.get(ns)
            if alias:
                self.cp_to_alias[cp] = alias
                self.alias_to_cp[alias] = cp
        
        # Variant lookup: (cp, pos, fvs_int) → vdata
        # 变体查找：（码位, 位置, FVS编号）→ 变体数据
        # Default lookup: (cp, pos) → (fvs_int, vdata)
        # 默认查找：（码位, 位置）→（FVS编号, 变体数据）
        # The "default" variant is the one rendered when no FVS is present.
        # "默认"变体是没有 FVS 时渲染的那个——这正是规范化要利用的特性。
        self.variant_lookup = {}
        self.default_variant = {}
        
        for char_name, pos_data in self.variants.items():
            cp = self.name_to_cp.get(char_name)
            if cp is None:
                continue
            for pos in POSITIONS:
                if pos not in pos_data:
                    continue
                for fvs_str, vdata in pos_data[pos].items():
                    fvs_int = int(fvs_str)
                    locales = vdata.get("locales", {})
                    if self.locale not in locales:
                        continue
                    self.variant_lookup[(cp, pos, fvs_int)] = vdata
                    if vdata.get("default"):
                        self.default_variant[(cp, pos)] = (fvs_int, vdata)
        
        # Locale categories: vowels and consonants classified by gender/harmony.
        # 区域分类：按性别/和谐分类的元音和辅音。
        # Mongolian vowel harmony splits vowels into masculine (阳性 o/u),
        # feminine (阴性 oe/ue/ee), and neuter (中性 i). This classification
        # drives h/g selection in step 2 and letter choice during normalization.
        # 蒙古文元音和谐将元音分为阳性（o/u）、阴性（oe/ue/ee）和中性（i）。
        # 这一分类驱动第2步中 h/g 的选择以及规范化时的字母选择。
        locale_info = self.locales_data.get(self.locale, {})
        cats = locale_info.get("categories", {})
        self.vowels = set(cats.get("vowel", []))
        self.consonants = set(cats.get("consonant", []))
        self.masculine_vowels = set(cats.get("vowelMasculine", []))
        self.feminine_vowels = set(cats.get("vowelFeminine", []))
        self.neuter_vowels = set(cats.get("vowelNeuter", []))
        
        # Particle dictionary: alias sequence → list of token indices needing "particle" condition
        # 助词词典：别名序列 → 需要"particle"条件的标记索引列表
        self.particle_dict = self.particles_data.get(self.locale, {})
    
    def _get_cp(self, name):
        try:
            import unicodedata
            return ord(unicodedata.lookup(name))
        except (KeyError, ValueError):
            return None
    
    # ── Written resolution / 书写单元解析 ──────────────────────────
    # "Written" units are the abstract glyph names that a letter produces at a
    # given position. E.g., A@init → ('A',), I@medi with vowel_devsger → ('I', 'I').
    # Some variants define their written indirectly (referencing another position/FVS),
    # so we need a recursive resolver.
    # "书写单元"是字母在特定位置产生的抽象字形名称。例如 A@首 → ('A',)，
    # I@中(元音连接) → ('I', 'I')。某些变体间接定义其书写单元（引用另一位置/FVS），
    # 因此需要递归解析。

    def _resolve_written(self, written, char_name, depth=0):
        if depth > 5 or written is None:
            return None
        if isinstance(written, list):
            if (len(written) >= 2 and isinstance(written[0], str) 
                    and written[0] in POSITIONS):
                rp, rf = written[0], str(written[1])
                pd = self.variants.get(char_name, {}).get(rp, {}).get(rf, {})
                for src in [
                    pd.get("locales", {}).get(self.locale, {}).get("written"),
                    pd.get("written"),
                ]:
                    if src:
                        r = self._resolve_written(src, char_name, depth + 1)
                        if r:
                            return r
                return None
            return tuple(str(x) for x in written)
        return None
    
    def _get_written(self, cp, pos, fvs_int):
        char_name = self.cp_to_name.get(cp, "")
        vdata = self.variant_lookup.get((cp, pos, fvs_int))
        if vdata:
            locale_data = vdata.get("locales", {}).get(self.locale, {})
            w_raw = locale_data.get("written") or vdata.get("written")
            return self._resolve_written(w_raw, char_name)
        return None
    
    def _get_condition_fvs(self, cp, pos, condition):
        """
        Find the FVS int for a given condition at (cp, pos).
        查找给定条件下（码位, 位置）对应的 FVS 编号。

        When the 5-step pipeline assigns a condition (e.g. "onset", "feminine"),
        this method looks up which FVS variant has that condition in its locale data.
        This is how conditions map to actual glyph variants.
        当5步管线分配了条件（如"onset"、"feminine"）后，此方法在区域数据中查找
        哪个 FVS 变体具有该条件。这就是条件映射到实际字形变体的机制。
        """
        char_name = self.cp_to_name.get(cp, "")
        pos_data = self.variants.get(char_name, {}).get(pos, {})
        for fvs_str, vdata in pos_data.items():
            locales = vdata.get("locales", {})
            if self.locale not in locales:
                continue
            locale_data = locales[self.locale]
            conditions = locale_data.get("conditions", [])
            if condition in conditions:
                return int(fvs_str)
        return None
    
    # ── Tokenization / 分词 ────────────────────────────────────────
    # Tokenization converts raw Unicode text into a sequence of Token objects,
    # grouping each letter with its trailing FVS (if any). Non-Mongolian characters
    # are silently skipped — only Mongolian letters and MVS markers survive.
    # 分词将原始 Unicode 文本转换为 Token 对象序列，将每个字母与其后续 FVS（如有）
    # 分组。非蒙古文字符被静默跳过——只有蒙古文字母和 MVS 标记被保留。

    def tokenize(self, text):
        """Split text into Token list. / 将文本拆分为 Token 列表。"""
        cps = [ord(c) for c in text]
        tokens = []
        i = 0
        idx = 0
        while i < len(cps):
            cp = cps[i]
            if is_mongolian_letter(cp):
                # Collect ALL trailing FVS marks on this letter. The
                # resolver tries them in stream order and picks the
                # FIRST one that maps to an existing variant — matching
                # iii.py's behavior where an invalid FVS produces no
                # substitution and a later valid FVS still fires
                # (`sh fvs3 fvs1 ...` → sh.init.fvs3 unknown, fvs1=S
                # wins). When only one FVS is present (the common case),
                # this collapses to the single-FVS behavior.
                fvs_list = []
                j = i + 1
                while j < len(cps) and cps[j] in FVS_CPS:
                    fvs_list.append(cps[j])
                    j += 1
                fvs_cp = fvs_list[0] if fvs_list else None
                tok = Token(cp, fvs_cp, index=idx)
                tok.fvs_cps = tuple(fvs_list)
                tok.alias = self.cp_to_alias.get(cp, "")
                tokens.append(tok)
                idx += 1
                i = j
            elif cp == MVS_CP or cp == NNBSP_CP:
                # Normalize NNBSP → MVS at tokenization (earliest preprocessing point).
                # NNBSP 在分词阶段统一转换为 MVS（最早的预处理点）。
                tok = Token(MVS_CP, index=idx)
                tok.alias = "mvs"
                tokens.append(tok)
                idx += 1
                i += 1
            elif cp == NIRUGU_CP or cp == ZWJ_CP:
                # Nirugu (U+180A) and ZWJ (U+200D) are both joining
                # markers that extend the chain. Tokenize them as
                # nirugu-equivalent so `assign_positions` counts them
                # toward the chain — `zwj + d` becomes [zwj.init,
                # d.fina], so d.fina default (`Dd`) is emitted. iii.py
                # preprocessing (line 42) collapses nirugu/zwj/zwnj into
                # the same "ignored" class, so reusing `is_nirugu` here
                # for ZWJ matches that grouping.
                tok = Token(cp, index=idx)
                tok.alias = "nirugu" if cp == NIRUGU_CP else "zwj"
                tok.is_nirugu = True
                tokens.append(tok)
                idx += 1
                i += 1
            else:
                i += 1
        return tokens
    
    def assign_positions(self, tokens):
        """
        Assign isol/init/medi/fina to letter tokens.
        为字母标记分配位置形式：独立/首/中/尾。

        Position assignment is structural (word boundary detection):
        single letter → isol, first → init, last → fina, middle → medi.
        MVS breaks the joining chain, so segments are assigned independently.
        位置分配纯粹是结构性的（词边界检测）：MVS 断开连接链。

        Nirugu (U+180A) is a joining marker that EXTENDS the chain — it
        counts as a "virtual member" of the segment for position
        purposes but doesn't get a position itself. So:
          - `nirugu a` → a is at fina (a is index 1 of [nirugu, a])
          - `a nirugu` → a is at init (a is index 0 of [a, nirugu])
          - `nirugu a nirugu` → a is medi
        Verified against DraftNew-Regular.otf via hb-shape.
        """
        segments = []
        current = []
        for t in tokens:
            if t.is_letter or t.is_nirugu:
                current.append(t)
            elif t.is_mvs:
                if current:
                    segments.append(current)
                    current = []
        if current:
            segments.append(current)
        for seg in segments:
            n = len(seg)
            for i, tok in enumerate(seg):
                if not tok.is_letter:
                    continue
                if n == 1:
                    tok.position = "isol"
                elif i == 0:
                    tok.position = "init"
                elif i == n - 1:
                    tok.position = "fina"
                else:
                    tok.position = "medi"
    
    # ── Helpers ─────────────────────────────────────────────────
    
    def _is_vowel(self, tok):
        return tok and tok.is_letter and tok.alias in self.vowels
    
    def _is_consonant(self, tok):
        return tok and tok.is_letter and tok.alias in self.consonants
    
    def _is_masc_vowel(self, tok):
        return tok and tok.alias in self.masculine_vowels
    
    def _is_fem_vowel(self, tok):
        return tok and tok.alias in self.feminine_vowels
    
    def _is_neut_vowel(self, tok):
        return tok and tok.alias in self.neuter_vowels
    
    def _prev_letter(self, tokens, index):
        for scan_index in range(index - 1, -1, -1):
            if tokens[scan_index].is_letter:
                return tokens[scan_index]
        return None

    def _next_letter(self, tokens, index):
        for scan_index in range(index + 1, len(tokens)):
            if tokens[scan_index].is_letter:
                return tokens[scan_index]
        return None

    def _prev_tok(self, tokens, index):
        return tokens[index - 1] if index > 0 else None

    def _next_tok(self, tokens, index):
        return tokens[index + 1] if index + 1 < len(tokens) else None

    def _prev_adjacent_letter(self, tokens, index):
        """Return the nearest preceding letter REACHING index without an
        intervening MVS. Nirugu (a joining marker) is transparent — we
        skip past it — but MVS (a word boundary) blocks.

        Use this when a rule requires GLYPH adjacency for joining
        purposes (e.g. iii2f.h_g.harmony's "adjacent vowel" checks).
        """
        scan_index = index - 1
        while scan_index >= 0:
            token = tokens[scan_index]
            if token.is_mvs:
                return None
            if token.is_letter:
                return token
            if token.is_nirugu:
                scan_index -= 1
                continue
            return None
        return None

    def _next_adjacent_letter(self, tokens, index):
        """Mirror image of `_prev_adjacent_letter`."""
        scan_index = index + 1
        while scan_index < len(tokens):
            token = tokens[scan_index]
            if token.is_mvs:
                return None
            if token.is_letter:
                return token
            if token.is_nirugu:
                scan_index += 1
                continue
            return None
        return None

    def _has_fvs(self, token):
        return token.fvs_cp is not None

    def _written_ends_with(self, token, unit):
        return token.written and token.written[-1] == unit
    
    def _resolve_token_written(self, tok):
        """
        Resolve a single token's written units (lazy evaluation).
        延迟解析单个标记的书写单元。

        Resolution priority / 解析优先级:
          1. Explicit FVS on the token → use that FVS variant IF it
             matches a variant in the data. Mirrors iii.py: the FVS
             variant lookup substitutes the glyph to its FVS variant.
             标记上有显式 FVS 且数据里有对应变体 → 用该 FVS 变体。
          2. Condition assigned by the rule pipeline → use the FVS that
             carries that condition. This is also the FALLBACK PATH for
             cases where the explicit FVS doesn't match any variant
             (mirrors iii.py: FVS substitution doesn't fire, but a later
             rule's substitution can still apply on the default glyph).
             规则管线分配的 condition → 用带该 condition 的 FVS;同时
             也是「显式 FVS 不命中数据」时的回退路径。
          3. Default variant (no FVS, no condition).
             默认变体(无 FVS 也无 condition 时)。
        """
        if tok.written is not None:
            return
        if tok.is_mvs:
            tok.written = ()
            return

        written = None

        # 1. Explicit FVS first — try each FVS attached in stream
        #    order, accept the first that maps to a real variant.
        for fvs_cp in tok.fvs_cps:
            fvs_int = FVS_CP_TO_INT.get(fvs_cp, 0)
            candidate = self._get_written(tok.cp, tok.position, fvs_int)
            if candidate:
                written = candidate
                break

        # 2. Condition (rule-assigned) — also covers the case where
        #    explicit FVS didn't yield a variant
        if not written and tok.condition:
            cond_fvs = self._get_condition_fvs(tok.cp, tok.position, tok.condition)
            if cond_fvs is not None:
                written = self._get_written(tok.cp, tok.position, cond_fvs)

        # 3. Default
        if not written:
            dflt = self.default_variant.get((tok.cp, tok.position))
            if dflt:
                written = self._get_written(tok.cp, tok.position, dflt[0])

        tok.written = written if written else ()
    
    def _get_word_aliases(self, tokens):
        """Get alias sequence for the word (for particle lookup)."""
        return [t.alias for t in tokens if t.is_letter or t.is_mvs]
    
    # ── Shaping steps ────────────────────────────────────────────
    # The condition-assignment logic is now in `rules.py` as a declarative
    # table that mirrors mongfontbuilder's iii.py (iii1..iii5). The helpers
    # below — `_masc_marker_reaches_g_h` and the small predicates — are
    # consumed by the rule functions via the shaper instance handed to them.

    def _masc_marker_reaches_g_h(self, tokens, idx):
        """
        Mirror MARKER_MASCULINE propagation from mongfontbuilder iii.py
        preprocessing — both FORWARD (A/B/C) and BACKWARD (G/H/I/J/K).
        模拟 mongfontbuilder iii.py 中 MARKER_MASCULINE 的双向传播链。

        Returns True iff a MASC marker would sit *immediately after* the g/h
        at `idx` after the full preprocessing chain — the precondition for
        the iii2f.A pattern 5 substitution `i + g/h + MASC → masculine_devsger`.

        Two complementary mechanisms in iii.py (`iii.py:78-212`):

        ── FORWARD (preprocessing.A + B + C) ──
        A masc vowel at init/medi position seeds a MASC marker which
        propagates forward through non-fem letters via chain-context sub
        (each step duplicates a new MASC after the matched letter).
        preprocessing.C strips MASC from after non-h/g letters but
        preserves it after h/g. Net: MASC sits after every h/g preceded
        somewhere by a masc vowel (init/medi) with no fem vowel between.

        ── BACKWARD (preprocessing.G + H + J + K) ──
        Documented in iii0b: "G to K implement masculinity indefinitely
        passing backward". A masc vowel triggers preprocessing.G on the
        immediately preceding init/medi consonant/neut vowel, marking it.
        preprocessing.H is a ReverseChainSingleSubst that propagates
        marked-ness backward through chain of init/medi non-fem letters.
        preprocessing.J reverts non-h/g letters to unmarked (they were
        only intermediate carriers). preprocessing.K converts marked h/g
        back to unmarked + adds MASC marker after. Net: a g/h at
        init/medi gets MASC after it if there's a later masc vowel
        reachable through unbroken chain of init/medi non-fem letters.

        Both mechanisms feed into the same iii2f.A pattern 5 substitution.
        For test coverage of the backward path, see `s i g s i g a → S I H
        S I Hx A` (`tests/data/core-hud.tsv: syllabic-h/g-10`): the first
        g(idx 2) has prev=i + reachable masc a(idx 6) via the chain
        g→s→i→g, so backward propagation marks it → "H".

        Verified against `DraftNew-Regular.otf` via hb-shape.
        """
        # Nirugu is transparent for all propagation scans (it's a joining
        # marker, invisible to the harmony machinery). MVS still blocks.
        # ── Forward (preprocessing.A/B/C) ──
        for j in range(idx - 1, -1, -1):
            t = tokens[j]
            if not t.is_letter:
                if t.is_nirugu:
                    continue
                break  # mvs/etc. blocks; fall through to backward check
            if self._is_fem_vowel(t):
                break  # blocks; fall through to backward check
            if self._is_masc_vowel(t) and t.position in ("init", "medi"):
                return True

        # ── Backward (preprocessing.G/H/J/K) ──
        # g/h must itself be at init/medi to participate in preprocessing.H.
        if tokens[idx].position not in ("init", "medi"):
            return False

        # Block backward chain when a fem vowel exists earlier in the word.
        # (preprocessing.D/E/F propagates FEM forward; FEM settling on
        # downstream h/g inhibits MASC promotion via iii2f.A. Empirically:
        #   `i g t a`         → first g = H  (no fem before)
        #   `e g e n i g t a` → second g = G (fem `e` earlier in word))
        for j in range(idx - 1, -1, -1):
            t = tokens[j]
            if not t.is_letter:
                if t.is_nirugu:
                    continue
                break
            if self._is_fem_vowel(t):
                return False

        # Walk forward; chain must be unbroken non-fem init/medi letters
        # terminating at a masc vowel OR `.fina letter + mvs + isol a`
        # (preprocessing.G second loop).
        for j in range(idx + 1, len(tokens)):
            nxt = tokens[j]
            if not nxt.is_letter:
                if nxt.is_nirugu:
                    continue  # transparent
                return False  # mvs etc. breaks chain
            if self._is_masc_vowel(nxt):
                return True
            if self._is_fem_vowel(nxt):
                return False
            if nxt.position in ("init", "medi"):
                continue
            # .fina position: check `fina + mvs + isol a` trigger.
            # Skip past nirugu to find the actual next non-nirugu tokens.
            k = j + 1
            while k < len(tokens) and tokens[k].is_nirugu:
                k += 1
            if (nxt.position == "fina"
                    and k < len(tokens) and tokens[k].is_mvs
                    and k + 1 < len(tokens)
                    and tokens[k + 1].is_letter
                    and tokens[k + 1].alias == "a"
                    and tokens[k + 1].position == "isol"):
                return True
            return False

        return False

    def shape(self, text):
        """
        Full shaping pipeline: text → glyph sequence (written units).
        完整字形管线：文本 → 字形序列（书写单元）。

        This is the forward direction of the shaping process:
        这是字形处理的正向过程：
          1. Tokenize: split text into letter tokens
             分词：将文本拆分为字母标记
          2. Assign positions: isol/init/medi/fina based on word structure
             分配位置：根据词结构确定 独立/首/中/尾
          3. Run 5-step condition assignment
             运行5步条件分配
          4. Resolve each token's written units
             解析每个标记的书写单元
          5. Flatten into a single list
             展平为单一列表

        Returns: list of written unit strings, e.g. ['S', 'A', 'I', 'I', 'A']
        返回：书写单元字符串列表，例如 ['S', 'A', 'I', 'I', 'A']

        Raises ValueError if `text` contains non-Mongolian characters
        (spaces, Latin, Chinese, punctuation, etc.). Use `normalize_text()`
        for free-form mixed-script input.
        若 `text` 含非蒙古文字符(空格、英文、中文、标点等)则抛 ValueError。
        混合文本请用 `normalize_text()`。
        """
        _check_word_chars(text)
        tokens = self.tokenize(text)
        self.assign_positions(tokens)

        # Condition mapping — runs the declarative rule table mirroring
        # mongfontbuilder's iii.py (one rule per OpenType Lookup, in order).
        _rules.run_rules(self._shaping_rules, tokens, self)

        for tok in tokens:
            self._resolve_token_written(tok)

        result = []
        for tok in tokens:
            if tok.is_mvs:
                result.append("mvs")
            elif tok.is_nirugu:
                # Joiners are shape tokens like 'mvs': nirugu renders a
                # visible stem glyph, ZWJ invisibly forces joining — both
                # are the evidence for neighbours' init/medi/fina forms and
                # must survive into the shape (and thus into normalize).
                # joiner 与 mvs 同级:nirugu 是可见的连笔字形,zwj 隐形强制
                # 连接;都是邻居 init/medi/fina 形的依据,必须保留进 shape。
                result.append(tok.alias)  # 'nirugu' or 'zwj'
            elif tok.written:
                result.extend(tok.written)
        return result
    
    def shape_str(self, text):
        return "+".join(self.shape(text))
    
    def same_shape(self, text1, text2):
        """
        Compare two strings for visual identity: do they render the same glyphs?
        比较两个字符串的视觉一致性：它们是否渲染相同的字形？

        This is the key operation that enables normalization — if two encodings
        produce the same shape, they are visually identical and should normalize
        to the same canonical form.
        这是实现规范化的关键操作——如果两种编码产生相同的字形序列，
        它们视觉上相同，应该规范化为同一规范形式。
        """
        return self.shape(text1) == self.shape(text2)

    def shape_detailed(self, text):
        """Return detailed shaping breakdown per token. Raises on non-Mongolian input."""
        _check_word_chars(text)
        tokens = self.tokenize(text)
        self.assign_positions(tokens)
        _rules.run_rules(self._shaping_rules, tokens, self)
        for tok in tokens:
            self._resolve_token_written(tok)
        
        details = []
        for tok in tokens:
            fvs = f"+FVS{FVS_CP_TO_INT.get(tok.fvs_cp, '?')}" if tok.fvs_cp else ""
            details.append({
                "cp": f"U+{tok.cp:04X}",
                "alias": tok.alias,
                "position": tok.position,
                "fvs": fvs,
                "condition": tok.condition or "",
                "written": list(tok.written) if tok.written else [],
            })
        return details
    
    def _build_candidates_map(self):
        """
        Build (pos, written) → list of {cp, fvs, alias, default} dicts.
        构建 (pos, written) → 候选 dict 列表。

        Each variant contributes TWO candidates / 每个变体贡献两个候选:
          1. (cp, fvs)  — the explicit FVS encoding from data
          2. (cp, 0)    — the BARE encoding, which can produce the same
                          written when a runtime rule fires the matching
                          condition (e.g. bare `i.medi` after vowel →
                          vowel_devsger → ('I','I') even though data only
                          records this written under fvs=2)
        Bare encodings are added only when missing; shape() verification
        in `_compute_chain_canonical` filters out combos whose rules
        don't actually fire.
        裸编码会被加入(若不存在):某些 shape 只能通过运行时规则触发
        condition 才能从裸字母得到(如元音后的 i.medi 经 vowel_devsger
        变成 ('I','I'),数据里只在 fvs=2 下记录)。

        Includes ALL variants for the locale — including archaic and
        unrecommended ones — so every shape that shape() can produce
        has at least one candidate encoding.
        包含此 locale 下所有变体(含 archaic / unrecommended),保证 shape()
        能产出的每个 shape 都至少有一个候选编码。
        """
        self._candidates_map = {}
        for char_name, pos_data in self.variants.items():
            cp = self.name_to_cp.get(char_name)
            if cp is None:
                continue
            alias = self.cp_to_alias.get(cp, "")
            for pos in POSITIONS:
                if pos not in pos_data:
                    continue
                for fvs_str, vdata in pos_data[pos].items():
                    fvs_int = int(fvs_str)
                    locales = vdata.get("locales", {})
                    if self.locale not in locales:
                        continue
                    locale_data = locales[self.locale]
                    w_raw = locale_data.get("written") or vdata.get("written")
                    written = self._resolve_written(w_raw, char_name)
                    if not written:
                        continue
                    # `conditions` in the source data is a list of named
                    # conditions this variant fires for (e.g. ['particle']
                    # for i.isol.fvs1). We track 'particle' explicitly so
                    # the canonical encoder can prefer particle variants
                    # at isol position (user-requested rule: shape ['I']
                    # at iso → i+fvs1, not bare j).
                    # `conditions` 记录该变体触发的命名条件(如 i.isol.fvs1
                    # 的 ['particle'])。canonical 编码器据此在 isol 位置
                    # 优先选 particle 变体。
                    raw_conds = locale_data.get("conditions") or vdata.get("conditions") or []
                    is_particle = (isinstance(raw_conds, list)
                                   and 'particle' in raw_conds)
                    key = (pos, written)
                    if key not in self._candidates_map:
                        self._candidates_map[key] = []
                    self._candidates_map[key].append({
                        "cp": cp, "fvs": fvs_int, "alias": alias,
                        "default": vdata.get("default", False),
                        "particle": is_particle,
                    })

        # Add bare (fvs=0) encoding for any cp that appears in a slot
        # but only under non-zero FVS. shape() verification will weed
        # out cases where context doesn't fire the needed rule.
        # 为只在非零 FVS 出现的 cp 补加裸编码,shape() 校验会筛掉
        # 上下文不触发所需规则的情况。
        for key, cands in list(self._candidates_map.items()):
            existing_bare_cps = {c["cp"] for c in cands if c["fvs"] == 0}
            for cp in {c["cp"] for c in cands} - existing_bare_cps:
                alias = self.cp_to_alias.get(cp, "")
                cands.append({
                    "cp": cp, "fvs": 0, "alias": alias,
                    "default": False,
                    "particle": False,  # speculative bare is never a real particle variant
                })

    def normalize(self, text):
        """
        Canonical normalize: a pure function of shape.
        规范化:纯粹的 shape → Unicode 函数。

        Stronger than round-trip / 比保形更强的性质:
          shape(x) == shape(y)  ⟹  normalize(x) == normalize(y)
        Two inputs with the same shape always normalize to the SAME Unicode
        sequence. This is achieved by deriving the output from shape() alone,
        not from the input encoding's token structure.
        同 shape 的两个输入必然得到完全相同的 Unicode 输出。算法只依赖 shape,
        不读输入的编码细节,从而保证多对一。

        Canonical = SHORTEST encoding, lex-smallest tiebreak.
        规范形 = 最短编码,字典序最小作为 tiebreak。
          - Fewest letters wins (so `y+fvs1 + y+fvs1 → i` collapses).
            字母数最少胜出 (故 `y+fvs1 + y+fvs1 → i` 折叠)。
          - Among same-length, lex-smallest Unicode wins (so `o` beats `u`
            when both produce ['A','O'] at isol).
            同长度下,Unicode 字典序最小胜出 (故同 shape 时 `o` 胜 `u`)。

        Algorithm / 算法:
          1. shape(text) → target  (a list of written units + 'mvs' tokens)
          2. Split target at 'mvs' into chains.
             按 'mvs' 切分 target 为多个 chain。
          3. For each chain, enumerate encodings via DP over partitions:
             for each n_letters from 1 to chain_len, for each partition of
             chain shape into n_letters slots, pick deterministic candidate
             from `_candidates_map`, verify by reshape, keep lex-smallest.
             Cache by chain shape for repeats.
             对每个 chain 通过 DP 枚举编码:按 n_letters 从小到大,枚举划分,
             从 `_candidates_map` 选候选,reshape 校验,保留字典序最小。
             按 chain shape 缓存以加速重复。
          4. Concatenate chain encodings with MVS between them.
             用 MVS 拼接各 chain 编码。

        Examples / 例:
          shape ['S','A','I','I','A']
            All five sain encodings collapse to `s a i n` (4 letters, lex-min).
            所有五种 sain 编码都落到 `s a i n` (4 字母,字典序最小)。
          shape ['A','O','R','O','A']
            Both `o r o n` and `o r u n` collapse to `o r o n` (o beats u lex).
            两种 oron 编码都落到 `o r o n` (o 字典序 < u)。
        """
        if not text:
            return text

        target = self.shape(text)
        if not target:
            # Empty shape ⟹ canonical is empty string. This drops
            # input that has only joining markers / FVS but no letter
            # (e.g. lone nirugu) — they're invisible, so canonical = "".
            # 空 shape ⟹ canonical 为空串。输入只有 nirugu / FVS 等不可见
            # 内容时统一归并到 ""(它们没有视觉,canonical 应一致)。
            return ""

        canonical = self._canonical_for_shape(target)

        # Safety net: if our enumeration somehow missed a valid encoding,
        # fall back to the input. By construction this should not fire on
        # valid Mongolian shapes — the candidates map covers every
        # production rule that shape() emits.
        # 安全网:若枚举失败,回退到原文。对正常蒙古文应不会触发。
        if not canonical or self.shape(canonical) != target:
            return text
        return canonical

    # ── Shape → canonical Unicode (pure function) ──────────────────
    # The chain shape acts as the key: any two inputs whose shape() output
    # is identical land in the same cache slot and get the same Unicode.
    # 链 shape 作为 key:任何 shape 相同的输入命中同一缓存,得同一 Unicode。

    def _canonical_for_shape(self, shape_list):
        """
        Build canonical Unicode from a full shape list (incl. 'mvs').
        从完整 shape 列表(含 'mvs')构建 canonical Unicode。

        Processes chains RIGHT-TO-LEFT so that each chain's encoding
        verification can include the already-encoded canonical of all
        following chains as suffix. This is necessary because of
        cross-MVS rule interactions: e.g. masc-vowel after MVS can
        propagate backward through MVS to mark a g/h in the previous
        chain (changing its rendering between G and H). Per-chain
        verification with only adjacent MVS is insufficient.
        从右向左处理 chain,每个 chain 的编码校验可以拿到后续 chain
        已确定的 canonical 作为 suffix。必要 —— MVS 之间存在跨链规则
        交互:MVS 后的阳性元音可能反向传播,影响 MVS 前的 g/h 渲染。
        仅包含相邻 MVS 的校验不足以捕获这类交互。
        """
        # Parse into structural (mvs/nirugu/zwj) + chain segments. Structural
        # tokens are copied VERBATIM into the canonical (they are part of the
        # shape, like 'mvs'); only the letter chains between them are encoded.
        # 切分为结构 token(mvs/nirugu/zwj)与 chain。结构 token 原样进
        # canonical;只对其间的字母 chain 编码。
        parts = []  # list of (structural_token, None) | ('chain', tuple)
        current_chain = []
        for unit in shape_list:
            if unit in self._STRUCTURAL_CHARS:
                if current_chain:
                    parts.append(('chain', tuple(current_chain)))
                    current_chain = []
                parts.append((unit, None))
            else:
                current_chain.append(unit)
        if current_chain:
            parts.append(('chain', tuple(current_chain)))

        encoded = [None] * len(parts)
        suffix_text = ""
        suffix_target = ()
        for index in range(len(parts) - 1, -1, -1):
            kind, body = parts[index]
            if kind != 'chain':
                encoded[index] = self._STRUCTURAL_CHARS[kind]
                suffix_text = encoded[index] + suffix_text
                suffix_target = (kind,) + suffix_target
            else:
                # Context = the full run of structural tokens immediately
                # before this chain (an MVS behind a nirugu still matters:
                # chachlag looks through nirugu).
                # 上下文 = 紧邻前方的整段结构 token(nirugu 背后的 MVS 仍有
                # 影响:chachlag 会穿透 nirugu)。
                scan = index - 1
                prefix_tokens = ()
                while scan >= 0 and parts[scan][0] != 'chain':
                    prefix_tokens = (parts[scan][0],) + prefix_tokens
                    scan -= 1
                chain_canonical = None
                # A chain directly after MVS is a suffix particle: encode it
                # as its STANDALONE canonical — drop the MVS, normalize,
                # re-attach — so the spelling is identical with and without
                # MVS and never depends on it. One exception: chachlag
                # ('Aa',), whose canonical after MVS is the bare letter a
                # (`mvs + a` IS the chachlag spelling). Verified in full
                # context; falls through if the spelling happens to render
                # differently after MVS.
                # MVS 后的 chain = 后缀词:去掉 MVS 按 standalone 求 canonical、
                # 再拼回 MVS —— 有无 MVS 拼写一致,不依赖 MVS。唯一例外:
                # chachlag('Aa'),MVS 后 canonical 是裸字母 a(`mvs + a`
                # 就是 chachlag 的标准拼写)。全上下文校验;变形则回退。
                if prefix_tokens and prefix_tokens[-1] == 'mvs':
                    if body == ('Aa',):
                        candidate = chr(0x1820)   # bare a
                    else:
                        candidate = self._encode_chain_canonical(body)
                    if candidate:
                        prefix_text = ''.join(self._STRUCTURAL_CHARS[t] for t in prefix_tokens)
                        want = prefix_tokens + body + suffix_target
                        if tuple(self.shape(prefix_text + candidate + suffix_text)) == want:
                            chain_canonical = candidate
                if chain_canonical is None:
                    chain_canonical = self._encode_chain_canonical(
                        body, prefix_tokens, suffix_text, suffix_target,
                    )
                encoded[index] = chain_canonical
                suffix_text = chain_canonical + suffix_text
                suffix_target = body + suffix_target
        return "".join(encoded)

    # Structural shape tokens and the characters they encode to, verbatim.
    # 结构 shape token 及其原样对应的字符。
    _STRUCTURAL_CHARS = {
        'mvs': chr(MVS_CP),
        'nirugu': chr(NIRUGU_CP),
        'zwj': chr(ZWJ_CP),
    }
    # Joiners force cursive connection on the adjacent letter (shift its
    # position to a joined form); MVS does not (post-MVS letters restart).
    # joiner 强制相邻字母连接(位置变连接形);MVS 不会(MVS 后重新起词)。
    _JOINER_TOKENS = frozenset({'nirugu', 'zwj'})

    def _encode_chain_canonical(self, chain_shape, prefix_tokens=(),
                                suffix_text="", suffix_target=()):
        """Memoised canonical encoding for a chain shape under its structural
        prefix (mvs/nirugu/zwj run) + suffix context."""
        if not hasattr(self, '_chain_canon_cache'):
            self._chain_canon_cache = {}
        cache_key = (chain_shape, prefix_tokens, suffix_text, suffix_target)
        cached = self._chain_canon_cache.get(cache_key)
        if cached is not None:
            return cached
        result = self._compute_chain_canonical(chain_shape, prefix_tokens, suffix_text, suffix_target)
        self._chain_canon_cache[cache_key] = result
        return result

    def _compute_chain_canonical(self, chain_shape, prefix_tokens=(),
                                  suffix_text="", suffix_target=()):
        """
        Canonical encoding for chain_shape under its structural context
        (prefix_tokens = the mvs/nirugu/zwj run right before the chain).
        canonical 编码(prefix_tokens = 紧邻前方的 mvs/nirugu/zwj 结构串)。

        The per-(position, written-unit) FVS-pinned table lookup IS the
        algorithm — deterministic, O(N), prefix-stable, shape()-verified.
        There is no search fallback: with the letter-major / FVS-first
        battery ordering and the bowed probe, the table covers every corpus
        chain. On a genuine gap we return "" and normalize()'s safety net
        hands the input back unchanged (round-trip preserved, never a
        silent mis-encoding); extend the table via
        scripts/gen_normalize_table.py.
        逐(位置, 单元)的 FVS 钉死查表就是算法本身 —— 确定性、O(N)、前缀
        稳定、shape 校验。没有搜索兜底:字母优先/FVS 优先的电池排序 + 弓形
        探针后,表已覆盖全部语料 chain。真缺口返回 "",由 normalize 的安全
        网原样返回输入(保住往返,绝不静默错编);扩表用
        scripts/gen_normalize_table.py。
        """
        result = self._unit_encode_chain(chain_shape, prefix_tokens, suffix_text, suffix_target)
        if result is not None:
            return result
        return ""

    # ── Unit-encoder (FVS-pinned per-(position, unit) table) ─────────
    # Each shape unit is encoded by a context-INDEPENDENT (letter, fvs):
    # one that produces exactly that unit at that position regardless of
    # neighbor letters. So encoding is a deterministic function of the
    # shape, giving prefix-stability + same-shape-same-output for free.
    # 每个 shape 单元用 context 无关的 (字母,fvs) 编码:在该位置、任何邻居
    # 下都恰好产出该单元。编码因此是 shape 的确定性函数,白拿前缀稳定 +
    # 同 shape 同输出。

    # Velar feminine forms: the adjacent ambiguous vowel is encoded with
    # its FEMININE letter for clean output (g+fvs2+o round-trips but looks
    # wrong; oe is the linguistically-correct partner of a 'G' velar).
    # velar 阴形:相邻歧义元音用阴性字母,输出更自然(g+fvs2+o 虽能还原但
    # 字难看;oe 才是 'G' velar 的语言学搭档)。
    _VELAR_FEM_UNITS = frozenset({'G', 'Gx'})
    _MASC_TO_FEM_CP = {0x1820: 0x1821,   # a → e
                       0x1823: 0x1825,   # o → oe
                       0x1824: 0x1826}   # u → ue


    def _build_unit_enc(self):
        """
        Load the per-unit encoding tables from the precomputed spec bundled in
        `mongol_norm/data/`. The spec is generated OFFLINE by
        scripts/gen_normalize_table.py (the selection battery lives there, not
        in the runtime); here we only load it.
        从 `mongol_norm/data/` 的预生成 spec 加载逐单元编码表。spec 由
        scripts/gen_normalize_table.py 离线生成(选择电池在那里,不在 runtime),
        这里只负责加载。
        """
        if hasattr(self, '_unit_enc'):
            return
        spec = self._load_external_normalize_spec()
        if spec is None:
            raise RuntimeError(
                f"no bundled normalize table for locale {self.locale!r}; "
                f"generate it with scripts/gen_normalize_table.py"
            )
        self._load_normalize_tables(spec)

    def _load_external_normalize_spec(self):
        """
        Return the precomputed normalize spec for self.locale from the bundled
        data, or None to fall back to the in-process battery (spec not yet
        generated).
        从 bundled 数据取本 locale 的预生成 spec;无则返回 None 走电池。
        """
        try:
            from ._data import load_normalize_table
        except Exception:
            return None
        try:
            return load_normalize_table(self.locale)
        except Exception:
            return None

    def _load_normalize_tables(self, spec):
        """
        Populate the runtime per-unit tables from a serialized spec — the
        inverse of compute_normalize_tables / what the JSON stores.
        从序列化 spec 还原运行时逐单元表(compute_normalize_tables 的逆)。
        """
        def decode_entry(entry):
            return (int(entry["cp"], 16),
                    int(entry["fvs"], 16) if entry["fvs"] else None)
        table = {}
        for position, entries in spec["unit_table"].items():
            for written_key, entry in entries.items():
                table[(position, tuple(written_key.split("+")))] = decode_entry(entry)
        feminine_table = {}
        for position, entries in spec.get("velar_fem", {}).items():
            for written_key, entry in entries.items():
                feminine_table[(position, tuple(written_key.split("+")))] = decode_entry(entry)
        self._unit_enc = table
        self._unit_enc_fem = feminine_table
        self._unit_enc_max_len = (spec.get("unit_enc_max_len")
                                  or max((len(written) for (_, written) in table),
                                         default=1))

    def _unit_encode_chain(self, chain_shape, prefix_tokens=(),
                           suffix_text="", suffix_target=()):
        """
        Encode a chain by per-unit table lookup (PRIMARY path). Returns the
        encoding str, or None if no table partition round-trips (→ fallback).
        逐单元查表编码(主路径)。无可还原的查表划分时返回 None(→ 回退)。

        A joiner (nirugu/zwj) directly before/after the chain shifts letter
        positions (e.g. a lone unit between two nirugus sits at medi, not
        isol) — the partition looks units up at those shifted positions.
        紧邻的 joiner(nirugu/zwj)会移动字母位置(如夹在两个 nirugu 之间的
        单一单元处于 medi 而非 isol),划分按移动后的位置查表。
        """
        self._build_unit_enc()
        joined_left = bool(prefix_tokens) and prefix_tokens[-1] in self._JOINER_TOKENS
        joined_right = bool(suffix_target) and suffix_target[0] in self._JOINER_TOKENS
        text = self._unit_partition(chain_shape, joined_left, joined_right)
        if text is None:
            return None
        # Verify the encoding in its FULL context: the structural prefix run
        # (mvs/nirugu/zwj) and the already-encoded following chains
        # (suffix_text / suffix_target), so cross-boundary interactions are
        # checked. `verify_target` MUST include suffix_target — otherwise
        # multi-chain words' non-last chains never verify and fall back to
        # the search encoder (which can emit junk like 'ng' for [A,G]).
        # 在完整上下文中校验:前导结构串 + 后续已编码 chain。verify_target 必须
        # 含 suffix_target,否则多 chain 词的非末尾 chain 永远校验失败。
        prefix_text = ''.join(self._STRUCTURAL_CHARS[t] for t in prefix_tokens)
        verify_target = tuple(prefix_tokens) + tuple(chain_shape) + tuple(suffix_target)
        if tuple(self.shape(prefix_text + text + suffix_text)) == verify_target:
            return text
        return None

    def _unit_partition(self, chain_shape, joined_left=False, joined_right=False):
        """
        Single deterministic local partition+encode pass. At each position:
          1. take the single unit if the table has it (preferred — clean
             output: a+g not ng, a+i not the i digraph),
          2. else the longest available multi-unit entry (last resort).
        Local + deterministic ⇒ prefix-stable. Returns text or None.
        单趟确定性局部划分:优先单单元(输出干净) → 其余多单元(兜底)。
        局部确定 → 前缀稳定。

        joined_left / joined_right: a joiner glyph sits right before / after
        this chain, so positions are computed as if one extra unit padded
        that side (nirugu+o+nirugu → o at medi).
        joined_left/right:chain 紧邻 joiner,位置按该侧多一个单元计算。
        """
        table = self._unit_enc
        unit_count = len(chain_shape)
        pad_left = 1 if joined_left else 0
        pad_right = 1 if joined_right else 0
        padded_count = unit_count + pad_left + pad_right
        letters = []          # [cp, fvs_cp] per emitted letter
        unit_at = []          # single unit this letter covers, or None (multi)
        index = 0
        while index < unit_count:
            span = min(self._unit_enc_max_len, unit_count - index)
            hit = None
            hit_length = 0
            # 1) single unit (preferred)
            position = self._slot_position(index + pad_left, 1, padded_count)
            key = (position, (chain_shape[index],))
            if key in table:
                hit = table[key]; hit_length = 1
            # 2) else the longest available multi-unit entry (last resort)
            if hit is None:
                for length in range(span, 1, -1):
                    position = self._slot_position(index + pad_left, length, padded_count)
                    key = (position, tuple(chain_shape[index:index + length]))
                    if key in table:
                        hit = table[key]; hit_length = length; break
            if hit is None:
                return None
            letters.append([hit[0], hit[1]])
            unit_at.append(chain_shape[index] if hit_length == 1 else None)
            index += hit_length
        # velar-feminine refinement (clean output for G/Gx-coupled vowels)
        self._apply_velar_fem(chain_shape, letters, unit_at, pad_left, pad_right)
        return ''.join(
            chr(cp) + (chr(fvs) if fvs is not None else '')
            for cp, fvs in letters
        )

    def _apply_velar_fem(self, chain_shape, letters, unit_at,
                         pad_left=0, pad_right=0):
        """
        In-place: switch the ambiguous vowel coupled to each G/Gx velar to
        its feminine letter. Coupling: init/medi velar ↔ following vowel;
        fina velar ↔ preceding vowel. Only flips a/o/u → e/oe/ue.
        把每个 G/Gx velar 耦合的歧义元音改阴性。耦合:init/medi 取后、fina 取前。
        """
        total = len(letters)
        for letter_index, unit in enumerate(unit_at):
            if unit not in self._VELAR_FEM_UNITS:
                continue
            # FORWARD coupling only (init/medi velar → following vowel).
            # We deliberately SKIP backward coupling (fina velar → preceding
            # vowel): a fina velar becomes medi when a suffix is appended,
            # flipping its coupling direction, which would make the
            # shared-prefix vowel diverge between B and A. The FVS-pinned
            # velar (g+fvs2) renders G regardless, so a masculine preceding
            # vowel still round-trips — we trade that one's prettiness for
            # prefix-stability. Forward coupling is stable (the following
            # vowel keeps its place).
            # 只做前向耦合(init/medi velar → 后元音)。跳过后向(fina velar →
            # 前元音):fina 加后缀变 medi 会翻转耦合方向,破坏前缀稳定。FVS 钉死
            # 的 velar 无论前元音阴阳都渲染 G,故前元音保持阳性仍能还原。
            padded_total = total + pad_left + pad_right
            position = self._letter_position(letter_index + pad_left, padded_total)
            target_index = letter_index + 1 if position in ('init', 'medi') else None
            if target_index is None or not (0 <= target_index < total):
                continue
            target_unit = unit_at[target_index]
            if target_unit is None:
                continue  # multi-unit coupled letter — leave it
            cp, _ = letters[target_index]
            # only flip if the coupled vowel is currently a masculine vowel
            if cp not in self._MASC_TO_FEM_CP:
                continue
            target_position = self._letter_position(target_index + pad_left, padded_total)
            feminine = self._unit_enc_fem.get((target_position, (target_unit,)))
            if feminine is None:
                continue  # no round-trip-safe feminine form → leave masculine
            letters[target_index] = [feminine[0], feminine[1]]

    def _slot_position(self, start_index, length, unit_count):
        """Position of a letter spanning units [start_index, start_index+length)
        in a chain of unit_count units."""
        if start_index == 0 and start_index + length == unit_count:
            return 'isol'
        if start_index == 0:
            return 'init'
        return 'fina' if start_index + length == unit_count else 'medi'

    def _letter_position(self, letter_index, total):
        """Position of the letter_index-th letter out of `total` letters."""
        if total == 1:
            return 'isol'
        if letter_index == 0:
            return 'init'
        return 'fina' if letter_index == total - 1 else 'medi'

    def normalize_text(self, text):
        """
        Normalize a whole text string (sentence, paragraph, etc.).
        规范化整段文本（句子、段落等）。

        Segments the input into Mongolian word runs vs non-Mongolian spans,
        normalizes each Mongolian word independently, and preserves everything
        else (spaces, punctuation, Latin text, etc.) verbatim.
        将输入分段为蒙古文词段和非蒙古文片段，独立规范化每个蒙古文词，
        其余内容（空格、标点、拉丁文等）原样保留。

        Why a separate method / 为什么需要单独的方法:
          normalize() treats its entire input as one word — spaces and
          non-Mongolian characters are silently dropped during tokenization.
          This method correctly handles multi-word and mixed-script text.
          normalize() 将整个输入当作一个词——空格和非蒙古文字符在分词时被静默丢弃。
          此方法正确处理多词和混合文字文本。
        """
        if not text:
            return text

        # Segment text into runs of Mongolian characters (letters + FVS + MVS)
        # vs everything else.
        # 将文本分段为蒙古文字符段（字母 + FVS + MVS）和其他字符段。
        segments = []  # list of (is_mongolian: bool, substring: str)
        current_is_mong = None
        current_start = 0

        for i, ch in enumerate(text):
            cp = ord(ch)
            is_mong = is_mongolian_word_char(cp)
            if current_is_mong is None:
                current_is_mong = is_mong
            elif is_mong != current_is_mong:
                segments.append((current_is_mong, text[current_start:i]))
                current_is_mong = is_mong
                current_start = i

        if current_start < len(text):
            segments.append((current_is_mong, text[current_start:]))

        # Normalize Mongolian segments, pass through everything else.
        # 规范化蒙古文段，其他内容原样传递。
        parts = []
        for is_mong, span in segments:
            if is_mong:
                parts.append(self.normalize(span))
            else:
                parts.append(span)

        return "".join(parts)


# ── CLI / 命令行接口 ─────────────────────────────────────────────
# Four commands: shape, same, normalize (word), normalize-text (full text).
# All accept --locale for non-MNG scripts.
# 四个命令：shape、same、normalize（单词）、normalize-text（全文）。
# 都接受 --locale 参数用于非 MNG 文字。

def _read_input(text_arg, input_file):
    """
    Resolve input from CLI args: positional text, `-` for stdin, or
    `-i FILE`. Returns the input string.
    解析 CLI 输入:位置参数文本、`-` 从 stdin、或 `-i FILE` 从文件。
    """
    import sys
    if input_file is not None:
        with open(input_file, "r", encoding="utf-8") as f:
            return f.read()
    if text_arg is None:
        # Default to stdin if no input given
        return sys.stdin.read()
    if text_arg == "-":
        return sys.stdin.read()
    return text_arg


def _write_output(text, output_file):
    """Write to `-o FILE` or stdout."""
    if output_file is not None:
        with open(output_file, "w", encoding="utf-8") as f:
            f.write(text)
    else:
        # print() handles the trailing newline; use sys.stdout.write to
        # preserve the caller's text exactly (including or excluding \n).
        # print() 会加换行;用 sys.stdout.write 保留原样。
        import sys
        sys.stdout.write(text)
        if not text.endswith("\n"):
            sys.stdout.write("\n")


def _process_batch(lines, fn):
    """Apply `fn` to each line; preserve line count. Errors include line no."""
    out = []
    for lineno, line in enumerate(lines.splitlines(), start=1):
        try:
            out.append(fn(line))
        except ValueError as e:
            raise ValueError(f"line {lineno}: {e}") from None
    return "\n".join(out) + ("\n" if lines.endswith("\n") else "")


def _add_io_args(p, with_input=True):
    """Add common I/O flags to subparser."""
    if with_input:
        p.add_argument("text", nargs="?",
                       help=("Input text (positional). Use `-` to read from "
                             "stdin; omit and pass `-i FILE` to read from a file."))
        p.add_argument("-i", "--input", metavar="FILE",
                       help="Read input from FILE (UTF-8). Mutually exclusive with positional text.")
    p.add_argument("-o", "--output", metavar="FILE",
                   help="Write output to FILE (UTF-8). Default: stdout.")
    p.add_argument("--batch", action="store_true",
                   help=("Process input line by line, emit one result per line. "
                         "Use this when -i/stdin is a multi-line file (one word/text per line)."))


# Reusable examples block shown in each subcommand's --help epilog.
# 每个子命令 --help 末尾共享的示例块。
_IO_EXAMPLES_TEMPLATE = """\
I/O modes / I/O 模式:
  inline text :    mongol-norm {cmd} '{ex_in}'
  stdin       :    echo '{ex_in}' | mongol-norm {cmd} -
                   cat file.txt | mongol-norm {cmd} -
  file in     :    mongol-norm {cmd} -i input.txt
  file out    :    mongol-norm {cmd} -i input.txt -o output.txt
  batch mode  :    mongol-norm {cmd} --batch -i words.txt -o out.txt
                   cat words.txt | mongol-norm {cmd} --batch -
"""


def _io_epilog(cmd, example_input):
    return _IO_EXAMPLES_TEMPLATE.format(cmd=cmd, ex_in=example_input)


def main():
    import argparse
    import sys

    parser = argparse.ArgumentParser(
        prog="mongol-norm",
        description=(
            "Mongolian shaping / normalization tool (UTN #57 v4 + GB/T 25914-2023).\n"
            "蒙古文字形 / 规范化工具 (UTN #57 v4 + GB/T 25914-2023)。"
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "I/O modes (apply to shape / normalize / normalize-text):\n"
            "I/O 模式(适用于 shape / normalize / normalize-text):\n"
            "  inline      :  mongol-norm <cmd> 'TEXT'\n"
            "  stdin       :  echo 'TEXT' | mongol-norm <cmd> -\n"
            "                 cat file.txt  | mongol-norm <cmd> -\n"
            "  file input  :  mongol-norm <cmd> -i input.txt\n"
            "  file output :  mongol-norm <cmd> -i input.txt -o output.txt\n"
            "  batch       :  mongol-norm <cmd> --batch -i words.txt -o out.txt\n"
            "                 (one word/text per line in, one result per line out)\n"
            "\n"
            "Examples / 示例:\n"
            "  mongol-norm normalize 'ᠰᠡᠢᠨ'                      # → ᠰᠠᠢᠠ\n"
            "  mongol-norm shape 'ᠰᠠᠢᠨ'                          # → S+A+I+I+A\n"
            "  mongol-norm normalize-text 'Hello ᠰᠡᠢᠨ world'      # mixed script\n"
            "  echo 'ᠰᠡᠢᠨ' | mongol-norm normalize -\n"
            "  mongol-norm normalize --batch -i words.txt -o canonical.txt\n"
            "  cat doc.txt | mongol-norm normalize-text - > doc.norm.txt\n"
            "\n"
            "See `mongol-norm <cmd> --help` for per-command details.\n"
            "查看具体命令帮助: `mongol-norm <cmd> --help`\n"
        ),
    )
    parser.add_argument("--locale", default="MNG",
                        help="Locale: MNG (default), TOD, SIB, MCH, etc.")
    sub = parser.add_subparsers(dest="cmd", required=True, metavar="CMD")

    p_shape = sub.add_parser(
        "shape",
        help="Return '+'-joined written-unit sequence",
        description="Shape input through the UTN57 pipeline; output written units joined by '+'.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=_io_epilog("shape", "ᠰᠠᠢᠨ"),
    )
    _add_io_args(p_shape)

    p_norm = sub.add_parser(
        "normalize",
        help="Normalize a single Mongolian WORD to canonical Unicode",
        description=(
            "Normalize a single Mongolian word (no spaces / non-Mongolian).\n"
            "Property: same shape → same Unicode. For mixed-script / multi-word\n"
            "text use `normalize-text` instead."
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=_io_epilog("normalize", "ᠰᠡᠢᠨ"),
    )
    _add_io_args(p_norm)

    p_normt = sub.add_parser(
        "normalize-text",
        help="Normalize full text (multi-word, mixed script)",
        description="Segment input into Mongolian runs vs others; normalize Mongolian, pass through rest.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=_io_epilog("normalize-text", "Hello ᠰᠡᠢᠨ world"),
    )
    _add_io_args(p_normt)

    p_same = sub.add_parser(
        "same",
        help="Check if TEXT1 and TEXT2 shape identically (exit 0 if yes)",
        description=(
            "Compare two inputs by shape. Exit 0 if visually identical, 1 otherwise.\n"
            "Inline-text only (no file I/O — this command takes two args)."
        ),
    )
    p_same.add_argument("text1")
    p_same.add_argument("text2")

    args = parser.parse_args()
    shaper = MongolianShaper(locale=args.locale)

    if args.cmd == "same":
        result = shaper.same_shape(args.text1, args.text2)
        print("true" if result else "false")
        sys.exit(0 if result else 1)

    # All other commands share the I/O pattern.
    text = _read_input(args.text, args.input)

    if args.cmd == "shape":
        op = lambda s: "+".join(shaper.shape(s))
    elif args.cmd == "normalize":
        op = shaper.normalize
    elif args.cmd == "normalize-text":
        op = shaper.normalize_text
    else:
        parser.error(f"unknown command: {args.cmd}")
        return

    try:
        if args.batch:
            result = _process_batch(text, op)
        else:
            result = op(text)
    except ValueError as e:
        # Friendly CLI error — no Python traceback.
        # CLI 友好错误,不抛 Python 栈。
        sys.stderr.write(f"error: {e}\n")
        sys.exit(2)

    _write_output(result, args.output)


if __name__ == "__main__":
    main()
