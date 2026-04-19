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

Data source / 数据来源: mongol-shape-data (flat pre-processed rules JSON).
The rules are generated from mongfontbuilder + UTN #57 by the preprocess
script in the mongol-shape-data package.
"""
from typing import List, Tuple, Optional, Dict, Set

from mongol_shape_data import load_rules

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
        'cp', 'fvs_cp', 'position', 'condition', 'written',
        'alias', 'is_mvs', 'is_letter', 'index',
    ]

    def __init__(self, cp, fvs_cp=None, index=0):
        self.cp = cp
        self.fvs_cp = fvs_cp
        self.position = "isol"
        self.condition = None
        self.written = None  # Lazily resolved — set during _resolve_token_written()
                             # 延迟解析——在 _resolve_token_written() 中设置
        self.alias = ""
        self.is_mvs = (cp == MVS_CP or cp == NNBSP_CP)
        self.is_letter = is_mongolian_letter(cp)
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

    # ── Data loading ────────────────────────────────────────────

    def _load_data(self):
        """
        Load the flat shape-rules JSON for the selected locale.
        加载当前 locale 的扁平 shape-rules JSON。

        Data comes from the `mongol-shape-data` package, which ships pre-processed
        rules generated from mongfontbuilder + UTN #57. No runtime reference
        resolution, no cross-locale merging — the JSON is language-agnostic and
        the same file is consumed by future JS/Dart/etc. ports.

        数据来自 `mongol-shape-data` 包，内含由 mongfontbuilder + UTN #57 预处理
        生成的规则。运行时无需解析交叉引用或跨 locale 合并——该 JSON 语言无关，
        未来 JS/Dart 等移植版本共享同一份数据。
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
                fvs_cp = None
                j = i + 1
                while j < len(cps) and cps[j] in FVS_CPS:
                    fvs_cp = cps[j]
                    j += 1
                tok = Token(cp, fvs_cp, index=idx)
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
            else:
                i += 1
        return tokens
    
    def assign_positions(self, tokens):
        """
        Assign isol/init/medi/fina to letter tokens.
        为字母标记分配位置形式：独立/首/中/尾。

        Position assignment is purely structural (word boundary detection):
        single letter → isol, first → init, last → fina, middle → medi.
        MVS breaks the joining chain, so segments are assigned independently.
        位置分配纯粹是结构性的（词边界检测）：
        单字母 → 独立形式，首字母 → 首形式，末字母 → 尾形式，中间 → 中形式。
        MVS 断开连接链，各段独立分配位置。
        """
        segments = []
        current = []
        for t in tokens:
            if t.is_letter:
                current.append(t)
            elif t.is_mvs:
                if current:
                    segments.append(current)
                    current = []
        if current:
            segments.append(current)
        for ltoks in segments:
            n = len(ltoks)
            for i, tok in enumerate(ltoks):
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
    
    def _prev_letter(self, tokens, idx):
        for i in range(idx - 1, -1, -1):
            if tokens[i].is_letter:
                return tokens[i]
        return None
    
    def _next_letter(self, tokens, idx):
        for i in range(idx + 1, len(tokens)):
            if tokens[i].is_letter:
                return tokens[i]
        return None
    
    def _prev_tok(self, tokens, idx):
        return tokens[idx - 1] if idx > 0 else None
    
    def _next_tok(self, tokens, idx):
        return tokens[idx + 1] if idx + 1 < len(tokens) else None
    
    def _has_fvs(self, tok):
        return tok.fvs_cp is not None
    
    def _written_ends_with(self, tok, unit):
        return tok.written and tok.written[-1] == unit
    
    def _resolve_token_written(self, tok):
        """
        Resolve a single token's written units (lazy evaluation).
        延迟解析单个标记的书写单元。

        Resolution priority / 解析优先级:
          1. Explicit FVS on the token → use that FVS variant directly
             标记上有显式 FVS → 直接使用该 FVS 变体
          2. Condition assigned by 5-step pipeline → find which FVS has that condition
             5步管线分配的条件 → 查找哪个 FVS 具有该条件
          3. Bare letter (no FVS, no condition hit) → use default variant
             裸字母（无 FVS、无条件命中）→ 使用默认变体
        """
        if tok.written is not None:
            return
        if tok.is_mvs:
            tok.written = ()
            return

        fvs_int = FVS_CP_TO_INT.get(tok.fvs_cp, 0) if tok.fvs_cp else 0

        # Condition → FVS mapping (only if user didn't specify an explicit FVS)
        # 条件 → FVS 映射（仅当用户未指定显式 FVS 时）
        if tok.condition and not tok.fvs_cp:
            cond_fvs = self._get_condition_fvs(tok.cp, tok.position, tok.condition)
            if cond_fvs is not None:
                fvs_int = cond_fvs

        # Fallback to default variant for bare letters
        # 裸字母回退到默认变体
        if fvs_int == 0 and not tok.fvs_cp:
            dflt = self.default_variant.get((tok.cp, tok.position))
            if dflt:
                fvs_int = dflt[0]

        written = self._get_written(tok.cp, tok.position, fvs_int)
        tok.written = written if written else ()
    
    def _get_word_aliases(self, tokens):
        """Get alias sequence for the word (for particle lookup)."""
        return [t.alias for t in tokens if t.is_letter or t.is_mvs]
    
    # ── Shaping Steps / 字形处理步骤 ───────────────────────────────
    # The 5 steps run sequentially and assign a "condition" string to each token.
    # Later, _resolve_token_written() uses the condition to pick the correct
    # FVS variant. Steps are ordered by specificity: earlier steps have higher
    # priority — once a token has a condition, later steps skip it.
    # 5个步骤按顺序运行，为每个标记分配一个"条件"字符串。
    # 之后 _resolve_token_written() 使用条件来选择正确的 FVS 变体。
    # 步骤按特定性排序：先前的步骤优先级更高——一旦标记有了条件，后续步骤就跳过它。

    def _step1_chachlag(self, tokens):
        """
        Step 1: Chachlag — a/e after MVS get suffix form.
        第1步：附加形式——MVS 后的 a/e 取词缀形态。

        In Mongolian, MVS separates a stem from its suffix (like a soft hyphen).
        The a/e letter immediately after MVS takes a special "chachlag" glyph
        that visually connects to the preceding stem differently than normal.
        在蒙古文中，MVS 将词干与词缀分隔开（类似软连字符）。
        MVS 后紧跟的 a/e 字母取特殊的"chachlag"字形，与前面词干的视觉连接方式不同于常规形式。
        """
        for i, tok in enumerate(tokens):
            if not tok.is_letter or tok.alias not in ("a", "e"):
                continue
            prev = self._prev_tok(tokens, i)
            if prev and prev.is_mvs:
                nxt = self._next_tok(tokens, i)
                if nxt and self._has_fvs(tok):
                    # follows MVS and has FVS → default (no condition)
                    pass
                else:
                    tok.condition = "chachlag"
    
    def _step2_syllabic(self, tokens):
        """
        Step 2: Syllabic — complex consonant/vowel context rules.
        第2步：音节规则——复杂的辅音/元音上下文规则。

        This is the largest and most complex step. It examines each letter's
        neighbors to determine its syllabic role:
        这是最大也最复杂的步骤。它检查每个字母的邻居来确定其音节角色：

          - o/u/oe/ue: "marked" form after initial consonant (tall vs short stem)
            o/u/oe/ue：首辅音后取"标记"形式（高杆 vs 矮杆）
          - n/t/d: "onset" before vowel, "devsger" after vowel (tooth direction)
            n/t/d：元音前为"起始"，元音后为"连接"（齿的方向）
          - h/g (QA/GA): masculine/feminine based on adjacent vowel harmony
            h/g（QA/GA）：根据相邻元音和谐确定阳性/阴性
          - d: "marked" before final vowel (different stroke)
            d：尾元音前取"标记"形式（不同笔画）
          - sh: "dotless" before i (dot omitted to avoid collision)
            sh：i 前取"无点"形式（省略点以避免碰撞）
          - g: "dotless" after s or d
            g：s 或 d 后取"无点"形式
        """
        for i, tok in enumerate(tokens):
            if not tok.is_letter:
                continue
            if tok.condition:
                continue  # already assigned by step 1
            if tok.fvs_cp is not None:
                continue  # explicit FVS → default
            
            alias = tok.alias
            pos = tok.position
            prev = self._prev_letter(tokens, i)
            nxt = self._next_letter(tokens, i)
            nxt_tok = self._next_tok(tokens, i)
            
            # --- o, u, oe, ue: marked/default ---
            # "Marked" = the tall-stem form used after an initial consonant.
            # Without this, o/u would use the short-stem default form.
            # "标记" = 首辅音后使用的高杆形式。没有这个，o/u 会使用矮杆默认形式。
            if alias in ("o", "u", "oe", "ue"):
                # if follows an initial consonant → marked
                if prev and self._is_consonant(prev) and prev.position == "init":
                    tok.condition = "marked"
                    continue
                # if precedes an FVS or follows an FVS → default
                if self._has_fvs(tok):
                    continue  # default
                # oe, ue special: if medial and follows consonant cluster starting from init
                if alias in ("oe", "ue") and pos == "medi":
                    if prev and self._is_consonant(prev) and prev.position == "medi":
                        # check if there's an init consonant before
                        pp = self._prev_letter(tokens, prev.index if hasattr(prev, 'index') else i-1)
                        if pp and self._is_consonant(pp) and pp.position == "init":
                            tok.condition = "marked"
                            continue
            
            # --- d: marked if precedes final vowel without FVS ---
            if alias == "d":
                if nxt and self._is_vowel(nxt) and nxt.position == "fina" and not self._has_fvs(nxt):
                    tok.condition = "marked"
                    continue
            
            # --- n, j, w: chachlag_onset (before MVS + isolated a/e) ---
            if alias in ("n", "j", "w"):
                if nxt_tok and nxt_tok.is_mvs:
                    nxt_after_mvs = self._next_letter(tokens, i + 1) if i + 1 < len(tokens) else None
                    if nxt_after_mvs and nxt_after_mvs.alias in ("a", "e") and nxt_after_mvs.position == "isol":
                        tok.condition = "chachlag_onset"
                        continue
            
            # --- h, g: chachlag_onset (before MVS + isolated a) ---
            if alias in ("h", "g"):
                if nxt_tok and nxt_tok.is_mvs:
                    nxt_after_mvs = self._next_letter(tokens, i + 1) if i + 1 < len(tokens) else None
                    if nxt_after_mvs:
                        if nxt_after_mvs.alias == "a" and nxt_after_mvs.position == "isol":
                            tok.condition = "chachlag_onset"
                            continue
                        if alias == "g" and nxt_after_mvs.alias == "e" and nxt_after_mvs.position == "isol":
                            tok.condition = "chachlag_onset"  # chachlag_onset_gb
                            continue
            
            # --- n, t, d: onset/devsger ---
            # "Onset" = before a vowel (beginning of syllable), shows a different tooth.
            # "Devsger" = after a vowel (connecting to next syllable), tooth faces the other way.
            # "起始(onset)" = 元音前（音节开头），显示不同的齿。
            # "连接(devsger)" = 元音后（连接下一音节），齿朝向相反。
            if alias in ("n", "t", "d"):
                if nxt and self._is_vowel(nxt):
                    tok.condition = "onset"
                    continue
                if prev and self._is_vowel(prev):
                    tok.condition = "devsger"
                    continue
            
            # --- h, g: masculine/feminine ---
            # h (QA) and g (GA) share many glyph forms but are distinguished by
            # vowel harmony. In a masculine word (containing o/u), use h (QA);
            # in feminine (oe/ue/ee), use g (GA). This is the primary ambiguity
            # that normalization must resolve.
            # h (QA) 和 g (GA) 共享许多字形但通过元音和谐来区分。
            # 在阳性词（含 o/u）中用 h (QA)；在阴性词（含 oe/ue/ee）中用 g (GA)。
            # 这是规范化必须解决的主要歧义。
            if alias in ("h", "g"):
                if nxt and self._is_masc_vowel(nxt):
                    tok.condition = "masculine_onset"
                    continue
                if nxt and (self._is_fem_vowel(nxt) or self._is_neut_vowel(nxt)):
                    tok.condition = "feminine"
                    continue
                if prev and self._is_masc_vowel(prev):
                    tok.condition = "masculine_devsger"
                    continue
                if prev and self._is_fem_vowel(prev):
                    tok.condition = "feminine"
                    continue
                # Remote vowel harmony scan: when no adjacent vowel reveals harmony,
                # scan the entire word for any unambiguous vowel.
                # 远程元音和谐扫描：当相邻元音无法揭示和谐类型时，扫描整词寻找明确元音。
                cond = self._scan_vowel_harmony(tokens, i)
                if cond:
                    tok.condition = cond
                    continue
                tok.condition = "feminine"  # default fallback
                continue
            
            # --- t: devsger before ee or consonant ---
            if alias == "t":
                if nxt and (nxt.alias == "ee" or self._is_consonant(nxt)):
                    tok.condition = "devsger"
                    continue
            
            # --- sh: dotless before i ---
            if alias == "sh":
                if pos == "init" and nxt and nxt.alias == "i" and nxt.position == "medi":
                    tok.condition = "dotless"
                    continue
                if pos == "medi" and nxt and nxt.alias == "i":
                    tok.condition = "dotless"
                    continue
            
            # --- g: dotless after s or d ---
            if alias == "g":
                if prev and prev.alias in ("s", "d"):
                    tok.condition = "dotless"
                    continue
    
    def _scan_vowel_harmony(self, tokens, idx):
        """
        Remote vowel harmony scan for h/g.
        h/g 的远程元音和谐扫描。

        When h or g has no immediately adjacent vowel, we scan the entire word
        to find any unambiguous masculine/feminine vowel. Backward scan takes
        priority (preceding context is more reliable for harmony).
        当 h 或 g 没有紧邻的元音时，扫描整词寻找任何明确的阳性/阴性元音。
        向后扫描优先（前方上下文对和谐判断更可靠）。
        """
        # Scan backwards for vowels / 向后扫描元音
        for i in range(idx - 1, -1, -1):
            if not tokens[i].is_letter:
                continue
            if self._is_masc_vowel(tokens[i]):
                return "masculine_devsger"
            if self._is_fem_vowel(tokens[i]):
                return "feminine"
        # Scan forwards
        for i in range(idx + 1, len(tokens)):
            if not tokens[i].is_letter:
                continue
            if self._is_masc_vowel(tokens[i]):
                return "masculine_devsger"
            if self._is_fem_vowel(tokens[i]):
                return "feminine"
        return None
    
    def _step3_particle(self, tokens):
        """
        Step 3: Particle — MVS particle dictionary lookup.
        第3步：助词——MVS 助词词典查找。

        Mongolian particles (grammatical suffixes) are preceded by MVS and have
        fixed glyph forms. Each MVS-delimited sub-sequence is matched against
        the particle dictionary independently. Matching tokens get the "particle"
        condition, overriding any syllabic condition from step 2.
        蒙古文助词（语法后缀）前面有 MVS，具有固定的字形形式。每个 MVS 分隔的
        子序列独立匹配助词词典。匹配的标记获得"particle"条件，覆盖第2步的音节条件。
        """
        # Build segments split at MVS boundaries, each starting with MVS
        # 构建以 MVS 为起点的分段
        segments = []  # list of (aliases, tok_indices) per MVS-delimited sub-sequence
        current_aliases = []
        current_indices = []
        for i, tok in enumerate(tokens):
            if tok.is_mvs:
                if current_aliases:
                    segments.append((current_aliases, current_indices))
                current_aliases = [tok.alias]
                current_indices = [i]
            elif tok.is_letter:
                current_aliases.append(tok.alias)
                current_indices.append(i)
        if current_aliases:
            segments.append((current_aliases, current_indices))
        
        # Match each segment against the particle dictionary
        for aliases, tok_indices in segments:
            alias_str = " ".join(aliases)
            particle_indices = self.particle_dict.get(alias_str)
            if particle_indices is not None:
                for pidx in particle_indices:
                    if pidx < len(tok_indices):
                        real_idx = tok_indices[pidx]
                        tok = tokens[real_idx]
                        if tok.is_letter and tok.alias in ("a", "e", "i", "u", "ue", "d", "y"):
                            tok.condition = "particle"
    
    def _step4_devsger(self, tokens):
        """
        Step 4: Devsger — i after vowel gets vowel_devsger (double tooth).
        第4步：连接齿——元音后的 i 取 vowel_devsger（双齿形）。

        When 'i' appears in medial position after a vowel, it renders as a
        double-tooth glyph (two 'I' written units) instead of a single tooth.
        This is the visual "devsger" (connecting tooth) that distinguishes
        vowel+i sequences from consonant+i.
        当 'i' 出现在元音后的中间位置时，渲染为双齿字形（两个 'I' 书写单元），
        而非单齿。这就是区分 元音+i 和 辅音+i 序列的视觉"连接齿"。

        Example / 例: ᠠᠢᠯ (ail) → A + I,I + L (the I after A gets double-tooth)
        """
        for i, tok in enumerate(tokens):
            if not tok.is_letter or tok.alias != "i" or tok.position != "medi":
                continue
            if tok.condition:
                continue  # already assigned
            if tok.fvs_cp is not None:
                continue  # explicit FVS
            
            prev = self._prev_letter(tokens, i)
            if prev and self._is_vowel(prev):
                # Resolve prev's written to check if it ends with I
                self._resolve_token_written(prev)
                if not self._written_ends_with(prev, "I"):
                    tok.condition = "vowel_devsger"
    
    def _step5_post_bowed(self, tokens):
        """
        Step 5: Post-bowed — vowels after bowed consonants get special forms.
        第5步：弓形后续——弓形辅音后的元音取特殊形态。

        Bowed consonants (G, Gx, K, K2, B, P, F) end with a rightward curve.
        A vowel following a bowed consonant needs a modified connection form
        to attach smoothly to the bowed stroke.
        弓形辅音（G, Gx, K, K2, B, P, F）末笔向右弯曲。
        弓形辅音后的元音需要修改连接形式以平滑地接入弓形笔画。

        Example / 例: ᠥᠭᠡ (üge) — the final 'e' after GA(bowed) takes post_bowed form
        """
        for i, tok in enumerate(tokens):
            if not tok.is_letter:
                continue
            if tok.condition:
                continue
            if tok.fvs_cp is not None:
                continue
            
            alias = tok.alias
            
            if alias in ("o", "u", "oe", "ue"):
                prev = self._prev_letter(tokens, i)
                if prev:
                    self._resolve_token_written(prev)
                    if prev.written and prev.written[-1] in BOWED_UNITS:
                        # Check if this token would be in written form U
                        # (final position produces U for o/u/oe/ue)
                        tok.condition = "post_bowed"
                        continue
            
            if alias in ("a", "e"):
                prev = self._prev_letter(tokens, i)
                if prev:
                    self._resolve_token_written(prev)
                    if prev.written and prev.written[-1] in BOWED_UNITS:
                        tok.condition = "post_bowed"
                        continue
    
    # ── Main pipeline / 主管线 ───────────────────────────────────

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
        """
        tokens = self.tokenize(text)
        self.assign_positions(tokens)
        
        # 5-step condition mapping
        self._step1_chachlag(tokens)
        self._step2_syllabic(tokens)
        self._step3_particle(tokens)
        self._step4_devsger(tokens)
        self._step5_post_bowed(tokens)
        
        # Resolve all written
        for tok in tokens:
            self._resolve_token_written(tok)
        
        # Collect written units (MVS emits a boundary marker)
        result = []
        for tok in tokens:
            if tok.is_mvs:
                result.append("mvs")
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
        """Return detailed shaping breakdown per token. / 返回每个标记的详细字形分解。"""
        tokens = self.tokenize(text)
        self.assign_positions(tokens)
        self._step1_chachlag(tokens)
        self._step2_syllabic(tokens)
        self._step3_particle(tokens)
        self._step4_devsger(tokens)
        self._step5_post_bowed(tokens)
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
    
    # ── Reverse shaping (unshape) / 反向字形处理 ──────────────────
    # Reverse shaping answers: "given these visual glyphs, what Unicode letters
    # produced them?" This is the inverse of the forward pipeline and is
    # essential for normalization: shape → reverse shape → canonical Unicode.
    # 反向字形处理回答："给定这些视觉字形，是哪些 Unicode 字母产生了它们？"
    # 这是正向管线的逆过程，对规范化至关重要：字形 → 反向字形 → 规范 Unicode。

    def build_reverse_map(self):
        """
        Build reverse lookup: (position, written_tuple) → canonical (cp, fvs_int)
        构建反向查找：（位置, 书写单元元组）→ 规范（码位, FVS编号）

        For normalization: given a shape sequence, find the canonical Unicode encoding.
        用于规范化：给定字形序列，找到规范的 Unicode 编码。
        Prefers: default=True, non-archaic, non-unrecommended, lowest codepoint.
        优先选择：default=True、非古体、非不推荐、最低码位。
        """
        self._reverse_map = {}  # (pos, written) → (cp, fvs_int)
        
        candidates = {}  # (pos, written) → list of (cp, fvs_int, is_default, ...)
        
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
                    locale_data = locales[self.locale]
                    w_raw = locale_data.get("written") or vdata.get("written")
                    written = self._resolve_written(w_raw, char_name)
                    if not written:
                        continue
                    
                    key = (pos, written)
                    if key not in candidates:
                        candidates[key] = []
                    candidates[key].append({
                        "cp": cp, "fvs": fvs_int,
                        "default": vdata.get("default", False),
                        "archaic": locale_data.get("archaic", False),
                        "unrecommended": locale_data.get("unrecommended", False),
                    })
        
        for key, cands in candidates.items():
            best = None
            for c in cands:
                if c["archaic"] or c["unrecommended"]:
                    continue
                if best is None:
                    best = c
                elif c["default"] and not best["default"]:
                    best = c
                elif c["default"] == best["default"] and c["cp"] < best["cp"]:
                    best = c
            if best:
                self._reverse_map[key] = (best["cp"], best["fvs"])
    
    def unshape(self, written_units, positions):
        """
        Reverse shape: written units + positions → canonical Unicode sequence.
        
        Args:
            written_units: list of (written_tuple, position) per letter
        Returns:
            canonical Unicode string
        """
        if not hasattr(self, '_reverse_map'):
            self.build_reverse_map()
        
        result = []
        for written, pos in zip(written_units, positions):
            written_t = tuple(written) if isinstance(written, list) else written
            key = (pos, written_t)
            canon = self._reverse_map.get(key)
            if canon:
                cp, fvs_int = canon
                result.append(chr(cp))
                fvs_cp = FVS_INT_TO_CP.get(fvs_int)
                if fvs_cp:
                    result.append(chr(fvs_cp))
            else:
                result.append("?")
        return "".join(result)
    
    def _detect_vowel_harmony(self, tokens):
        """
        Detect the vowel harmony class of a word.
        检测一个词的元音和谐类别。

        Mongolian vowel harmony is a phonological constraint: a native word
        contains EITHER masculine vowels (o, u) OR feminine vowels (oe, ue, ee),
        never both. The neuter vowel 'i' can appear in either class.
        蒙古文元音和谐是一种语音学约束：一个本族词只包含阳性元音（o, u）或
        阴性元音（oe, ue, ee），不会两者兼有。中性元音 'i' 可以出现在任一类中。

        This matters for normalization because a/e and h/g are harmony-ambiguous
        pairs: 'a' appears in masculine words, 'e' in feminine, but both produce
        the SAME glyph in medial/final positions. Knowing the harmony class lets
        us pick the correct canonical letter.
        这对规范化很重要，因为 a/e 和 h/g 是和谐歧义对：'a' 出现在阳性词中，
        'e' 出现在阴性词中，但两者在中间/尾部位置产生相同的字形。
        知道和谐类别让我们能选择正确的规范字母。

        Priority order / 优先级:
        1. Unambiguous vowels: o/u → masculine, oe/ue/ee → feminine
           明确元音：o/u → 阳性，oe/ue/ee → 阴性
        2. h/g condition from syllabic step (most reliable when a/e is ambiguous)
           第2步中 h/g 的条件（当 a/e 歧义时最可靠）
        3. Default to masculine (conventional choice)
           默认阳性（惯例选择）
        """
        # Priority 1: unambiguous vowels
        has_masc = False
        has_fem = False
        UNAMB_MASC = {"o", "u"}
        UNAMB_FEM = {"oe", "ue", "ee"}
        
        for tok in tokens:
            if not tok.is_letter:
                continue
            if tok.alias in UNAMB_MASC:
                has_masc = True
            elif tok.alias in UNAMB_FEM:
                has_fem = True
        
        if has_masc and not has_fem:
            return "masculine"
        if has_fem and not has_masc:
            return "feminine"
        if has_masc and has_fem:
            # Mixed — use first unambiguous vowel
            for tok in tokens:
                if tok.alias in UNAMB_MASC:
                    return "masculine"
                if tok.alias in UNAMB_FEM:
                    return "feminine"
        
        # Priority 2: h/g condition from syllabic step
        # These conditions are computed from vowel context, so they're reliable
        for tok in tokens:
            if tok.alias not in ("h", "g"):
                continue
            if tok.condition == "feminine":
                return "feminine"
            if tok.condition in ("masculine_onset", "masculine_devsger"):
                return "masculine"
        
        # Priority 3: default to masculine
        return "masculine"
    
    def _get_candidates(self, pos, written):
        """Get all (cp, fvs_int) candidates that produce the given written at pos."""
        if not hasattr(self, '_candidates_map'):
            self._build_candidates_map()
        return self._candidates_map.get((pos, written), [])
    
    def _build_candidates_map(self):
        """Build (pos, written) → list of candidate dicts."""
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
                    archaic = locale_data.get("archaic", False)
                    unrec = locale_data.get("unrecommended", False)
                    if archaic or unrec:
                        continue
                    
                    key = (pos, written)
                    if key not in self._candidates_map:
                        self._candidates_map[key] = []
                    self._candidates_map[key].append({
                        "cp": cp, "fvs": fvs_int, "alias": alias,
                        "default": vdata.get("default", False),
                    })
    
    # Harmony-aware letter pairs: these letter pairs are visually indistinguishable
    # in certain positions, so the choice between them depends on vowel harmony.
    # 和谐相关字母对：这些字母对在某些位置视觉上不可区分，因此它们之间的选择取决于元音和谐。
    HARMONY_PAIRS = {
        # (masculine_alias, feminine_alias)
        ("a", "e"),    # same medi/fina written / 中/尾部书写单元相同
        ("h", "g"),    # QA/GA — same written in many positions / 多个位置书写单元相同
    }
    
    def _pick_by_harmony(self, candidates, harmony, original_alias, pos=None, written=None):
        """
        Pick the best candidate considering vowel harmony.
        考虑元音和谐选择最佳候选字母。

        This is the heart of normalization's letter selection. Multiple Unicode
        letters can produce the same written unit at the same position. We must
        pick exactly one as canonical.
        这是规范化字母选择的核心。多个 Unicode 字母可以在同一位置产生相同的
        书写单元。我们必须精确选择一个作为规范形式。

        Strategy / 策略:
          1. If original letter is among candidates AND not part of a harmony pair
             → preserve it (minimal change principle)
             如果原字母在候选中且不属于和谐对 → 保留（最小变更原则）
          2. For a/e pair: masculine→a, feminine→e
             a/e 对：阳性→a，阴性→e
          3. For h/g pair: masculine→h, feminine→g
             h/g 对：阳性→h，阴性→g
          4. For other ambiguities: keep original if candidate, else pick default
             其他歧义：保留原字母（如果是候选），否则选默认
        """
        if not candidates:
            return None
        
        # Check if original letter is among candidates
        orig_candidates = [c for c in candidates if c["alias"] == original_alias]
        
        # If original is a candidate AND is NOT part of a harmony pair → preserve it immediately
        # This prevents e.g. 'n' being replaced by 'a' just because they share the same written
        cand_aliases = {c["alias"] for c in candidates}
        harmony_aliases = set()
        for masc_alias, fem_alias in self.HARMONY_PAIRS:
            if masc_alias in cand_aliases and fem_alias in cand_aliases:
                harmony_aliases.add(masc_alias)
                harmony_aliases.add(fem_alias)
        
        if orig_candidates and original_alias not in harmony_aliases:
            # Original is valid but not part of a harmony pair.
            # 
            # Preserve original ONLY when:
            #   1. No harmony ambiguity at all among candidates, OR
            #   2. Position is init/fina/isol AND original is NOT producing a
            #      "foreign" written (e.g. consonant NA producing vowel-like 'A')
            #
            # A consonant producing vowel written = likely misencoded → replace
            # A letter at boundary producing its natural written = keep
            
            if not harmony_aliases:
                # No a/e or h/g pair in candidates → no ambiguity → preserve original
                for c in orig_candidates:
                    if c["default"]:
                        return c
                return orig_candidates[0]
            
            # Harmony pair exists. Check if original is "naturally" producing this written.
            # A consonant whose default at THIS position produces the same written 
            # is "acting as" that vowel form → should be replaced by the vowel.
            # But at word boundaries (init/fina), if original's bare form naturally
            # produces this written, it might be correct (e.g. NA@fina → 'A' is normal).
            is_boundary = pos in ("init", "fina", "isol")
            if is_boundary:
                # At boundaries: preserve original (NA@fina → 'A' is the normal form of N in final)
                for c in orig_candidates:
                    if c["default"]:
                        return c
                return orig_candidates[0]
            
            # In medi: let harmony pick the canonical vowel letter
            # This replaces e.g. NA@medi(producing 'A') with A@medi
        
        # Apply harmony resolution for a/e, h/g pairs
        for masc_alias, fem_alias in self.HARMONY_PAIRS:
            if masc_alias in cand_aliases and fem_alias in cand_aliases:
                if harmony == "masculine":
                    target_alias = masc_alias
                elif harmony == "feminine":
                    target_alias = fem_alias
                else:
                    if original_alias in (masc_alias, fem_alias):
                        target_alias = original_alias
                    else:
                        target_alias = masc_alias
                
                target_cands = [c for c in candidates if c["alias"] == target_alias]
                if target_cands:
                    for c in target_cands:
                        if c["default"]:
                            return c
                    return target_cands[0]
        
        # Preserve original if possible
        if orig_candidates:
            for c in orig_candidates:
                if c["default"]:
                    return c
            return orig_candidates[0]
        
        # Original not among candidates → pick default
        for c in candidates:
            if c["default"]:
                return c
        return candidates[0]
    
    def normalize(self, text):
        """
        Normalize text to canonical bare-Unicode encoding.
        将文本规范化为规范的裸 Unicode 编码。

        MINIMAL ENCODING principle: bare Unicode (no FVS) is the canonical form.
        最小编码原则：裸 Unicode（无 FVS）是规范形式。

        Key insight / 核心洞察:
          The shaping engine automatically selects the correct default variant
          for bare letters based on context. So we only need to:
          字形引擎会根据上下文自动为裸字母选择正确的默认变体。所以我们只需：
            1. Select the CORRECT LETTER (resolving a/e and h/g via vowel harmony)
               选择正确的字母（通过元音和谐解决 a/e 和 h/g 歧义）
            2. Output bare Unicode (no FVS)
               输出裸 Unicode（无 FVS）

        Algorithm / 算法:
          1. Shape input → get written units per token
             字形处理输入 → 获取每个标记的书写单元
          2. Detect vowel harmony from unambiguous vowels
             从明确元音检测元音和谐
          3. Merge adjacent identical single-unit written (I+I → devsger)
             合并相邻的相同单书写单元（I+I → 连接齿）
          4. For each token, pick canonical letter via harmony + original preservation
             为每个标记通过和谐+原字母保留选择规范字母
          5. Output bare Unicode (no FVS for default variants)
             输出裸 Unicode（默认变体无 FVS）

        Example / 例: "ᠰᠡᠢᠨ" (sain with E) → "ᠰᠠᠢᠨ" (sain with A)
          E→A because this is a masculine word (no oe/ue/ee), so a/e resolves to 'a'.
          E→A 因为这是阳性词（无 oe/ue/ee），所以 a/e 解析为 'a'。
        """
        tokens = self.tokenize(text)
        self.assign_positions(tokens)
        self._step1_chachlag(tokens)
        self._step2_syllabic(tokens)
        self._step3_particle(tokens)
        self._step4_devsger(tokens)
        self._step5_post_bowed(tokens)
        for tok in tokens:
            self._resolve_token_written(tok)
        
        if not hasattr(self, '_reverse_map'):
            self.build_reverse_map()
        
        # Detect vowel harmony — this determines whether a/e → 'a' or 'e', h/g → 'h' or 'g'
        # 检测元音和谐——决定 a/e → 'a' 还是 'e'，h/g → 'h' 还是 'g'
        harmony = self._detect_vowel_harmony(tokens)

        # Build segments preserving original alias (needed for harmony resolution later)
        # 构建段落，保留原始别名（后续和谐解析需要）
        segments = []  # (type, written, original_alias) or ('mvs', (), '')
        for tok in tokens:
            if tok.is_mvs:
                segments.append(('mvs', (), ''))
            elif tok.is_letter and tok.written:
                segments.append(('letter', tok.written, tok.alias))
        
        # Merge identical adjacent single-unit letter segments.
        # This handles the YA+FVS1 + YA+FVS1 → single I case: two tokens each
        # producing ('I',) get merged into one token producing ('I', 'I'),
        # which maps to the devsger form of the letter 'i'.
        # 合并相邻的相同单书写单元字母段。
        # 这处理了 YA+FVS1 + YA+FVS1 → 单个 I 的情况：两个各产生 ('I',) 的标记
        # 合并为一个产生 ('I', 'I') 的标记，映射到字母 'i' 的连接齿形式。
        changed = True
        while changed:
            changed = False
            new_segments = []
            i = 0
            while i < len(segments):
                if segments[i][0] == 'mvs':
                    new_segments.append(segments[i])
                    i += 1
                    continue
                
                cur_written = segments[i][1]
                cur_alias = segments[i][2]
                
                if (i + 1 < len(segments)
                        and segments[i + 1][0] == 'letter'
                        and segments[i + 1][1] == cur_written
                        and len(cur_written) == 1):
                    
                    combined = cur_written + cur_written
                    # Check if combined exists in any medi position
                    letter_before = sum(1 for s in new_segments if s[0] == 'letter')
                    letter_after = sum(1 for s in segments[i + 2:] if s[0] == 'letter')
                    total = letter_before + 1 + letter_after
                    
                    if total == 1: est_pos = "isol"
                    elif letter_before == 0: est_pos = "init"
                    elif letter_after == 0: est_pos = "fina"
                    else: est_pos = "medi"
                    
                    if (est_pos, combined) in self._reverse_map:
                        # Keep the alias of the first token for harmony resolution
                        new_segments.append(('letter', combined, cur_alias))
                        i += 2
                        changed = True
                        continue
                
                new_segments.append(segments[i])
                i += 1
            segments = new_segments
        
        # Assign positions and pick canonical letters.
        # Re-derive positions from the merged segment list (not original tokens,
        # since merging may have changed the letter count).
        # MVS breaks the joining chain, so each MVS-delimited group gets
        # positions assigned independently.
        # 分配位置并选择规范字母。
        # 从合并后的段落列表重新推导位置（不是原始标记，因为合并可能改变了字母数量）。
        # MVS 断开连接链，每个 MVS 分隔的组独立分配位置。
        
        # Split segments into groups at MVS boundaries
        groups = []
        current_group = []
        for seg in segments:
            if seg[0] == 'mvs':
                if current_group:
                    groups.append(current_group)
                groups.append([seg])  # MVS as its own group
                current_group = []
            else:
                current_group.append(seg)
        if current_group:
            groups.append(current_group)
        
        result = []
        
        for group in groups:
            if len(group) == 1 and group[0][0] == 'mvs':
                # Always output MVS — NNBSP was already normalized during tokenization.
                # 始终输出 MVS——NNBSP 已在分词阶段被规范化。
                result.append(chr(MVS_CP))
                continue
            
            n_letters = len(group)
            for letter_seq, seg in enumerate(group):
                written = seg[1]
                orig_alias = seg[2]
                
                if n_letters == 1: pos = "isol"
                elif letter_seq == 0: pos = "init"
                elif letter_seq == n_letters - 1: pos = "fina"
                else: pos = "medi"
                
                # Get all candidates for this (pos, written)
                candidates = self._get_candidates(pos, written)
                
                # Pick best candidate using harmony + original preservation
                best = self._pick_by_harmony(candidates, harmony, orig_alias, pos=pos, written=written)
                
                if best:
                    # BARE ENCODING: output only the letter codepoint, no FVS.
                    # The shaping engine will automatically pick the correct default
                    # variant based on context — this is the entire point of normalization.
                    # 裸编码：只输出字母码位，不加 FVS。
                    # 字形引擎会根据上下文自动选择正确的默认变体——这正是规范化的意义所在。
                    result.append(chr(best["cp"]))
                else:
                    # Absolute fallback: preserve original token  
                    result.append(f"<{'|'.join(written)}>")
        
        return "".join(result)

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
            is_mong = is_mongolian_letter(cp) or cp in FVS_CPS or cp == MVS_CP or cp == NNBSP_CP
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

def main():
    import argparse
    import sys

    parser = argparse.ArgumentParser(
        prog="shaper",
        description="Mongolian shaping tool (UTN #57 v4)",
    )
    parser.add_argument("--locale", default="MNG", help="Locale (default: MNG)")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_shape = sub.add_parser("shape", help="Return written-unit sequence for TEXT")
    p_shape.add_argument("text")

    p_same = sub.add_parser("same", help="Check if TEXT1 and TEXT2 are visually identical")
    p_same.add_argument("text1")
    p_same.add_argument("text2")

    p_norm = sub.add_parser("normalize", help="Normalize TEXT to canonical bare Unicode")
    p_norm.add_argument("text")

    p_normt = sub.add_parser("normalize-text", help="Normalize full text (multi-word, mixed script)")
    p_normt.add_argument("text")

    args = parser.parse_args()
    shaper = MongolianShaper(locale=args.locale)

    if args.cmd == "shape":
        units = shaper.shape(args.text)
        print("+".join(units))
    elif args.cmd == "same":
        result = shaper.same_shape(args.text1, args.text2)
        print("true" if result else "false")
        sys.exit(0 if result else 1)
    elif args.cmd == "normalize":
        print(shaper.normalize(args.text))
    elif args.cmd == "normalize-text":
        print(shaper.normalize_text(args.text))


if __name__ == "__main__":
    main()
