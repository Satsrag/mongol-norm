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
from typing import Callable, FrozenSet, List

BOWED_UNITS = {"G", "Gx", "K", "K2", "B", "P", "F"}

FVS2_CP = 0x180C
FVS4_CP = 0x180F


class Rule:
    """A single shaping rule = a single mongfontbuilder Lookup.
    (Plain class, not a dataclass, to keep the package Python 3.7+.)"""
    __slots__ = ("name", "locales", "apply")

    def __init__(self, name: str, locales: FrozenSet[str],
                 apply: Callable) -> None:
        self.name = name
        self.locales = locales
        self.apply = apply  # (tokens, shaper) -> None, mutates tokens in place


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
#     GB.A resets this when an FVS is adjacent to the vowel;
#     GB.B re-asserts it for the narrow h/g + FVS2/FVS4 + oe/ue.fina case.
# (1') oe/ue in medial position also get `marked` when the preceding medi
#      consonant sits in a cluster that starts with an init consonant.
# (2)  Initial d before a final vowel gets `marked` (Twelve Syllabaries form).
#      The init gate is load-bearing, not descriptive — see the rule body
#      for why we diverge from the formal UTN57 wording here. The GB
#      exception (FVS adjacent to d or the following vowel) is inlined in
#      `_iii2a_d_marked_at`.

def _iii2a_o_u_oe_ue_marked(tokens, shaper):
    """mongfontbuilder Lookup: III.o_u_oe_ue.marked"""
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
    prev = shaper._prev_letter(tokens, i)
    if prev is None:
        return
    if not (shaper._is_consonant(prev) and prev.position == "init"):
        return
    tok.condition = "marked"


def _iii2a_o_u_oe_ue_marked_gb_a(tokens, shaper):
    """mongfontbuilder Lookup: III.o_u_oe_ue.marked.GB.A

    Reverts `marked` to default when an FVS sits directly before or after
    the vowel. In mongfontbuilder this is expressed via two sub-rules under
    `UseMarkFilteringSet = fvs`:

        sub [vowel→reset] [fvs]                  # FVS attached to vowel
        sub [fvs] [vowel→reset]                  # FVS attached to prev letter
    """
    for i in range(len(tokens)):
        _iii2a_o_u_oe_ue_marked_gb_a_at(tokens, i, shaper)


def _iii2a_o_u_oe_ue_marked_gb_a_at(tokens, i, shaper):
    tok = tokens[i]
    if not tok.is_letter:
        return
    if tok.alias not in ("o", "u", "oe", "ue"):
        return
    if tok.condition != "marked":
        return
    # GB.A reset is observed only on .FINA-position vowels (verified
    # against DraftNew-Regular.otf via hb-shape):
    #   `h fvs3 oe`   (oe.fina) → reset to default "U"
    #   `h fvs3 oe l` (oe.medi) → STAYS marked "OI"
    #   `d fvs1 ue`   (ue.fina) → reset to default "U"
    #   `d fvs1 ue l` (ue.medi) → STAYS marked "OI"
    # iii.py's GB.A pattern uses `c.variants("MNG", ..., (medi, fina))`
    # which by the time iii2a.MAIN has substituted the input glyph to
    # the marked variant, no longer matches at medi position (the marked
    # medi glyph isn't in the class). Net effect: only fina vowels get
    # reset. We mirror that with an explicit position gate.
    if tok.position != "fina":
        return
    if shaper._has_fvs(tok):
        tok.condition = None
        return
    prev = shaper._prev_letter(tokens, i)
    if prev is not None and shaper._has_fvs(prev):
        tok.condition = None


def _iii2a_o_u_oe_ue_marked_gb_b(tokens, shaper):
    """mongfontbuilder Lookup: III.o_u_oe_ue.marked.GB.B

    The single exception to GB.A: initial h/g carrying FVS2 or FVS4
    followed by final oe/ue stays `marked`, even though an FVS sits
    between the consonant and the vowel.
    """
    for i in range(len(tokens)):
        _iii2a_o_u_oe_ue_marked_gb_b_at(tokens, i, shaper)


