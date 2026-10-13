#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Glyph-name classification for the bundled unified shaping font (`fonts/unified.otf`).

mongfontbuilder names every glyph it composes after what it draws, so the glyph order of the
font is the whole glyph → written-unit mapping:

  uXXXX[_uYYYY...].UNITS.pos[.mvs | ._pos]   a letter variant or a ligature: the code points that
                                             drew it, the written units it shows, the position of
                                             the written form (`isol|init|medi|fina`), optionally
                                             the joining position it serves (`._isol`) or the
                                             before-MVS variant (`.mvs`)
  u1880 … u1886, u18A9[.fvs1]                the Ali Gali signs: spacing/combining marks the
                                             composer leaves under their nominal name
  mvs, mvs.narrow, nnbsp, nirugu, zwj, …     the format controls and their shaping-time variants
  _A.isol, I4, marker.initial, …             the composer's building blocks and markers: never
                                             part of a shaped run

Shared by `gen_font_shaper.py` (the glyph table in `src/generated/font_unified.rs`) and
`gen_rust_tables.py` (the font's units join the `WrittenUnit` enum). Python 3.9+, standard
library only.
"""
import re
from collections import namedtuple

# The parts of a letter glyph name. The units segment is parsed with UNIT_TOKEN and must be
# tiled by it exactly (`AOI` -> A, O, I), the same reading mongfontbuilder's own tests use.
LETTER_GLYPH = re.compile(
    r"^(?P<cps>u[0-9A-F]{4,6}(?:_u[0-9A-F]{4,6})*)"
    r"\.(?P<units>[A-Za-z0-9]+)"
    r"\.(?P<form>isol|init|medi|fina)"
    r"(?:\.(?:mvs|_(?P<joining>isol|init|medi|fina)))?$"
)
UNIT_TOKEN = re.compile(r"[A-Z][a-z0-9]*")

# The Ali Gali signs, by nominal glyph name. The composer draws them as marks and does not
# rename them, so their written unit is assigned here, after the names mongfontbuilder's test
# harness gives the six it knows (`Visarga` and `Dagalga` follow the same scheme).
MARK_UNITS = {
    "u1880": "Anusvara",
    "u1881": "Visarga",
    "u1882": "Damaru",
    "u1883": "Ubadama",
    "u1884": "Invubadama",
    "u1885": "Baluda",
    "u1886": "Tribaluda",
    "u18A9": "Dagalga",
}

# Unit tokens of the glyph names that the crate knows under another name: mongfontbuilder's
# data calls the nirugu stroke `Ni` — the glyph of U+180A and the stem some letters are
# drawn with (Sibe e: `ANi.init`) alike — and the crate's unit for that ink is `Nirugu`.
UNIT_RENAMES = {"Ni": "Nirugu"}

# Format controls. An MVS that separates nothing is drawn as `nbspace` + `mvs.ignored`, so the
# unit rides on the space and the second glyph stays silent; `nirugu.extend` is the stem the
# composer inserts after a bowed letter, not a nirugu of the text; an FVS is never ink the
# crate reports (valid or not); U+25CC is the dotted circle HarfBuzz puts under a lone mark.
MVS_GLYPHS = frozenset(["mvs", "mvs.narrow", "mvs.wide", "mvs.nominal", "nbspace", "nnbsp"])
NIRUGU_GLYPHS = frozenset(["nirugu", "nirugu.ignored"])
ZWJ_GLYPHS = frozenset(["zwj", "zwj.ignored"])
SILENT_GLYPHS = frozenset(["mvs.ignored", "nirugu.extend", "zwnj", "zwnj.ignored", "u25CC"])
SILENT_PREFIXES = ("fvs1", "fvs2", "fvs3", "fvs4", "marker.")

KINDS = ("letters", "mvs", "nirugu", "zwj", "silent", "unsupported")

# kind: one of KINDS; units: tuple of unit names; letters: code points the glyph draws;
# form / joining: positions (None unless kind == "letters" and the glyph is a letter variant).
Glyph = namedtuple("Glyph", "kind units letters form joining")


def classify(name):
    """The `Glyph` record of glyph `name` (raises ValueError on a malformed letter name)."""
    if name in MVS_GLYPHS:
        return Glyph("mvs", ("Mvs",), 0, None, None)
    if name in NIRUGU_GLYPHS:
        return Glyph("nirugu", ("Nirugu",), 0, None, None)
    if name in ZWJ_GLYPHS:
        return Glyph("zwj", ("Zwj",), 0, None, None)
    if name in SILENT_GLYPHS or name.startswith(SILENT_PREFIXES):
        return Glyph("silent", (), 0, None, None)
    base = name.split(".")[0]
    if base in MARK_UNITS and name in (base, base + ".fvs1"):
        return Glyph("letters", (MARK_UNITS[base],), 1, None, None)
    match = LETTER_GLYPH.match(name)
    if match is None:
        # `.notdef`, `space`, the vertical punctuation, a letter the font only has the nominal
        # glyph of (U+1878, U+1898), the `_X.pos` building blocks, …
        return Glyph("unsupported", (), 0, None, None)
    units = tuple(UNIT_TOKEN.findall(match.group("units")))
    if "".join(units) != match.group("units"):
        raise ValueError("glyph {!r}: units segment is not a sequence of units".format(name))
    units = tuple(UNIT_RENAMES.get(unit, unit) for unit in units)
    letters = match.group("cps").count("u")
    form = match.group("form")
    joining = match.group("joining") or form
    return Glyph("letters", units, letters, form, joining)


def read_glyph_order(path):
    """The glyph names of `fonts/unified.glyphs.txt`, in glyph-id order."""
    names = path.read_text(encoding="utf-8").split("\n")
    if names and names[-1] == "":
        names.pop()
    if not names or any(not name or name != name.strip() for name in names):
        raise ValueError("{}: expected one glyph name per line".format(path))
    return names


def font_units(names):
    """Every written unit the font can show (the structural `Mvs`/`Nirugu`/`Zwj` included)."""
    units = set()
    for name in names:
        units.update(classify(name).units)
    return units
