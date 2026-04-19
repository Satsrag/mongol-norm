"""
Declarative shaping rules for Traditional Mongolian script.

This module mirrors the structure of mongfontbuilder's OpenType shaping
pipeline (`mongfontbuilder/lib/mongfontbuilder/otl/iii.py`). Each rule here
corresponds 1:1 to a single Lookup in iii.py — same name, same intent,
same order.

Why this shape
--------------
In mongfontbuilder / OpenType each rule is expressed as:

    sub  [left context]  [input + action]  [right context]  by ...

i.e. a pattern → action, with context checks on each side of the target.
Lookups fire in a fixed order and a later Lookup sees the state produced
by earlier ones.

This file translates that into Python:

    def _iii<N><letter>_<topic>(tokens, shaper):
        for i in range(len(tokens)):
            _iii<N><letter>_<topic>_at(tokens, i, shaper)

    def _iii<N><letter>_<topic>_at(tokens, i, shaper):
        tok = tokens[i]
        if <pattern does not hold>: return
        ...
        tok.condition = "<action>"

One rule = one function = one mongfontbuilder Lookup. No `continue` is used
to short-circuit between rules; each rule is independent and applied in
order by `run_rules()`.

Only rules actually implemented so far are included. Items still awaiting
port (gender propagation III.0b, Sibe-specific iii2b/iii2d, etc.) are
tracked as `TODO` entries in the RULES lists.
"""
from dataclasses import dataclass
from typing import Callable, FrozenSet, List

BOWED_UNITS = {"G", "Gx", "K", "K2", "B", "P", "F"}


@dataclass(frozen=True)
class Rule:
    """A single shaping rule = a single mongfontbuilder Lookup."""
    name: str
    locales: FrozenSet[str]
    apply: Callable  # (tokens, shaper) -> None, mutates tokens in place


def run_rules(rules: List[Rule], tokens, shaper) -> None:
    """Execute rules in declared order for the shaper's locale."""
    for rule in rules:
        if shaper.locale in rule.locales:
            rule.apply(tokens, shaper)


# ═══════════════════════════════════════════════════════════════════════
# Phase III.1 — Phonetic: Chachlag
#   mongfontbuilder iii.py::iii1
# ═══════════════════════════════════════════════════════════════════════
#
# The isolated Hudum a/e choose `chachlag` when they follow an MVS. Per GB,
# an FVS right after the chachlag vowel cancels the assignment so the MVS
# shaping gets postponed to the particle step.

def _iii1_chachlag(tokens, shaper):
    for i in range(len(tokens)):
        _iii1_chachlag_at(tokens, i, shaper)


def _iii1_chachlag_at(tokens, i, shaper):
    tok = tokens[i]
    if not tok.is_letter:
        return
    if tok.alias not in ("a", "e"):
        return
    prev = shaper._prev_tok(tokens, i)
    if prev is None or not prev.is_mvs:
        return
    if shaper._has_fvs(tok):
        return  # GB: FVS → postpone to particle step
    tok.condition = "chachlag"


# ═══════════════════════════════════════════════════════════════════════
# Phase III.2a — Phonetic: o/u/oe/ue `marked`, d `marked`
#   mongfontbuilder iii.py::iii2a
# ═══════════════════════════════════════════════════════════════════════
#
# (1) o/u/oe/ue following an initial consonant get `marked` (tall-stem form).
# (1') oe/ue in medial position also get `marked` when the preceding medi
#      consonant sits in a cluster that starts with an init consonant.
# (2)  Initial d before a final vowel gets `marked` (Twelve Syllabaries form).
# GB exception across the board: explicit FVS on the target reverts to default.

def _iii2a_o_u_oe_ue_marked(tokens, shaper):
    for i in range(len(tokens)):
        _iii2a_o_u_oe_ue_marked_at(tokens, i, shaper)


def _iii2a_o_u_oe_ue_marked_at(tokens, i, shaper):
    tok = tokens[i]
    if tok.condition is not None:
        return
    if not tok.is_letter:
        return
    if tok.alias not in ("o", "u", "oe", "ue"):
        return
    if shaper._has_fvs(tok):
        return  # GB: explicit FVS → default
    prev = shaper._prev_letter(tokens, i)
    if prev is None:
        return
    if not (shaper._is_consonant(prev) and prev.position == "init"):
        return
    tok.condition = "marked"


def _iii2a_oe_ue_cluster_marked(tokens, shaper):
    for i in range(len(tokens)):
        _iii2a_oe_ue_cluster_marked_at(tokens, i, shaper)