def _iii2a_o_u_oe_ue_marked_gb_b_at(tokens, i, shaper):
    tok = tokens[i]
    if tok.condition is not None:
        return
    if not tok.is_letter:
        return
    if tok.alias not in ("oe", "ue") or tok.position != "fina":
        return
    prev = shaper._prev_letter(tokens, i)
    if prev is None:
        return
    if prev.alias not in ("h", "g") or prev.position != "init":
        return
    if prev.fvs_cp not in (FVS2_CP, FVS4_CP):
        return
    tok.condition = "marked"


def _iii2a_oe_ue_cluster_marked(tokens, shaper):
    for i in range(len(tokens)):
        _iii2a_oe_ue_cluster_marked_at(tokens, i, shaper)


def _iii2a_oe_ue_cluster_marked_at(tokens, i, shaper):
    """iii2a.GB.A/B/C combined — propagate `marked` through a leading
    consonant cluster of any length to the following o/u/oe/ue.medi.

    iii.py uses three rclt lookups that cascade left-to-right:
      A) consonant.medi after [consonant.init OR _.marked] → _.marked
      B) o/u/oe/ue.medi after [consonant.init OR _.marked] → marked
      C) clean up the temporary _.marked from consonants
    We collapse them into a single backward walk: if the vowel sits at
    the end of an unbroken init→medi consonant chain (no vowel between),
    mark it. Verified against DraftNew-Regular.otf:
      `g ue l`         → ue stays default (iii2a.MAIN handles single init)
      `g g ue l`       → ue marked (this rule fires)
      `g g g g ue l`   → ue marked
      `b a g ue l`     → ue stays default (vowel `a` breaks the chain)
    """
    tok = tokens[i]
    if tok.condition is not None:
        return
    if not tok.is_letter:
        return
    if tok.alias not in ("o", "u", "oe", "ue"):
        return
    if tok.position != "medi":
        return
    if shaper._has_fvs(tok):
        return
    # Walk back through consonants (nirugu transparent). Need ≥1
    # consonant.medi then a consonant.init for this rule to fire (the
    # single-init case is handled by iii2a.MAIN, not here).
    j = i - 1
    saw_medi = False
    while j >= 0:
        t = tokens[j]
        if not t.is_letter:
            if t.is_nirugu:
                j -= 1
                continue
            return  # mvs/etc. blocks
        if not shaper._is_consonant(t):
            return  # vowel blocks the chain
        if t.position == "init":
            if saw_medi:
                tok.condition = "marked"
            return
        if t.position != "medi":
            return  # fina / isol — not a propagation source
        saw_medi = True
        j -= 1


def _iii2a_d_marked(tokens, shaper):
    for i in range(len(tokens)):
        _iii2a_d_marked_at(tokens, i, shaper)


def _iii2a_d_marked_at(tokens, i, shaper):
    tok = tokens[i]
    if tok.condition is not None:
        return
    if not tok.is_letter or tok.alias != "d":
        return
    # Restrict to d.init. The formal UTN57 wording in
    # `mongfontbuilder/web/docs/hudum.mdx` says just "d", but both
    # reference implementations gate on init:
    #   - mongfontbuilder/lib/mongfontbuilder/otl/iii.py:359-364
    #     uses `MNG-d.init` as the input class
    #   - mongolian/sources/otl/lookups-general-syllabic.fea:72
    #     uses `@d-hud.init'` as the matched glyph
    # The data layer agrees: `condition.hud.marked`
    # (mongolian/sources/otl/lookups-conditions-hag.fea:33-41) only
    # ships a marked variant for d.init (`uni1833.D.init`); d.medi has
    # no marked form. Without this gate, a d.medi sitting before a fina
    # vowel would silently claim the `marked` slot and — via the
    # first-writer-wins guard at the top — block iii2e from setting
    # `onset` on the same d, dropping the correct Twelve-Syllabaries
    # onset form. So we mirror the implementations, not the prose.
    if tok.position != "init":
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
        # GB variant: data exposes this as a distinct condition.
        tok.condition = "chachlag_onset_gb"


