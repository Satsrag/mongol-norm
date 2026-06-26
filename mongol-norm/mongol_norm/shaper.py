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
        if (is_mongolian_letter(cp)
                or cp in FVS_CPS
                or cp == MVS_CP
                or cp == NNBSP_CP
                or cp == NIRUGU_CP
                or cp == ZWJ_CP):
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

    def _prev_adjacent_letter(self, tokens, idx):
        """Return the nearest preceding letter REACHING idx without an
        intervening MVS. Nirugu (a joining marker) is transparent — we
        skip past it — but MVS (a word boundary) blocks.

        Use this when a rule requires GLYPH adjacency for joining
        purposes (e.g. iii2f.h_g.harmony's "adjacent vowel" checks).
        """
        j = idx - 1
        while j >= 0:
            tok = tokens[j]
            if tok.is_mvs:
                return None
            if tok.is_letter:
                return tok
            if tok.is_nirugu:
                j -= 1
                continue
            return None
        return None

    def _next_adjacent_letter(self, tokens, idx):
        """Mirror image of `_prev_adjacent_letter`."""
        j = idx + 1
        while j < len(tokens):
            tok = tokens[j]
            if tok.is_mvs:
                return None
            if tok.is_letter:
                return tok
            if tok.is_nirugu:
                j += 1
                continue
            return None
        return None

    def _has_fvs(self, tok):
        return tok.fvs_cp is not None
    
    def _written_ends_with(self, tok, unit):
        return tok.written and tok.written[-1] == unit
    
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

        # Post-process: rewrite single-letter chains so non-chachlag
        # particles get their explicit particle-FVS encoding instead of
        # relying on positional context (MVS / init-form rendering).
        # See `_apply_particle_substitution` for the rule.
        # 后处理:把单字母 chain 改为显式 particle 编码(非 chachlag),
        # 不依赖 MVS / init-形态渲染。
        canonical = self._apply_particle_substitution(canonical, target)
        return canonical

    # Shapes that should KEEP the chachlag (bare a/e + MVS) encoding —
    # per user request, particle post-processing skips these. The chachlag
    # rule fires on a/e after MVS to give 'Aa'; explicit particle form
    # would replace bare a with a+fvs2, which the user wants to avoid.
    # 跳过 particle 替换的 shape:chachlag 'Aa' 保留 bare a/e + MVS 编码。
    _CHACHLAG_CHAIN_SHAPES = frozenset({('Aa',)})

    def _apply_particle_substitution(self, text, target_shape):
        """
        Post-process canonical text: for each single-letter chain whose
        context-aware shape matches a particle-tagged variant at isol,
        replace the chain encoding with that explicit particle form.
        对每个单字母 chain,若其上下文 shape 匹配 isol 的 particle 变体,
        将该 chain 改为显式 particle 编码。

        Handles two user-requested rules:
        处理用户提出的两条规则:
          1. shape ['I'] at iso always → i+fvs1 (not bare j)
             单字母 'I' 总归到 i+fvs1(而非 bare j)
          2. mvs + bare-particle → mvs + particle+fvs (mvs-independent rendering)
             mvs 后的裸 particle → mvs + particle+fvs(渲染不依赖 mvs)

        Chachlag (chain ['Aa']) is excluded; bare a/e + MVS stays.
        Chachlag(chain ['Aa'])排除;bare a/e + MVS 保留。

        Verifies shape-preservation per substitution; reverts on mismatch.
        每次替换都校验保形,不通过则回滚。
        """
        if not text:
            return text

        # Chunk text into (kind, body) where kind is 'mvs' or 'chain'.
        # 切分文本为 (kind, body),kind 是 'mvs' 或 'chain'。
        chunks = []
        cur = []
        for ch in text:
            if ord(ch) == MVS_CP:
                if cur:
                    chunks.append(['chain', ''.join(cur)])
                    cur = []
                chunks.append(['mvs', chr(MVS_CP)])
            else:
                cur.append(ch)
        if cur:
            chunks.append(['chain', ''.join(cur)])

        changed = False

        def chunk_ctx(i):
            """Return (prev_mvs, next_mvs, prefix, suffix) for chunk i."""
            pm = i > 0 and chunks[i - 1][0] == 'mvs'
            nm = i + 1 < len(chunks) and chunks[i + 1][0] == 'mvs'
            return pm, nm, (chr(MVS_CP) if pm else ''), (chr(MVS_CP) if nm else '')

        def chunk_chain_shape(i, body):
            pm, nm, p, su = chunk_ctx(i)
            with_ctx = tuple(self.shape(p + body + su))
            cs = with_ctx
            if pm:
                cs = cs[1:]
            if nm:
                cs = cs[:-1]
            return cs, with_ctx, p, su, pm

        # ── Pass 1: chains AFTER MVS use the standalone canonical so the
        #    encoding doesn't rely on MVS to fire particle / init-form
        #    rules. Skip chachlag (chain ('Aa',)) — user wants
        #    `mvs + bare a/e` preserved.
        #    MVS 后的 chain 改用 standalone canonical,不依赖 MVS 触发规则。
        #    Chachlag(chain ('Aa',))跳过,保留 `mvs + bare a/e`。
        for i, (kind, body) in enumerate(chunks):
            if kind != 'chain' or not body:
                continue
            pm, nm, prefix, suffix = chunk_ctx(i)
            if not pm:
                continue
            chain_shape, with_ctx_shape, _, _, _ = chunk_chain_shape(i, body)

            if chain_shape in self._CHACHLAG_CHAIN_SHAPES:
                # Strip FVS for chachlag: prefer `mvs + bare a/e` over
                # `mvs + a+fvs2`/`mvs + e+fvs1`. Try removing the FVS
                # from the body and verify shape preserved.
                # chachlag 去 FVS:把 a+fvs2/e+fvs1 还原为 bare。
                cps = [ord(c) for c in body]
                if len(cps) == 2 and is_mongolian_letter(cps[0]) and cps[1] in FVS_CPS:
                    bare_body = chr(cps[0])
                    if tuple(self.shape(prefix + bare_body + suffix)) == with_ctx_shape:
                        chunks[i] = ['chain', bare_body]
                        changed = True
                continue

            # Compute the standalone canonical of this chain shape (the
            # one that works without MVS context). If it verifies with
            # MVS prefix, swap it in. This makes `mvs + du` → `mvs +
            # d+u+fvs2` (and similar multi-letter particle chains).
            # 算 chain shape 的 standalone canonical,用 MVS 上下文校验。
            standalone = self._encode_chain_canonical(chain_shape, False, '', ())
            if not standalone or standalone == body:
                continue
            if tuple(self.shape(prefix + standalone + suffix)) == with_ctx_shape:
                chunks[i] = ['chain', standalone]
                changed = True

        # ── Pass 2: single-letter chains get isol-particle preference.
        #    Handles Rule 1 (`I` iso → `i+fvs1`, replacing bare `j`).
        #    单字母 chain 用 isol particle(规则 1:I iso → i+fvs1)。
        for i, (kind, body) in enumerate(chunks):
            if kind != 'chain' or not body:
                continue
            cps = [ord(c) for c in body]
            if len(cps) == 1:
                if not is_mongolian_letter(cps[0]):
                    continue
            elif len(cps) == 2:
                if not (is_mongolian_letter(cps[0]) and cps[1] in FVS_CPS):
                    continue
            else:
                continue

            chain_shape, with_ctx_shape, prefix, suffix, _ = chunk_chain_shape(i, body)
            if chain_shape in self._CHACHLAG_CHAIN_SHAPES:
                continue

            cands = self._candidates_map.get(('isol', chain_shape), [])
            particle_cands = sorted(
                ((c['cp'], FVS_INT_TO_CP.get(c['fvs'])) for c in cands if c.get('particle')),
                key=lambda p: (p[0], (p[1] or 0))
            )
            for cp, fvs_cp in particle_cands:
                if fvs_cp is None:
                    new_body = chr(cp)
                else:
                    new_body = chr(cp) + chr(fvs_cp)
                if new_body == body:
                    break  # already in particle form
                if tuple(self.shape(prefix + new_body + suffix)) == with_ctx_shape:
                    chunks[i] = ['chain', new_body]
                    changed = True
                    break

        if not changed:
            return text

        new_text = ''.join(body for _, body in chunks)
        # Whole-text safety verification — the per-chunk verify catches
        # local issues but a final shape() check guards against any
        # cross-chain interaction we might have missed. Compare as lists
        # so the equality works regardless of how the caller passed it.
        # 整体校验,防御跨链交互。比较 list 形式以匹配 shape() 返回值。
        if self.shape(new_text) != list(target_shape):
            return text  # post-process broke something; revert
        return new_text

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
        # Parse into structural (mvs) + chain segments.
        parts = []  # list of ('mvs', None) | ('chain', tuple)
        cur = []
        for u in shape_list:
            if u == 'mvs':
                if cur:
                    parts.append(('chain', tuple(cur)))
                    cur = []
                parts.append(('mvs', None))
            else:
                cur.append(u)
        if cur:
            parts.append(('chain', tuple(cur)))

        encoded = [None] * len(parts)
        suffix_text = ""
        suffix_target = ()
        for i in range(len(parts) - 1, -1, -1):
            kind, body = parts[i]
            if kind == 'mvs':
                encoded[i] = chr(MVS_CP)
                suffix_text = chr(MVS_CP) + suffix_text
                suffix_target = ('mvs',) + suffix_target
            else:
                prev_mvs = i > 0 and parts[i - 1][0] == 'mvs'
                chain_canon = self._encode_chain_canonical(
                    body, prev_mvs, suffix_text, suffix_target,
                )
                encoded[i] = chain_canon
                suffix_text = chain_canon + suffix_text
                suffix_target = body + suffix_target
        return "".join(encoded)

    def _encode_chain_canonical(self, chain_shape, prev_mvs, suffix_text, suffix_target):
        """Memoised canonical encoding for a chain shape under MVS / suffix context."""
        if not hasattr(self, '_chain_canon_cache'):
            self._chain_canon_cache = {}
        cache_key = (chain_shape, prev_mvs, suffix_text, suffix_target)
        cached = self._chain_canon_cache.get(cache_key)
        if cached is not None:
            return cached
        result = self._compute_chain_canonical(chain_shape, prev_mvs, suffix_text, suffix_target)
        self._chain_canon_cache[cache_key] = result
        return result

    # Nirugu-wrap permutations tried per chain. Each (pre, post) shifts
    # letter positions: e.g. (1,1) puts a single letter at 'medi' instead
    # of 'isol', enabling shapes like ['O'] that only appear in medi.
    # Multiple nirugus don't change shape further, so {0,1}² covers it.
    # 每个 chain 尝试的 nirugu 包裹组合。如 (1,1) 把单字母放到 medi,
    # 这样像 ['O'] 这类只出现在 medi 的 shape 才有编码。
    _NIRUGU_WRAPS = ((0, 0), (0, 1), (1, 0), (1, 1))

    def _build_pos_tables(self):
        """
        Per-position lookup: {pos: {written_tuple: [(cp, fvs_cp_or_None), ...]}}.
        每位置查表:{位置: {written 元组: 候选列表}}.

        Candidates sorted by rule A preference: bare (no FVS) first, then
        by cp ascending, then by fvs_cp ascending. The greedy encoder
        iterates candidates in this order so the first valid match found
        is the canonical (rule A) choice for the slot.
        候选按规则 A 偏好排序:裸字母优先,然后 cp、fvs_cp 升序。贪婪
        编码器按此顺序枚举,首个有效匹配即为该槽的 canonical 选择。

        After building raw tables, runs a pre-filter pass that probes each
        (cp, fvs_cp) candidate in plausible neighbor contexts (vowel-prev,
        consonant-prev, etc.) and keeps only those whose shaped output
        for the letter actually equals the slot's `written`. This removes
        speculative bare candidates added by `_build_candidates_map`
        (e.g. bare `j` for slot (medi, ('A',))) that never produce the
        right written in any real context, drastically pruning DFS dead
        branches.
        构建后做预过滤:用合理的邻居上下文测试每个候选,只保留实际能产
        出 slot 的 written 的那些。剔除 `_build_candidates_map` 的投机
        裸候选(如 (medi, ('A',)) slot 里的裸 j),大幅剪枝 DFS 死分支。
        """
        if not hasattr(self, '_candidates_map'):
            self._build_candidates_map()
        self._pos_tables = {}
        for (pos, written), raw in self._candidates_map.items():
            seen = set()
            pairs = []
            for c in raw:
                fvs_cp = FVS_INT_TO_CP.get(c['fvs'])
                key = (c['cp'], fvs_cp)
                if key not in seen:
                    seen.add(key)
                    pairs.append(key)
            # bare-first, then cp asc, then fvs_cp asc (None < any int)
            pairs.sort(key=lambda p: (p[1] is not None, p[0], p[1] or 0))
            self._pos_tables[(pos, written)] = pairs

        self._filter_pos_tables()

    # Neighbor letters used to probe whether a candidate actually produces
    # the slot's written in plausible contexts. 'a' covers vowel neighbors
    # (triggers vowel_devsger, onset/devsger on n/t/d, etc.); 'n' covers
    # consonant neighbors (default rules).
    # 测试邻居:'a' 覆盖元音(触发 vowel_devsger / n.onset 等),'n' 覆盖辅音。
    _PROBE_VOWEL_CP = 0x1820  # MONGOLIAN LETTER A
    _PROBE_CONS_CP = 0x1828   # MONGOLIAN LETTER NA

    def _probe_contexts(self, pos, letter_text):
        """Yield test strings that place `letter_text` at `pos`."""
        v = chr(self._PROBE_VOWEL_CP)
        c = chr(self._PROBE_CONS_CP)
        if pos == 'isol':
            yield letter_text
        elif pos == 'init':
            yield letter_text + v
            yield letter_text + c
        elif pos == 'fina':
            yield v + letter_text
            yield c + letter_text
        else:  # medi
            yield v + letter_text + v
            yield c + letter_text + c
            yield v + letter_text + c
            yield c + letter_text + v

    def _filter_pos_tables(self):
        """
        Remove candidates that don't produce the slot's `written` in any
        plausible test context. See `_build_pos_tables` for rationale.
        剔除任何测试上下文都产不出 slot written 的候选。
        """
        # Cache per-letter probe results to avoid re-shaping the same
        # (cp, fvs_cp, pos) combo across many slots.
        # 缓存 (cp, fvs_cp, pos) 的探测结果,避免跨 slot 重复 shape。
        probe_cache = {}

        def probe(cp, fvs_cp, pos):
            key = (cp, fvs_cp, pos)
            if key in probe_cache:
                return probe_cache[key]
            letter_text = chr(cp) + (chr(fvs_cp) if fvs_cp is not None else '')
            results = set()
            for test_text in self._probe_contexts(pos, letter_text):
                try:
                    details = self.shape_detailed(test_text)
                except Exception:
                    continue
                # Find the test letter (cp matches) and record its written.
                # The probe letter sits at index wrap_pre in the chain;
                # match by cp ordinal.
                for d in details:
                    if d['cp'] == f'U+{cp:04X}':
                        if d.get('written'):
                            results.add(tuple(d['written']))
                        break
            probe_cache[key] = results
            return results

        for key in list(self._pos_tables.keys()):
            pos, written = key
            filtered = []
            for cp, fvs_cp in self._pos_tables[key]:
                produced = probe(cp, fvs_cp, pos)
                if written in produced:
                    filtered.append((cp, fvs_cp))
            # Only replace if filtering left at least one candidate; otherwise
            # keep the unfiltered list as a safety net (shouldn't happen with
            # data-derived slots, but guards against over-aggressive probes).
            # 仅在过滤后还有候选时替换;否则保留原表作为安全网。
            if filtered:
                self._pos_tables[key] = filtered

    def _greedy_encode_chain(self, chain_shape, prev_mvs, suffix_text, suffix_target):
        """
        Fast path: iterate total encoded length ascending, DFS within each
        budget to find the first (= lex-smallest) valid encoding.
        快速路径:按总编码长度升序枚举,每个长度内做 DFS 找首个(= 字典序最小)
        有效编码。

        Encoding cost / 编码花销:
          Each letter contributes 1 char if bare, 2 if FVS. Wraps add
          1 char per nirugu. The DFS sums these along the way and prunes
          when the running cost exceeds the budget.
          每个字母裸=1 字符,带 FVS=2 字符;每个 nirugu 包裹+1 字符。
          DFS 沿途累加并按预算剪枝。

        Per-slot candidate order: by (cp asc, fvs_cp asc with None first).
        Combined with budget-ascending outer loop, the first valid encoding
        found is the canonical under rule A.
        每槽候选按 (cp 升序, fvs_cp 升序 - None 在前) 排序。配合预算升序,
        DFS 找到的首个有效解即为规则 A 下的 canonical。

        Returns None if no valid encoding within any budget — caller falls
        back to exhaustive enumeration.
        若任何预算下都找不到 → 返回 None,调用方走穷举回退。
        """
        if not hasattr(self, '_pos_tables'):
            self._build_pos_tables()

        n_units = len(chain_shape)
        mvs_ch = chr(MVS_CP)
        nirugu_ch = chr(NIRUGU_CP)
        prefix_text = mvs_ch if prev_mvs else ''
        verify_target = (('mvs',) if prev_mvs else ()) + chain_shape + suffix_target

        # Encoding preference: no-wrap > nirugu-wrapped. Nirugu is a
        # joining marker; using it in normalize output is awkward when a
        # non-wrap encoding exists. So we exhaust all no-wrap budgets
        # first; only chains unreachable without nirugu (e.g. shape ['O']
        # which requires medi position) fall through to wrapped DFS.
        # 编码偏好:无 wrap > nirugu 包裹。nirugu 是连接标记,normalize 输出
        # 里出现它不直观。先穷尽所有无 wrap 预算,再考虑 nirugu 包裹。

        # 1) Greedy single-pass no-wrap.
        greedy_nw = self._greedy_one_shot_no_wrap(
            chain_shape, n_units, prev_mvs,
            prefix_text, suffix_text, verify_target,
        )
        if greedy_nw is not None and len(greedy_nw) <= n_units:
            return greedy_nw

        # 2) DFS no-wrap, budgets ascending. Stop at first valid.
        no_wrap_upper = (
            len(greedy_nw) if greedy_nw is not None else (2 * n_units + 2)
        )
        for total_chars in range(1, no_wrap_upper + 1):
            letters = []
            result = self._dfs_budget(
                chain_shape, n_units, 0, total_chars, letters,
                0, 0,  # wrap_pre, wrap_post
                prefix_text, suffix_text, verify_target, nirugu_ch,
            )
            if result is not None:
                return result
        if greedy_nw is not None:
            return greedy_nw  # equal or larger budgets already exhausted

        # 3) Wrapped DFS — chains unreachable without nirugu (rare).
        # 包含 nirugu 的 DFS —— 仅在无 wrap 编码不存在时使用。
        max_total = 2 * n_units + 2
        for total_chars in range(1, max_total + 1):
            for wrap_pre, wrap_post in ((0, 1), (1, 0), (1, 1)):
                wrap_cost = wrap_pre + wrap_post
                if wrap_cost >= total_chars:
                    continue
                letters_budget = total_chars - wrap_cost
                if letters_budget < 1:
                    continue
                letters = []
                result = self._dfs_budget(
                    chain_shape, n_units, 0, letters_budget, letters,
                    wrap_pre, wrap_post,
                    prefix_text, suffix_text, verify_target, nirugu_ch,
                )
                if result is not None:
                    return result
        return None

    def _greedy_one_shot_no_wrap(self, chain_shape, n_units, prev_mvs,
                                  prefix_text, suffix_text, verify_target):
        """Deterministic greedy without nirugu wraps. Returns text or None."""
        letters = []
        i = 0
        k = 0
        while i < n_units:
            picked = None
            for prefer_bare in (True, False):
                for L in range(n_units - i, 0, -1):
                    is_last_letter = (i + L == n_units)
                    if is_last_letter:
                        if k + 1 == 1:
                            pos = 'isol'
                        elif k == 0:
                            pos = 'init'
                        else:
                            pos = 'fina'
                    else:
                        pos = 'init' if k == 0 else 'medi'
                    written = tuple(chain_shape[i:i + L])
                    cands = self._pos_tables.get((pos, written), ())
                    if not cands:
                        continue
                    for cp, fvs_cp in cands:
                        is_bare = fvs_cp is None
                        if prefer_bare and not is_bare:
                            continue
                        if not prefer_bare and is_bare:
                            continue
                        picked = (L, cp, fvs_cp)
                        break
                    if picked is not None:
                        break
                if picked is not None:
                    break
            if picked is None:
                return None
            L, cp, fvs_cp = picked
            letters.append((cp, fvs_cp))
            i += L
            k += 1
        letters_text = "".join(
            chr(cp) if fvs_cp is None else (chr(cp) + chr(fvs_cp))
            for cp, fvs_cp in letters
        )
        if tuple(self.shape(prefix_text + letters_text + suffix_text)) == verify_target:
            return letters_text
        return None

    def _greedy_one_shot(self, chain_shape, n_units, prev_mvs,
                          prefix_text, suffix_text, verify_target, nirugu_ch):
        """
        Deterministic greedy: scan chain left-to-right, pick the best
        candidate per slot, verify the whole encoding once. No backtracking.
        Returns text on success or None if any slot has no candidate or
        the final shape check fails.
        左到右一遍贪婪,每槽取最佳候选,最后整体校验一次。失败返回 None。

        "Best" per slot is chosen with full canonical-rule precedence:
          1. shortest encoding (longest L bare wins over shorter L with FVS)
          2. among ties, lex-smallest (cp, fvs_cp)
        With pre-filtered `_pos_tables`, the top candidate is reliable for
        the common case.
        """
        # Try without nirugu wraps first (common case); only fall back to
        # wrapped variants when an unwrapped greedy can't even produce a
        # candidate (e.g. shape ['O'] needs medi position).
        for wrap_pre, wrap_post in self._NIRUGU_WRAPS:
            letters = []
            i = 0
            k = 0
            ok = True
            while i < n_units:
                # Per-slot ordering for canonical (rule A) at a single slot:
                #   1. Try bare candidates (cost=1) at longest L first.
                #   2. Only if no bare anywhere, try FVS candidates
                #      (cost=2) at longest L first.
                # Bare beats FVS even at smaller L because each bare slot
                # costs 1 char vs 2 for FVS. Among bare, longer L → fewer
                # total slots → fewer total chars.
                # 单槽规则 A:先按最长 L 找 bare,任何 L 找不到 bare 再退
                # 到最长 L 的 FVS。bare 比 FVS 短 1 字符,优先级更高。
                picked = None
                for prefer_bare in (True, False):
                    for L in range(n_units - i, 0, -1):
                        is_last_letter = (i + L == n_units)
                        chain_idx = wrap_pre + k
                        if is_last_letter:
                            eff_chain = k + 1 + wrap_pre + wrap_post
                            if eff_chain == 1:
                                pos = 'isol'
                            elif chain_idx == 0:
                                pos = 'init'
                            elif chain_idx == eff_chain - 1:
                                pos = 'fina'
                            else:
                                pos = 'medi'
                        else:
                            pos = 'init' if chain_idx == 0 else 'medi'
                        written = tuple(chain_shape[i:i + L])
                        cands = self._pos_tables.get((pos, written), ())
                        if not cands:
                            continue
                        for cp, fvs_cp in cands:
                            is_bare = fvs_cp is None
                            if prefer_bare and not is_bare:
                                continue
                            if not prefer_bare and is_bare:
                                continue
                            picked = (L, cp, fvs_cp)
                            break
                        if picked is not None:
                            break
                    if picked is not None:
                        break
                if picked is None:
                    ok = False
                    break
                L, cp, fvs_cp = picked
                letters.append((cp, fvs_cp))
                i += L
                k += 1
            if not ok:
                continue
            letters_text = "".join(
                chr(cp) if fvs_cp is None else (chr(cp) + chr(fvs_cp))
                for cp, fvs_cp in letters
            )
            text = nirugu_ch * wrap_pre + letters_text + nirugu_ch * wrap_post
            if tuple(self.shape(prefix_text + text + suffix_text)) == verify_target:
                return text
        return None

    def _dfs_budget(self, chain_shape, n_units, i, budget, letters,
                    wrap_pre, wrap_post,
                    prefix_text, suffix_text, verify_target, nirugu_ch):
        """DFS one slot; returns text on first valid encoding, else None."""
        if i == n_units:
            if budget != 0:
                return None
            letters_text = "".join(
                chr(cp) if fvs_cp is None else (chr(cp) + chr(fvs_cp))
                for cp, fvs_cp in letters
            )
            text = nirugu_ch * wrap_pre + letters_text + nirugu_ch * wrap_post
            if tuple(self.shape(prefix_text + text + suffix_text)) == verify_target:
                return text
            return None

        if budget < 1:
            return None

        k = len(letters)
        # Collect all (cost, L, cp, fvs_cp) options at this slot across
        # every possible L, then sort by (cp, fvs_cp) for lex-min iteration.
        # 汇总此槽所有 (cost, L, cp, fvs_cp) 选项,按 (cp, fvs_cp) 排序,
        # 字典序最小先试。
        slot_options = []
        for L in range(1, n_units - i + 1):
            is_last_letter = (i + L == n_units)
            chain_idx = wrap_pre + k
            if is_last_letter:
                eff_chain = k + 1 + wrap_pre + wrap_post
                if eff_chain == 1:
                    pos = 'isol'
                elif chain_idx == 0:
                    pos = 'init'
                elif chain_idx == eff_chain - 1:
                    pos = 'fina'
                else:
                    pos = 'medi'
            else:
                pos = 'init' if chain_idx == 0 else 'medi'

            written = tuple(chain_shape[i:i + L])
            cands = self._pos_tables.get((pos, written), ())
            for cp, fvs_cp in cands:
                cost = 1 if fvs_cp is None else 2
                if cost <= budget:
                    slot_options.append((cost, L, cp, fvs_cp))

        # Sort lex: cp asc, then bare (None) before FVS, then fvs_cp asc.
        # 字典序:cp 升序,裸优先,fvs_cp 升序。
        slot_options.sort(key=lambda o: (o[2], o[3] is not None, o[3] or 0))

        for cost, L, cp, fvs_cp in slot_options:
            letters.append((cp, fvs_cp))
            result = self._dfs_budget(
                chain_shape, n_units, i + L, budget - cost, letters,
                wrap_pre, wrap_post,
                prefix_text, suffix_text, verify_target, nirugu_ch,
            )
            if result is not None:
                return result
            letters.pop()
        return None

    def _compute_chain_canonical(self, chain_shape, prev_mvs=False,
                                  suffix_text="", suffix_target=()):
        """
        Canonical encoding for chain_shape. Tries fast greedy path first,
        falls back to exhaustive enumeration on greedy failure.
        canonical 编码:先走贪婪快路径,失败回退到穷举。
        """
        result = self._greedy_encode_chain(chain_shape, prev_mvs, suffix_text, suffix_target)
        if result is not None:
            return result
        return self._exhaustive_chain_canonical(chain_shape, prev_mvs, suffix_text, suffix_target)

    def _exhaustive_chain_canonical(self, chain_shape, prev_mvs=False,
                                     suffix_text="", suffix_target=()):
        """
        Exhaustive search fallback (slow). Used when greedy can't find an
        encoding. Same algorithm as before greedy was added.
        穷举回退(慢)。贪婪找不到时使用,算法同加入贪婪之前。

        Each "encoding" is a sequence of letters (with optional FVS each),
        optionally wrapped by nirugu before / after. Canonical = min by
        (total char length, lex).

        Search strategy / 搜索策略:
          Outer:  iterate n_letters from 1 upward.
                  从 1 开始递增枚举字母数。
          Middle: iterate nirugu wraps {(0,0),(0,1),(1,0),(1,1)}.
                  枚举 nirugu 包裹组合。
          Inner:  for each partition, iterate n_fvs_target (FVS count)
                  from 0 upward and stop as soon as length exceeds
                  best_len. With FVS count fixed, every combo has known
                  encoded length so we can length-prune without per-combo
                  sum().
                  按 FVS 数量递增枚举,长度即可一次性算出,无需逐组合 sum()。

        Why wraps / 为什么有 wraps:
          Some shape-units only appear at non-default positions. For
          example ['O'] can only be produced when a vowel is at medi —
          which a 1-letter chain doesn't have. Wrapping with nirugu
          extends the chain and shifts position: nirugu+o+nirugu → o at medi.
          某些 shape 单元只在非默认位置出现 (如 ['O'] 仅在元音处于 medi
          时才有)。nirugu 包裹延长 chain、移动位置。
        """
        if not hasattr(self, '_candidates_map'):
            self._build_candidates_map()
        if not hasattr(self, '_slot_candidates_cache'):
            self._slot_candidates_cache = {}

        from itertools import combinations as iter_combos, product as iter_product
        n_units = len(chain_shape)
        chain_list = list(chain_shape)
        nirugu_ch = chr(NIRUGU_CP)
        mvs_ch = chr(MVS_CP)
        # Verification context: prepend MVS if previous part is MVS; append
        # the caller-supplied suffix (which contains MVS + following chain
        # canonicals for cross-chain rule interactions like backward masc
        # propagation through MVS).
        # 校验上下文:前置 MVS(若前一段是 MVS);后置 suffix_text(由调用方
        # 提供 —— 含 MVS 及后续链的 canonical,以捕获跨 MVS 规则交互)。
        prefix_text = mvs_ch if prev_mvs else ''
        verify_target = (
            (('mvs',) if prev_mvs else ()) + chain_shape + suffix_target
        )

        best_len = None
        best_text = None

        for n_letters in range(1, n_units + 1):
            if best_len is not None and n_letters > best_len:
                break

            for wrap_pre, wrap_post in self._NIRUGU_WRAPS:
                min_total_no_fvs = n_letters + wrap_pre + wrap_post
                if best_len is not None and min_total_no_fvs > best_len:
                    continue
                eff_chain = n_letters + wrap_pre + wrap_post
                positions = []
                for i in range(n_letters):
                    chain_idx = wrap_pre + i
                    if eff_chain == 1:
                        positions.append('isol')
                    elif chain_idx == 0:
                        positions.append('init')
                    elif chain_idx == eff_chain - 1:
                        positions.append('fina')
                    else:
                        positions.append('medi')
                positions = tuple(positions)

                for partition in self._iter_chain_partitions(n_units, n_letters):
                    # Build slot candidates split into (bare, fvs) per slot.
                    # Each cached as a 2-tuple of cp-sorted tuples.
                    # 每个 slot 候选拆为 (bare, fvs) 两份,按 cp 排序后缓存。
                    slot_bare = []
                    slot_fvs = []
                    idx = 0
                    ok = True
                    for size, pos in zip(partition, positions):
                        sl = tuple(chain_list[idx:idx + size])
                        idx += size
                        key = (pos, sl)
                        cached = self._slot_candidates_cache.get(key)
                        if cached is None:
                            raw = self._candidates_map.get(key, [])
                            bare_set = set()
                            fvs_set = set()
                            for c in raw:
                                fvs_cp = FVS_INT_TO_CP.get(c['fvs'])
                                if fvs_cp is None:
                                    bare_set.add(c['cp'])
                                else:
                                    fvs_set.add((c['cp'], fvs_cp))
                            bare_tuple = tuple(
                                (cp, None) for cp in sorted(bare_set)
                            )
                            fvs_tuple = tuple(sorted(fvs_set))
                            cached = (bare_tuple, fvs_tuple)
                            self._slot_candidates_cache[key] = cached
                        if not cached[0] and not cached[1]:
                            ok = False
                            break
                        slot_bare.append(cached[0])
                        slot_fvs.append(cached[1])
                    if not ok:
                        continue

                    # Iterate by FVS count so each pass has fixed length L.
                    # 按 FVS 数量分层,每层长度固定,length-prune 无需 per-combo sum。
                    for n_fvs_target in range(0, n_letters + 1):
                        L = n_letters + n_fvs_target + wrap_pre + wrap_post
                        if best_len is not None and L > best_len:
                            break
                        for fvs_indices in iter_combos(range(n_letters), n_fvs_target):
                            fvs_idx_set = set(fvs_indices)
                            per_slot = []
                            slot_skip = False
                            for i in range(n_letters):
                                cands = slot_fvs[i] if i in fvs_idx_set else slot_bare[i]
                                if not cands:
                                    slot_skip = True
                                    break
                                per_slot.append(cands)
                            if slot_skip:
                                continue
                            for combo in iter_product(*per_slot):
                                letters_text = "".join(
                                    chr(cp) if fvs_cp is None else (chr(cp) + chr(fvs_cp))
                                    for cp, fvs_cp in combo
                                )
                                text = nirugu_ch * wrap_pre + letters_text + nirugu_ch * wrap_post
                                if best_len is not None and L == best_len and text >= best_text:
                                    continue
                                # Verify in MVS context — shape of
                                # (prefix MVS) + text + (suffix MVS)
                                # must equal the parent shape's
                                # corresponding window.
                                if tuple(self.shape(prefix_text + text + suffix_text)) == verify_target:
                                    if best_len is None or L < best_len or text < best_text:
                                        best_len = L
                                        best_text = text

        return best_text or ""

    def _iter_chain_partitions(self, n, k):
        """Yield all ordered compositions of n into exactly k positive parts."""
        if k == 1:
            yield (n,)
            return
        # Each first-part size leaves n-first for the remaining k-1 parts;
        # the smallest remaining sum is k-1, so first ranges 1..n-k+1.
        # 第一部分大小留 n-first 给后 k-1 部分;后部最小和为 k-1,故 first 取 1..n-k+1。
        for first in range(1, n - k + 2):
            for rest in self._iter_chain_partitions(n - first, k - 1):
                yield (first,) + rest

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
                       help="Input text (or `-` for stdin; omit to use --input)")
        p.add_argument("-i", "--input", metavar="FILE",
                       help="Read input from FILE (UTF-8)")
    p.add_argument("-o", "--output", metavar="FILE",
                   help="Write output to FILE (UTF-8); default stdout")
    p.add_argument("--batch", action="store_true",
                   help="Process input line by line, emit one result per line")


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
            "Examples / 示例:\n"
            "  mongol-norm normalize 'ᠰᠡᠢᠨ'\n"
            "  echo 'ᠰᠡᠢᠨ' | mongol-norm normalize -\n"
            "  mongol-norm normalize -i words.txt -o canonical.txt\n"
            "  mongol-norm normalize --batch -i words.txt    # one word per line\n"
            "  mongol-norm shape 'ᠰᠠᠢᠨ'                       # → S+A+I+I+A\n"
            "  mongol-norm normalize-text 'Hello ᠰᠡᠢᠨ world'  # mixed script\n"
        ),
    )
    parser.add_argument("--locale", default="MNG",
                        help="Locale: MNG (default), TOD, SIB, MCH, etc.")
    sub = parser.add_subparsers(dest="cmd", required=True, metavar="CMD")

    p_shape = sub.add_parser(
        "shape",
        help="Return '+'-joined written-unit sequence",
        description="Shape input through the UTN57 pipeline; output written units joined by '+'.",
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
    )
    _add_io_args(p_norm)

    p_normt = sub.add_parser(
        "normalize-text",
        help="Normalize full text (multi-word, mixed script)",
        description="Segment input into Mongolian runs vs others; normalize Mongolian, pass through rest.",
    )
    _add_io_args(p_normt)

    p_same = sub.add_parser(
        "same",
        help="Check if TEXT1 and TEXT2 shape identically (exit 0 if yes)",
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
