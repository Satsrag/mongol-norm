#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
The half of `gen_font_shaper.py` that runs inside a mongfontbuilder checkout's environment.

    uv run python <this file> SOURCE_OTF OUT_DIR

It needs fontTools, which that environment has and this repository's tooling does not —
`gen_font_shaper.py` runs it with `uv run` from the checkout. It writes into OUT_DIR:

  hudum.otf          SOURCE_OTF with everything that is not shaping removed: the outlines (CFF),
                     the metrics and names, GPOS and the debugging table go; GSUB, GDEF, cmap and
                     the sfnt skeleton stay. Advances are zeroed and `post` is cut to format 3
                     (no glyph names; `hudum.glyphs.txt` has them).
  hudum.glyphs.txt   the glyph order, one name per line

and prints a JSON object with the checksums and counts the pin file records.
"""
import hashlib
import json
import sys
from importlib.metadata import version
from pathlib import Path

from fontTools.ttLib import TTFont

DROP_TABLES = ("CFF ", "Debg", "GPOS", "VORG", "name", "OS/2", "vhea", "vmtx")


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def strip(source, target):
    font = TTFont(source)
    order = font.getGlyphOrder()
    for tag in DROP_TABLES:
        if tag in font:
            del font[tag]
    font["post"].formatType = 3.0
    font["post"].extraNames = []
    font["post"].mapping = {}
    # hhea/hmtx only have to be well-formed: nothing reads an advance.
    font["hmtx"].metrics = {name: (0, 0) for name in order}
    font.save(target)
    return order


def main():
    source = Path(sys.argv[1])
    out = Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)

    order = strip(source, out / "hudum.otf")
    (out / "hudum.glyphs.txt").write_text("\n".join(order) + "\n", encoding="utf-8")

    print(json.dumps({
        "mongfontbuilder_version": version("mongfontbuilder"),
        "source_sha256": sha256(source),
        "source_bytes": source.stat().st_size,
        "font_sha256": sha256(out / "hudum.otf"),
        "font_bytes": (out / "hudum.otf").stat().st_size,
        "glyphs": len(order),
    }))
    return 0


if __name__ == "__main__":
    sys.exit(main())