# ═══════════════════════════════════════════════════════════════════════
# Phase III.2e — Phonetic: n/t/d onset vs devsger
#   mongfontbuilder iii.py::iii2e
# ═══════════════════════════════════════════════════════════════════════
#
# For n/t/d: `onset` before a vowel, `devsger` after a vowel.
#
# Carve-out: `t + ee` is owned by iii2g.t.devsger, NOT iii2e. In iii.py
# (`mongfontbuilder/lib/mongfontbuilder/otl/iii.py:738-753`), iii2g uses
# OpenType's class-membership trick to overwrite iii2e's onset substitution
# (the condition lookup matches the post-iii2e glyph because `MNG-t` class
# includes all variants of t). Our first-writer-wins model can't replicate
# that, so iii2e must bow out for the `t + ee` case to let iii2g fire.
# Verified against `DraftNew-Regular.otf` via hb-shape:
#   `a t ee n` → u1832.T.medi (devsger), NOT u1832.D.medi (default/onset)
# Note this carve-out is t-only (not n or d): d before ee still goes onset
# via iii2e (and font confirms `a d ee` renders default D).

def _iii2e_n_t_d_onset_devsger(tokens, shaper):
    for i in range(len(tokens)):
        _iii2e_n_t_d_onset_devsger_at(tokens, i, shaper)


