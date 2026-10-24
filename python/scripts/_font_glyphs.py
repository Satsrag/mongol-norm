#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Glyph-name classification for the bundled shaping font (`fonts/hudum.otf`).

mongfontbuilder names every glyph it composes after what it draws, so the glyph order of the
font is the whole glyph → written-unit mapping:

  uXXXX[_uYYYY...].UNITS.pos[.mvs | ._pos]   a letter variant or a ligature: the code points that
                                             drew it, the written units it shows, the position of
                                             the written form (`isol|init|medi|fina`), optionally
                                             the joining position it serves (`._isol`) or the
                                             before-MVS variant (`.mvs`)
  mvs, mvs.narrow, nnbsp, nirugu, zwj, zwnj  the format controls and their shaping-time variants
  _A.isol, marker.initial, …                 the composer's building blocks and markers: never
                                             part of a shaped run

Used by `gen_font_shaper.py` (the glyph table in `src/generated/font_hudum.rs`). Python 3.9+,
standard library only.
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

# Format controls. An MVS that separates nothing is drawn as `nbspace` + `mvs.ignored`, so the
# unit rides on the space and the second glyph stays silent; an FVS is never ink the crate
# reports (valid or not); a composer marker is a shaping-time placeholder.
MVS_GLYPHS = frozenset(["mvs", "mvs.narrow", "mvs.wide", "mvs.nominal", "nbspace", "nnbsp"])
NIRUGU_GLYPHS = frozenset(["nirugu", "nirugu.ignored"])
ZWJ_GLYPHS = frozenset(["zwj", "zwj.ignored"])
ZWNJ_GLYPHS = frozenset(["zwnj", "zwnj.ignored"])
SILENT_GLYPHS = frozenset(["mvs.ignored"])
SILENT_PREFIXES = ("fvs1", "fvs2", "fvs3", "fvs4", "marker.")

KINDS = ("letters", "mvs", "nirugu", "zwj", "zwnj", "silent", "unsupported")

# kind: one of KINDS; units: tuple of unit names; letters: code points the glyph draws;
# form / joining: positions (None unless kind == "letters").
Glyph = namedtuple("Glyph", "kind units letters form joining")


def classify(name):
    """The `Glyph` record of glyph `name` (raises ValueError on a malformed letter name)."""
    if name in MVS_GLYPHS:
        return Glyph("mvs", ("Mvs",), 0, None, None)
    if name in NIRUGU_GLYPHS:
        return Glyph("nirugu", ("Nirugu",), 0, None, None)
    if name in ZWJ_GLYPHS:
        return Glyph("zwj", ("Zwj",), 0, None, None)
    if name in ZWNJ_GLYPHS:
        return Glyph("zwnj", ("Zwnj",), 0, None, None)
    if name in SILENT_GLYPHS or name.startswith(SILENT_PREFIXES):
        return Glyph("silent", (), 0, None, None)
    match = LETTER_GLYPH.match(name)
    if match is None:
        # `.notdef`, `space`, punctuation, the `_X.pos` building blocks, …
        return Glyph("unsupported", (), 0, None, None)
    units = tuple(UNIT_TOKEN.findall(match.group("units")))
    if "".join(units) != match.group("units"):
        raise ValueError("glyph {!r}: units segment is not a sequence of units".format(name))
    letters = match.group("cps").count("u")
    form = match.group("form")
    joining = match.group("joining") or form
    return Glyph("letters", units, letters, form, joining)


def read_glyph_order(path):
    """The glyph names of `fonts/hudum.glyphs.txt`, in glyph-id order."""
    names = path.read_text(encoding="utf-8").split("\n")
    if names and names[-1] == "":
        names.pop()
    if not names or any(not name or name != name.strip() for name in names):
        raise ValueError("{}: expected one glyph name per line".format(path))
    return names


def font_units(names):
    """Every written unit the font can show (the structural `Mvs`/`Nirugu`/`Zwj`/`Zwnj` included)."""
    units = set()
    for name in names:
        units.update(classify(name).units)
    return units