def _iii2a_oe_ue_cluster_marked_at(tokens, i, shaper):
    tok = tokens[i]
    if tok.condition is not None:
        return
    if not tok.is_letter:
        return
    if tok.alias not in ("oe", "ue"):
        return
    if tok.position != "medi":
        return
    if shaper._has_fvs(tok):
        return
    prev = shaper._prev_letter(tokens, i)
    if prev is None:
        return
    if not (shaper._is_consonant(prev) and prev.position == "medi"):
        return
    pp_idx = prev.index if hasattr(prev, "index") else i - 1
    pp = shaper._prev_letter(tokens, pp_idx)
    if pp is None:
        return
    if not (shaper._is_consonant(pp) and pp.position == "init"):
        return
    tok.condition = "marked"


def _iii2a_d_marked(tokens, shaper):
    for i in range(len(tokens)):
        _iii2a_d_marked_at(tokens, i, shaper)


def _iii2a_d_marked_at(tokens, i, shaper):
    tok = tokens[i]
    if tok.condition is not None:
        return
    if not tok.is_letter or tok.alias != "d":
        return
    if shaper._has_fvs(tok):
        return
    nxt = shaper._next_letter(tokens, i)
    if nxt is None:
        return
    if not (shaper._is_vowel(nxt) and nxt.position == "fina"):
        return
    if shaper._has_fvs(nxt):
        return  # GB: FVS on the following vowel also cancels
    tok.condition = "marked"


# ═══════════════════════════════════════════════════════════════════════
# Phase III.2c — Phonetic: chachlag_onset for n/j/w/h/g
#   mongfontbuilder iii.py::iii2c
# ═══════════════════════════════════════════════════════════════════════
#
# Hudum n/j/w immediately before an MVS followed by isolated a/e take
# `chachlag_onset`. Hudum h/g before an MVS followed by isolated a take
# `chachlag_onset`; g before MVS + isolated e also qualifies under GB.

def _iii2c_chachlag_onset(tokens, shaper):
    for i in range(len(tokens)):
        _iii2c_chachlag_onset_at(tokens, i, shaper)


def _iii2c_chachlag_onset_at(tokens, i, shaper):
    tok = tokens[i]
    if tok.condition is not None:
        return
    if not tok.is_letter:
        return
    if shaper._has_fvs(tok):
        return
    alias = tok.alias

    nxt_tok = shaper._next_tok(tokens, i)
    if nxt_tok is None or not nxt_tok.is_mvs:
        return
    nxt_after_mvs = shaper._next_letter(tokens, i + 1) if i + 1 < len(tokens) else None
    if nxt_after_mvs is None or nxt_after_mvs.position != "isol":
        return

    target_vowel = nxt_after_mvs.alias
    if alias in ("n", "j", "w") and target_vowel in ("a", "e"):
        tok.condition = "chachlag_onset"
        return
    if alias in ("h", "g") and target_vowel == "a":
        tok.condition = "chachlag_onset"
        return
    if alias == "g" and target_vowel == "e":
        # GB variant — same condition name in the data (chachlag_onset covers it).
        tok.condition = "chachlag_onset"


# ═══════════════════════════════════════════════════════════════════════
# Phase III.2e — Phonetic: n/t/d onset vs devsger
#   mongfontbuilder iii.py::iii2e
# ═══════════════════════════════════════════════════════════════════════
#
# For n/t/d: `onset` before a vowel, `devsger` after a vowel.

def _iii2e_n_t_d_onset_devsger(tokens, shaper):
    for i in range(len(tokens)):
        _iii2e_n_t_d_onset_devsger_at(tokens, i, shaper)


def _iii2e_n_t_d_onset_devsger_at(tokens, i, shaper):
    tok = tokens[i]
    if tok.condition is not None:
        return
    if not tok.is_letter or tok.alias not in ("n", "t", "d"):
        return
    if shaper._has_fvs(tok):
        return
    nxt = shaper._next_letter(tokens, i)
    if nxt is not None and shaper._is_vowel(nxt):
        tok.condition = "onset"
        return
    prev = shaper._prev_letter(tokens, i)
    if prev is not None and shaper._is_vowel(prev):
        tok.condition = "devsger"


# ═══════════════════════════════════════════════════════════════════════
# Phase III.2f — Phonetic: h/g harmony (masculine_onset / feminine / ...)
#   mongfontbuilder iii.py::iii2f
# ═══════════════════════════════════════════════════════════════════════
#
# For h/g: choose masculine_onset vs feminine based on neighboring vowel
# harmony. Adjacent vowels take priority; otherwise scan the word for any
# unambiguous vowel (_scan_vowel_harmony). Falls back to `feminine`.