def _iii2e_n_t_d_onset_devsger_at(tokens, i, shaper):
    tok = tokens[i]
    if tok.condition is not None:
        return
    if not tok.is_letter or tok.alias not in ("n", "t", "d"):
        return
    # iii.py iii2e uses `IgnoreMarks: True` (iii.py:516), so the
    # onset/devsger context match fires regardless of attached FVS.
    # The condition still gets set; if the user's FVS happens to
    # name a variant on this letter, the resolver picks the FVS form
    # (matching iii.py's later FVS-substitution stage), otherwise the
    # condition's variant fires — e.g. `ue n fvs3 ue` → fvs3 unknown
    # on n.medi, onset condition picks fvs1=N.
    nxt = shaper._next_letter(tokens, i)
    if nxt is not None and shaper._is_vowel(nxt):
        # iii2g.t.devsger owns the `t + ee` case (see block comment above).
        if not (tok.alias == "t" and nxt.alias == "ee"):
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
# For h/g, condition assignment proceeds in three layers, mirroring iii.py:
#
#  (a) Adjacent vowel — `III.k_g_h.onset_and_devsger_and_gender.MNG_TOD_SIB_MCH`
#      (iii.py:592-683). The IMMEDIATELY adjacent vowel decides:
#          next masc      → masculine_onset
#          next fem/neut  → feminine
#          prev masc      → masculine_devsger
#          prev fem       → feminine
#
#  (b) `i + g/h` with a propagated MASC marker —
#      `III.g_h.onset_and_devsger_and_gender.A.MNG` (iii.py:686-707).
#      Only fires when prev is `i`. preprocessing.A/B/C (iii.py:78-101)
#      duplicates a MASC marker after every non-fem letter downstream of
#      a masc vowel, then strips MASC after every non-h/g letter. Net
#      result: MASC sits immediately after every h/g letter that is
#      preceded by a masc vowel with no fem vowel in between (regardless
#      of what follows the h/g). When that happens, pattern 5 fires →
#      `masculine_devsger`. Otherwise pattern 6 (i + g, no MASC) fires →
#      `feminine` — but only for `g`, not `h`. See
#      `_masc_marker_reaches_g_h` in shaper.py for the precise check.
#
#  (c) `g.init/h.init + consonant → feminine` —
#      `III.g_h.onset_and_devsger_and_gender.B.MNG` (iii.py:709-718).
#
# ─── Doc/implementation discrepancy ────────────────────────────────────
# `mongfontbuilder/web/docs/hudum.mdx:189-220` describes FOUR additional
# "remotely follows / remotely precedes" rules (e.g. "h/g remotely follows
# masc vowel without a blocking fem → masculine_devsger"). Neither
# mongfontbuilder iii.py nor mongolian/.fea actually implements those —
# verified by font rendering: `o l g` renders G (default), NOT H, even
# though o is a masc vowel earlier in the word. Layer (b) above is the
# ONLY non-adjacent harmony mechanism in the implementation, and it is
# strictly gated on prev=i.
# We follow the implementations (which match the font), not the prose.
# A previous version of this rule used `_scan_vowel_harmony` to do an
# unconstrained backward+forward scan; that over-applied masculine_devsger
# whenever any masc vowel existed in the word and was the source of the
# `o l g → H` regression. The new helper `_masc_marker_reaches_g_h`
# encodes the precise marker-propagation semantics from preprocessing.A/B.
#
# When none of (a)/(b)/(c) fire, we leave `tok.condition = None` and let
# the resolver fall back to the data's default variant. For g.medi/g.fina
# and h.medi the default glyph happens to coincide with the feminine glyph
# (`G` for g, `G` for h.medi); for h.fina the default is `H` (no feminine
# variant — falls back to default anyway).

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
    # Adjacency-strict prev/nxt: iii.py's iii2f main rules run with
    # `IgnoreMarks: True` (FVS marks invisible) but MVS is a base glyph
    # that BREAKS adjacency. Skipping past MVS via `_prev/_next_letter`
    # would let us false-match vowels across word boundaries (e.g.
    # `b a d a g mvs u n` would otherwise see u as g.fina's "next masc
    # vowel" and fire masculine_onset, but iii.py keeps them apart so
    # g.fina sees only its adjacent prev=a and gets masculine_devsger).
    nxt = shaper._next_adjacent_letter(tokens, i)
    prev = shaper._prev_adjacent_letter(tokens, i)

    # ── (a) Adjacent vowel decides ──
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

    # ── (b) i + g/h with reachable MASC marker (iii2f.A) ──
    if prev is not None and prev.alias == "i":
        if shaper._masc_marker_reaches_g_h(tokens, i):
            tok.condition = "masculine_devsger"
            return
        if tok.alias == "g":  # iii2f.A pattern 6: g only, not h
            tok.condition = "feminine"
            return
        # h with prev=i and no reachable marker: no rule fires here.

    # ── (c) g.init/h.init + consonant (iii2f.B) ──
    if tok.position == "init" and nxt is not None and shaper._is_consonant(nxt):
        tok.condition = "feminine"
        return

    # No layer fired — leave condition = None so the resolver uses the
    # data's default variant (matches .fea/iii.py behaviour exactly).


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
    """mongfontbuilder Lookup: III.t_sh_g.MNG.GB (g branch, iii.py:764-776)

    Two precise sub-rules:
      (1) `s/d + g.medi + masc vowel` → `dotless`
      (2) `s/d + g.fina + MVS + (chachlag a.isol)` → `dotless`

    Unlike the other rules in this file, this lookup OVERRIDES any
    condition already set on `g`. In iii.py / mongolian/.fea the same
    effect comes from OpenType's class-membership semantics — iii2g's
    pattern matches the g.medi/g.fina input even after iii2f or iii2c
    already substituted it, and its `condition.hud.dotless` lookup
    rewrites the glyph. Our first-writer-wins model can't reproduce that
    organically, so we explicitly drop the `if tok.condition is not None`
    guard and write `dotless` over whatever earlier rules set.

    Concretely:
      - iii2f.h_g.harmony tags `s + g + masc` as `masculine_onset` (Hx);
        rule (1) here OVERRIDES that with `dotless` (H).
      - iii2c.chachlag_onset tags `s + g + mvs + isol a` as
        `chachlag_onset` (Hx); rule (2) here OVERRIDES that with
        `dotless` (H).
      - `s + g + mvs + isol e` is already H via `chachlag_onset_gb`
        (different glyph slot, same final shape), so no override needed.

    All expected outputs verified against `DraftNew-Regular.otf` via
    `hb-shape --unicodes=...`.
    """
    tok = tokens[i]
    # NOTE: no `if tok.condition is not None: return` — see docstring.
    if not tok.is_letter or tok.alias != "g":
        return
    if shaper._has_fvs(tok):
        return
    prev = shaper._prev_letter(tokens, i)
    if prev is None or prev.alias not in ("s", "d"):
        return

    nxt = shaper._next_letter(tokens, i)
    nxt_tok = shaper._next_tok(tokens, i)

    # Rule (1): g.medi + immediately following masc vowel
    if tok.position == "medi" and nxt is not None and shaper._is_masc_vowel(nxt):
        tok.condition = "dotless"
        return

    # Rule (2): g.fina + MVS + chachlag a.isol
    # The chachlag check encodes iii.py's literal `"u1820.Aa.isol"` context
    # (the glyph name AFTER iii1.chachlag has fired). If a has FVS that
    # cancelled chachlag, the glyph would be default a.isol and iii.py's
    # rule wouldn't match — we mirror that by requiring chachlag here.
    if tok.position == "fina" and nxt_tok is not None and nxt_tok.is_mvs:
        nxt_after_mvs = shaper._next_letter(tokens, i + 1)
        if (nxt_after_mvs is not None
                and nxt_after_mvs.alias == "a"
                and nxt_after_mvs.position == "isol"
                and nxt_after_mvs.condition == "chachlag"):
            tok.condition = "dotless"
            return


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
        # Try the segment as-is first, then (for mvs-headed segments)
        # retry with mvs stripped — iii.py's particle dict also contains
        # patterns like `u u` / `ue ue` / `b ue ue` that match anywhere,
        # not just word-initial; the GSUB lookup fires regardless of
        # whether mvs precedes (per iii.py line 785: "Apply `particle`
        # for letters in particles not following MVS in Hudum").
        particle_indices = shaper.particle_dict.get(" ".join(aliases))
        used_indices = indices
        if particle_indices is None and aliases and aliases[0] == "mvs":
            particle_indices = shaper.particle_dict.get(" ".join(aliases[1:]))
            used_indices = indices[1:]
        if particle_indices is None:
            continue  # loop filter — no particle match for this segment
        # iii.py iii3 uses `UseMarkFilteringSet @fvs` (iii.py:795), so
        # FVS marks are VISIBLE during the contextual match. A particle
        # dict entry like `mvs i y a r` requires the 5 base glyphs to be
        # adjacent; any FVS attached to a letter in the segment breaks
        # that adjacency and the lookup doesn't fire. Mirror by skipping
        # this segment if any letter carries an FVS.
        # Verified against DraftNew-Regular.otf:
        #   `mvs i y a r`       → particle fires (i=I, y=I)
        #   `mvs i y fvs1 a r`  → particle does NOT fire (i=AI, y=I default)
        if any(tokens[idx].is_letter and tokens[idx].fvs_cp is not None
               for idx in indices):
            continue
        # iii3's lookup keys on position-specific classes (e.g.
        # `MNG-y.init`, `MNG-i.fina`). If nirugu sits between mvs and the
        # first chain letter, that letter's joining position shifts off
        # `init` (single-letter chains: `isol` → `fina`; multi-letter:
        # `init` → `medi`), so the lookup's first-letter class no longer
        # matches and particle doesn't fire. Verified against
        # DraftNew-Regular.otf:
        #   `mvs y i`               → particle fires (y=I)
        #   `mvs nirugu y i`        → does NOT fire (y stays default Y)
        #   `mvs i nirugu y a r`    → particle fires (i=I, y=I)
        #   `mvs i`                 → particle fires (i.isol → I)
        first_letter_idx = next(
            (idx for idx in indices if tokens[idx].is_letter), None)
        if (first_letter_idx is not None
                and tokens[first_letter_idx].position not in ("init", "isol")):
            continue
        for pidx in particle_indices:
            if pidx >= len(used_indices):
                continue  # defensive; never expected to fire
            tok = tokens[used_indices[pidx]]
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
#
# Like iii2g.t.devsger and iii2g.g.dotless, this rule OVERRIDES any
# condition set by earlier phases (notably iii3.particle and
# iii2a.marked) when its precise context fires. In iii.py the same
# effect comes from OpenType's class-membership semantics: iii5 matches
# `bowed letter + vowel.fina` regardless of any earlier substitution
# and re-substitutes the vowel via `condition.MNG:post_bowed`. We mirror
# this by dropping the `if tok.condition is not None: return` guard.
#
# Concrete example fixed by the override (see `core-hud.tsv: particle-15`):
#   `mvs h ue` — h.init+ue.fina, iii2f gives h the feminine form "G"
#   which is bowedG. iii3 particle dict `mvs h ue` → [2] would set ue's
#   condition to `particle` (→ "U"). iii5 then OVERRIDES with
#   `post_bowed` (→ "O"), matching DraftNew font output.

