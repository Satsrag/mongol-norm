#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
UTN57 Mongolian Shaping Engine (Hudum)

Implements the COMPLETE shaping process from:
  https://mongfontbuilder.pages.dev/hudum/
  UTN #57 v4 (Encoding and Shaping of the Mongolian Script)

5-step Mongolian-specific shaping phase:
  1. Chachlag   — MVS-triggered suffix forms for a/e
  2. Syllabic   — consonant/vowel context (onset/devsger/marked/masculine/feminine/dotless)
  3. Particle   — MVS particle dictionary lookup
  4. Devsger    — i vowel_devsger (double tooth after vowel)
  5. Post-bowed — vowel forms after bowed consonants (G, Gx, K, K2, B, P, F)

Data source: mongfontbuilder PyPI package (mongfontbuilder.data)
"""
import json
import os
from typing import List, Tuple, Optional, Dict, Set
import mongfontbuilder as _mfb

POSITIONS = ["isol", "init", "medi", "fina"]

FVS_CPS = {0x180B, 0x180C, 0x180D, 0x180F}
FVS_INT_TO_CP = {0: None, 1: 0x180B, 2: 0x180C, 3: 0x180D, 4: 0x180F}
FVS_CP_TO_INT = {v: k for k, v in FVS_INT_TO_CP.items() if v is not None}

MVS_CP = 0x180E
NIRUGU_CP = 0x180A
ZWJ_CP = 0x200D
ZWNJ_CP = 0x200C
NNBSP_CP = 0x202F

MONGOLIAN_BLOCK = range(0x1800, 0x18B0)

# Bowed written units (from UTN57 spec)
BOWED_UNITS = {"G", "Gx", "K", "K2", "B", "P", "F"}


def is_mongolian_letter(cp):
    """Is cp a Mongolian letter (not FVS/MVS/punct/digit)?"""
    if cp in FVS_CPS or cp == MVS_CP or cp == NIRUGU_CP:
        return False
    if cp in range(0x1800, 0x180A):  # punctuation
        return False
    if cp in range(0x1810, 0x181A):  # digits
        return False
    return cp in MONGOLIAN_BLOCK


# ── Token ───────────────────────────────────────────────────────

class Token:
    """A token in the shaping pipeline."""
    __slots__ = [
        'cp', 'fvs_cp', 'position', 'condition', 'written',
        'alias', 'is_mvs', 'is_letter', 'index',
    ]
    
    def __init__(self, cp, fvs_cp=None, index=0):
        self.cp = cp
        self.fvs_cp = fvs_cp
        self.position = "isol"
        self.condition = None
        self.written = None
        self.alias = ""
        self.is_mvs = (cp == MVS_CP)
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
    
    Usage:
        shaper = MongolianShaper(locale="MNG")
        shape = shaper.shape("ᠰᠠᠢᠨ")  # → ['S', 'A', 'I', 'I', 'A']
    """

    def __init__(self, locale="MNG"):
        self.locale = locale
        self._load_data()
        self._build_lookups()

    # ── Data loading ────────────────────────────────────────────

    def _load_data(self):
        data_dir = os.path.join(os.path.dirname(_mfb.__file__), "data")
        def load(name):
            with open(os.path.join(data_dir, name), encoding="utf-8") as f:
                return json.load(f)
        self.variants = load("variants.json")
        self.aliases = load("aliases.json")
        self.locales_data = load("locales.json")
        self.particles_data = load("particles.json")
    
    def _build_lookups(self):
        # Character name ↔ codepoint
        self.cp_to_name = {}
        self.name_to_cp = {}
        for char_name in self.variants:
            cp = self._get_cp(char_name)
            if cp is not None:
                self.cp_to_name[cp] = char_name
                self.name_to_cp[char_name] = cp
        
        # Alias: cp → alias string for this locale
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
        # Default lookup: (cp, pos) → (fvs_int, vdata)
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
        
        # Locale categories
        locale_info = self.locales_data.get(self.locale, {})
        cats = locale_info.get("categories", {})
        self.vowels = set(cats.get("vowel", []))
        self.consonants = set(cats.get("consonant", []))
        self.masculine_vowels = set(cats.get("vowelMasculine", []))
        self.feminine_vowels = set(cats.get("vowelFeminine", []))
        self.neuter_vowels = set(cats.get("vowelNeuter", []))
        
        # Particle dictionary: sequence_str → list of particle_indices
        self.particle_dict = self.particles_data.get(self.locale, {})
    
    def _get_cp(self, name):
        try:
            import unicodedata
            return ord(unicodedata.lookup(name))
        except (KeyError, ValueError):
            return None
    
    # ── Written resolution ──────────────────────────────────────
    
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
        """Find the FVS int for a given condition at (cp, pos)."""
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
    
    # ── Tokenization ────────────────────────────────────────────
    
    def tokenize(self, text):
        """Split text into Token list."""
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
            elif cp == MVS_CP:
                tok = Token(cp, index=idx)
                tok.alias = "mvs"
                tokens.append(tok)
                idx += 1
                i += 1
            else:
                i += 1
        return tokens
    
    def assign_positions(self, tokens):
        """Assign isol/init/medi/fina to letter tokens."""
        ltoks = [t for t in tokens if t.is_letter]
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
        """Resolve a single token's written (lazy)."""
        if tok.written is not None:
            return
        if tok.is_mvs:
            tok.written = ()
            return
        
        fvs_int = FVS_CP_TO_INT.get(tok.fvs_cp, 0) if tok.fvs_cp else 0
        
        # Condition-mapped FVS (only if no explicit FVS)
        if tok.condition and not tok.fvs_cp:
            cond_fvs = self._get_condition_fvs(tok.cp, tok.position, tok.condition)
            if cond_fvs is not None:
                fvs_int = cond_fvs
        
        # If bare (no FVS, no condition hit) → use default variant
        if fvs_int == 0 and not tok.fvs_cp:
            dflt = self.default_variant.get((tok.cp, tok.position))
            if dflt:
                fvs_int = dflt[0]
        
        written = self._get_written(tok.cp, tok.position, fvs_int)
        tok.written = written if written else ()
    
    def _get_word_aliases(self, tokens):
        """Get alias sequence for the word (for particle lookup)."""
        return [t.alias for t in tokens if t.is_letter or t.is_mvs]
    
    # ── Shaping Steps ───────────────────────────────────────────
    
    def _step1_chachlag(self, tokens):
        """Step 1: Chachlag — a/e after MVS."""
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
        """Step 2: Syllabic — complex consonant/vowel context rules."""
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
            if alias in ("n", "t", "d"):
                if nxt and self._is_vowel(nxt):
                    tok.condition = "onset"
                    continue
                if prev and self._is_vowel(prev):
                    tok.condition = "devsger"
                    continue
            
            # --- h, g: masculine/feminine ---
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
                # Remote vowel harmony scan
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
        """Remote vowel harmony scan for h/g."""
        # Scan backwards for vowels
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
        """Step 3: Particle — MVS particle dictionary lookup."""
        # Build the alias sequence for particle matching
        aliases = []
        tok_indices = []
        for i, tok in enumerate(tokens):
            if tok.is_letter or tok.is_mvs:
                aliases.append(tok.alias)
                tok_indices.append(i)
        
        # Check if this word matches a particle pattern
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
        """Step 4: Devsger — i after vowel gets vowel_devsger."""
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
        """Step 5: Post-bowed — vowels after bowed written units."""
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
    
    # ── Main pipeline ───────────────────────────────────────────
    
    def shape(self, text):
        """
        Full shaping pipeline: text → glyph sequence (written units).
        
        Returns: list of written unit strings, e.g. ['S', 'A', 'I', 'I', 'A']
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
        
        # Collect written units
        result = []
        for tok in tokens:
            if tok.written:
                result.extend(tok.written)
        return result
    
    def shape_str(self, text):
        return "+".join(self.shape(text))
    
    def same_shape(self, text1, text2):
        return self.shape(text1) == self.shape(text2)
    
    def shape_detailed(self, text):
        """Return detailed shaping breakdown per token."""
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
    
    # ── Reverse shaping (unshape) ───────────────────────────────
    
    def build_reverse_map(self):
        """
        Build reverse lookup: (position, written_tuple) → canonical (cp, fvs_int)
        
        For normalization: given a shape sequence, find the canonical Unicode encoding.
        Prefers: default=True, non-archaic, non-unrecommended, lowest codepoint.
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
        
        Priority order:
        1. Unambiguous vowels: o/u → masculine, oe/ue/ee → feminine
        2. Condition of h/g letters (set during syllabic step):
           - feminine condition → feminine word
           - masculine_onset / masculine_devsger → masculine word
           This is the most reliable indicator when a/e ambiguity exists.
        3. Default to masculine (conventional choice)
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
    
    # Harmony-aware letter pairs
    HARMONY_PAIRS = {
        # (masculine_alias, feminine_alias)
        ("a", "e"),    # same medi/fina written
        ("h", "g"),    # QA/GA — same written in many positions
    }
    
    def _pick_by_harmony(self, candidates, harmony, original_alias, pos=None, written=None):
        """
        Pick the best candidate considering vowel harmony.
        
        Strategy:
          1. If original letter is among candidates → prefer it (minimal change)
          2. For a/e pair: masculine→a, feminine→e
          3. For h/g pair: masculine→h, feminine→g  
          4. For other ambiguities: keep original if candidate, else pick default
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
        Human-readable normalization using MINIMAL ENCODING principle.
        
        Key insight: bare Unicode (no FVS) is the canonical encoding.
        The shaping engine automatically selects the correct default variant.
        We only need to:
          1. Select the CORRECT LETTER (resolving ambiguity via vowel harmony)
          2. Output bare Unicode (no FVS, unless non-default variant needed)
        
        Algorithm:
          1. Shape input → get written units per token
          2. Detect vowel harmony from unambiguous vowels
          3. Merge adjacent identical written units (I+I → single I@medi+vowel_devsger)
          4. For each merged token, pick canonical letter:
             - Original letter preserved if it produces the same written (minimal change)
             - a/e pair → harmony-resolved (a for masculine, e for feminine)  
             - h/g pair → harmony-resolved (h for masculine, g for feminine)
             - Other ambiguous cases → original letter preserved
          5. Output bare Unicode (no FVS for default variants)
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
        
        # Detect vowel harmony
        harmony = self._detect_vowel_harmony(tokens)
        
        # Build segments with original alias info
        segments = []  # (type, written, original_alias)
        for tok in tokens:
            if tok.is_mvs:
                segments.append(('mvs', (), ''))
            elif tok.is_letter and tok.written:
                segments.append(('letter', tok.written, tok.alias))
        
        # Merge identical adjacent single-unit letter segments
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
        
        # Assign positions and pick canonical letters
        letter_segs = [(idx, s) for idx, s in enumerate(segments) if s[0] == 'letter']
        n_letters = len(letter_segs)
        
        result = []
        letter_seq = 0
        
        for idx, seg in enumerate(segments):
            tp = seg[0]
            if tp == 'mvs':
                result.append(chr(MVS_CP))
                continue
            
            written = seg[1]
            orig_alias = seg[2]
            
            if n_letters == 1: pos = "isol"
            elif letter_seq == 0: pos = "init"
            elif letter_seq == n_letters - 1: pos = "fina"
            else: pos = "medi"
            letter_seq += 1
            
            # Get all candidates for this (pos, written)
            candidates = self._get_candidates(pos, written)
            
            # Pick best candidate using harmony + original preservation
            best = self._pick_by_harmony(candidates, harmony, orig_alias, pos=pos, written=written)
            
            if best:
                # BARE ENCODING: output only the letter, no FVS
                # The shaping engine will automatically pick the correct variant
                # based on context (vowel_devsger, onset, devsger, etc.)
                result.append(chr(best["cp"]))
            else:
                # Absolute fallback: preserve original token  
                result.append(f"<{'|'.join(written)}>")
        
        return "".join(result)


# ── CLI ─────────────────────────────────────────────────────────

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


if __name__ == "__main__":
    main()