def _iii2f_h_g_harmony(tokens, shaper):
    for i in range(len(tokens)):
        _iii2f_h_g_harmony_at(tokens, i, shaper)


def _iii2f_h_g_harmony_at(tokens, i, shaper):
    tok = tokens[i]
    if tok.condition is not None:
        return
    if not tok.is_letter or tok.alias not in ("h", "g"):
        return
    if shaper._has_fvs(tok):
        return
    nxt = shaper._next_letter(tokens, i)
    prev = shaper._prev_letter(tokens, i)

    if nxt is not None and shaper._is_masc_vowel(nxt):
        tok.condition = "masculine_onset"
        return
    if nxt is not None and (shaper._is_fem_vowel(nxt) or shaper._is_neut_vowel(nxt)):
        tok.condition = "feminine"
        return
    if prev is not None and shaper._is_masc_vowel(prev):
        tok.condition = "masculine_devsger"
        return
    if prev is not None and shaper._is_fem_vowel(prev):
        tok.condition = "feminine"
        return
    remote = shaper._scan_vowel_harmony(tokens, i)
    if remote is not None:
        tok.condition = remote
        return
    tok.condition = "feminine"  # conventional fallback


# ═══════════════════════════════════════════════════════════════════════
# Phase III.2g — Phonetic: t devsger / sh dotless / g dotless
#   mongfontbuilder iii.py::iii2g
# ═══════════════════════════════════════════════════════════════════════
#
# (1) t before ee or a consonant → devsger
# (2) sh before i → dotless (dot would collide with the following i)
# (3) g after s or d → dotless

def _iii2g_t_devsger(tokens, shaper):
    for i in range(len(tokens)):
        _iii2g_t_devsger_at(tokens, i, shaper)


def _iii2g_t_devsger_at(tokens, i, shaper):
    tok = tokens[i]
    if tok.condition is not None:
        return
    if not tok.is_letter or tok.alias != "t":
        return
    if shaper._has_fvs(tok):
        return
    nxt = shaper._next_letter(tokens, i)
    if nxt is None:
        return
    if nxt.alias == "ee" or shaper._is_consonant(nxt):
        tok.condition = "devsger"


def _iii2g_sh_dotless(tokens, shaper):
    for i in range(len(tokens)):
        _iii2g_sh_dotless_at(tokens, i, shaper)


def _iii2g_sh_dotless_at(tokens, i, shaper):
    tok = tokens[i]
    if tok.condition is not None:
        return
    if not tok.is_letter or tok.alias != "sh":
        return
    if shaper._has_fvs(tok):
        return
    nxt = shaper._next_letter(tokens, i)
    if nxt is None or nxt.alias != "i":
        return
    if tok.position == "init" and nxt.position == "medi":
        tok.condition = "dotless"
        return
    if tok.position == "medi":
        tok.condition = "dotless"


def _iii2g_g_dotless(tokens, shaper):
    for i in range(len(tokens)):
        _iii2g_g_dotless_at(tokens, i, shaper)


def _iii2g_g_dotless_at(tokens, i, shaper):
    tok = tokens[i]
    if tok.condition is not None:
        return
    if not tok.is_letter or tok.alias != "g":
        return
    if shaper._has_fvs(tok):
        return
    prev = shaper._prev_letter(tokens, i)
    if prev is None or prev.alias not in ("s", "d"):
        return
    tok.condition = "dotless"


# ═══════════════════════════════════════════════════════════════════════
# Phase III.3 — Phonetic: Particle
#   mongfontbuilder iii.py::iii3
# ═══════════════════════════════════════════════════════════════════════
#
# Match each MVS-headed segment against the particle dictionary. Tokens at
# the hit indices receive `particle` (overrides prior conditions for the
# specific aliases listed below — matches the subset of letters that the
# particle condition actually has a variant for in the data).

_PARTICLE_TARGET_ALIASES = ("a", "e", "i", "u", "ue", "d", "y")


def _iii3_particle(tokens, shaper):
    segments = _build_mvs_segments(tokens)
    for aliases, indices in segments:
        particle_indices = shaper.particle_dict.get(" ".join(aliases))
        if particle_indices is None:
            continue  # loop filter — no particle match for this segment
        for pidx in particle_indices:
            if pidx >= len(indices):
                continue  # defensive; never expected to fire
            tok = tokens[indices[pidx]]
            if tok.is_letter and tok.alias in _PARTICLE_TARGET_ALIASES:
                tok.condition = "particle"


def _build_mvs_segments(tokens):
    segments = []
    current_aliases: List[str] = []
    current_indices: List[int] = []
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
    return segments