def _iii5_post_bowed(tokens, shaper):
    for i in range(len(tokens)):
        _iii5_post_bowed_at(tokens, i, shaper)


def _iii5_post_bowed_at(tokens, i, shaper):
    tok = tokens[i]
    # iii5 overrides most prior conditions (notably iii3.particle), but
    # explicitly defers when iii2a has set `marked` — iii.py's iii5 has a
    # leading ignore pattern for `u1825.Ue.fina` / `u1826.Ue.fina`
    # (`iii.py:966`), which are exactly the post-marked glyphs of oe/ue.
    if tok.condition == "marked":
        return
    if not tok.is_letter:
        return
    # NOTE: no `if shaper._has_fvs(tok): return` — iii.py iii5 uses
    # IgnoreMarks, so FVS on the vowel doesn't block the rule. When FVS
    # matches a variant the resolver's FVS-first priority wins; when it
    # doesn't (e.g. `b a fvs3`), the resolver falls through to the
    # condition and applies post_bowed correctly.
    if tok.alias not in ("o", "u", "oe", "ue", "a", "e"):
        return
    if tok.position != "fina":
        return
    prev = shaper._prev_letter(tokens, i)
    if prev is None:
        return
    shaper._resolve_token_written(prev)
    if not prev.written:
        return

    last_unit = prev.written[-1]
    is_bowedG = last_unit in ("G", "Gx")
    is_bowedBK = last_unit in ("B", "P", "F", "K", "K2")
    if not (is_bowedG or is_bowedBK):
        return

    # iii.py iii5 main rule input sets (iii.py:967-984):
    #   bowed (any)            + [o,u,oe,ue].fina → post_bowed
    #   bowedB + bowedK        + [a,e].fina       → post_bowed
    #   bowedG                 + e.fina           → post_bowed (a EXCLUDED)
    # So bowedG + a does NOT fire post_bowed. Verified against
    # `DraftNew-Regular.otf`: `h fvs2 a → G A` (default, not Aa).
    if is_bowedG and tok.alias == "a":
        return

    # ── iii5.GB rules (FVS-aware, iii.py:993-1036) ──
    # Verified against DraftNew-Regular.otf via hb-shape.
    if prev.fvs_cp is not None:
        FVS2 = 0x180C
        FVS4 = 0x180F
        # Rule E: g/h.(init|medi) + fvs2/fvs4 + [o,u].fina → reset
        # (suppress post_bowed). Example: `h fvs2 o → "U"` (default),
        # NOT `"O"` (post_bowed).
        if (is_bowedG and prev.alias in ("g", "h")
                and prev.fvs_cp in (FVS2, FVS4)
                and tok.alias in ("o", "u")):
            return
        # Rule C: bowedB/bowedK.init + fvs + [oe,ue].fina → MARKED
        # (not post_bowed). Example: `b fvs1 ue → "Ue"` (marked),
        # NOT `"O"` (post_bowed).
        if (is_bowedBK and prev.position == "init"
                and tok.alias in ("oe", "ue")):
            tok.condition = "marked"
            return
        # Note: iii.py's other GB rules (B, D for h/g+fvs1/3 → reset;
        # G for h/g.init+fvs2/4+oe/ue → marked) are already covered by
        # our existing pipeline. B/D: h/g+fvs1/3 renders as Hx/H (not
        # bowed) → iii5 main wouldn't fire anyway, and iii2a.GB.A
        # has already reset any marked condition. G: handled by
        # iii2a.GB.B which we implement directly.

    tok.condition = "post_bowed"


# ═══════════════════════════════════════════════════════════════════════
# Locale rule tables
# ═══════════════════════════════════════════════════════════════════════

RULES_MNG: List[Rule] = [
    Rule("III.1.chachlag",                 frozenset({"MNG"}), _iii1_chachlag),
    Rule("III.2a.o_u_oe_ue.marked",        frozenset({"MNG"}), _iii2a_o_u_oe_ue_marked),
    Rule("III.2a.o_u_oe_ue.marked.GB.A",   frozenset({"MNG"}), _iii2a_o_u_oe_ue_marked_gb_a),
    Rule("III.2a.o_u_oe_ue.marked.GB.B",   frozenset({"MNG"}), _iii2a_o_u_oe_ue_marked_gb_b),
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
    base = locale[:-1] if locale.endswith("x") else locale
    return {
        "MNG": RULES_MNG,
        "TOD": RULES_TOD,
        "SIB": RULES_SIB,
        "MCH": RULES_MCH,
    }.get(base, [])