# ═══════════════════════════════════════════════════════════════════════
# Phase III.4 — Graphemic: Devsger (i after vowel → vowel_devsger)
#   mongfontbuilder iii.py::iii4
# ═══════════════════════════════════════════════════════════════════════
#
# Medial i after a vowel whose shape does not already end in "I" becomes a
# double-tooth form (`vowel_devsger`).

def _iii4_vowel_devsger(tokens, shaper):
    for i in range(len(tokens)):
        _iii4_vowel_devsger_at(tokens, i, shaper)


def _iii4_vowel_devsger_at(tokens, i, shaper):
    tok = tokens[i]
    if tok.condition is not None:
        return
    if not tok.is_letter or tok.alias != "i" or tok.position != "medi":
        return
    if shaper._has_fvs(tok):
        return
    prev = shaper._prev_letter(tokens, i)
    if prev is None or not shaper._is_vowel(prev):
        return
    shaper._resolve_token_written(prev)
    if shaper._written_ends_with(prev, "I"):
        return  # already ends in I, no double tooth needed
    tok.condition = "vowel_devsger"


# ═══════════════════════════════════════════════════════════════════════
# Phase III.5 — Graphemic: Post-bowed
#   mongfontbuilder iii.py::iii5
# ═══════════════════════════════════════════════════════════════════════
#
# A vowel following a bowed consonant (G, Gx, K, K2, B, P, F) takes the
# `post_bowed` form to attach smoothly to the bow's rightward stroke.

def _iii5_post_bowed(tokens, shaper):
    for i in range(len(tokens)):
        _iii5_post_bowed_at(tokens, i, shaper)


def _iii5_post_bowed_at(tokens, i, shaper):
    tok = tokens[i]
    if tok.condition is not None:
        return
    if not tok.is_letter:
        return
    if shaper._has_fvs(tok):
        return
    if tok.alias not in ("o", "u", "oe", "ue", "a", "e"):
        return
    prev = shaper._prev_letter(tokens, i)
    if prev is None:
        return
    shaper._resolve_token_written(prev)
    if not prev.written:
        return
    if prev.written[-1] not in BOWED_UNITS:
        return
    tok.condition = "post_bowed"


# ═══════════════════════════════════════════════════════════════════════
# Locale rule tables
# ═══════════════════════════════════════════════════════════════════════

RULES_MNG: List[Rule] = [
    Rule("III.1.chachlag",                 frozenset({"MNG"}), _iii1_chachlag),
    Rule("III.2a.o_u_oe_ue.marked",        frozenset({"MNG"}), _iii2a_o_u_oe_ue_marked),
    Rule("III.2a.oe_ue.cluster.marked",    frozenset({"MNG"}), _iii2a_oe_ue_cluster_marked),
    Rule("III.2a.d.marked",                frozenset({"MNG"}), _iii2a_d_marked),
    Rule("III.2c.chachlag_onset",          frozenset({"MNG"}), _iii2c_chachlag_onset),
    Rule("III.2e.n_t_d.onset_devsger",     frozenset({"MNG"}), _iii2e_n_t_d_onset_devsger),
    Rule("III.2f.h_g.harmony",             frozenset({"MNG"}), _iii2f_h_g_harmony),
    Rule("III.2g.t.devsger",               frozenset({"MNG"}), _iii2g_t_devsger),
    Rule("III.2g.sh.dotless",              frozenset({"MNG"}), _iii2g_sh_dotless),
    Rule("III.2g.g.dotless",               frozenset({"MNG"}), _iii2g_g_dotless),
    Rule("III.3.particle",                 frozenset({"MNG"}), _iii3_particle),
    Rule("III.4.vowel_devsger",            frozenset({"MNG"}), _iii4_vowel_devsger),
    Rule("III.5.post_bowed",               frozenset({"MNG"}), _iii5_post_bowed),
    # TODO: III.0b (gender propagation, word-wide masculine/feminine marker)
    # TODO: III.2b (Sibe z before i marked; Manchu i/z; Manchu f; MAG)
    # TODO: III.2d (Sibe/Manchu feminine after t/d/k/g/h)
    # TODO: III.6 (FVS-selected `manual` — currently implicit in _resolve_token_written)
]

RULES_TOD: List[Rule] = []  # TODO: port Todo-specific rules
RULES_SIB: List[Rule] = []  # TODO: port Sibe-specific rules
RULES_MCH: List[Rule] = []  # TODO: port Manchu-specific rules


def get_rules_for_locale(locale: str) -> List[Rule]:
    base = locale.removesuffix("x") if locale.endswith("x") else locale
    return {
        "MNG": RULES_MNG,
        "TOD": RULES_TOD,
        "SIB": RULES_SIB,
        "MCH": RULES_MCH,
    }.get(base, [])
