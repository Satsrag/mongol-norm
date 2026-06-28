#!/usr/bin/env python3
"""
Generate the normalize table JSON consumed by mongol-norm and other-language ports.

mongol-norm's normalize() turns any encoding of a word into one canonical
Unicode string (same shape -> same output). Its primary path is a per-(position,
written-unit) FVS-pinned table: for each shaping position (isol/init/medi/fina)
and each written unit (the glyph form), exactly one (letter, fvs) that renders
that unit regardless of neighbours. Selecting those encodings requires a shaping
engine (the context-independence "battery"), which lives in mongol-norm. This
script drives that selection and serializes it to JSON so:

  * the Python runtime loads it instead of recomputing on every startup, and
  * ports in other languages can implement normalize with only a JSON parser
    plus the small partition algorithm documented in the mongol-norm README.

Run it from the mongol-norm package after a code change that affects shaping or
the selection battery, and commit the regenerated JSON:

    pip install -e .                                 # or have mongol_norm importable
    python scripts/gen_normalize_table.py            # all supported locales
    python scripts/gen_normalize_table.py MNG        # specific locales

Output: mongol_norm/data/<LOCALE>.normalize.json  (commit it).
"""
import argparse
import json
import sys
from pathlib import Path

# Make `mongol_norm` importable when run as `python scripts/gen_normalize_table.py`
# from the package root, without requiring an install.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

# Locales with a normalization implementation in mongol-norm. Extend as more
# scripts gain normalize support.
LOCALES = ["MNG"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[1])
    parser.add_argument(
        "locales",
        nargs="*",
        help=f"Locales to generate (default: all). Supported: {', '.join(LOCALES)}.",
    )
    parser.add_argument(
        "-o", "--output-dir",
        type=Path,
        default=Path(__file__).resolve().parent.parent / "mongol_norm" / "data",
        help="Output directory (default: ../mongol_norm/data/).",
    )
    args = parser.parse_args()

    try:
        from mongol_norm.shaper import MongolianShaper
    except ImportError:
        parser.error(
            "cannot import mongol_norm. Run from the mongol-norm package "
            "(or `pip install -e .` first)."
        )

    target_locales = args.locales or list(LOCALES)
    for loc in target_locales:
        if loc not in LOCALES:
            parser.error(f"unknown locale {loc!r}. Supported: {', '.join(LOCALES)}")

    args.output_dir.mkdir(parents=True, exist_ok=True)

    for locale in target_locales:
        shaper = MongolianShaper(locale=locale)
        spec = shaper.compute_normalize_tables()
        out_path = args.output_dir / f"{locale}.normalize.json"
        with out_path.open("w", encoding="utf-8") as f:
            json.dump(spec, f, ensure_ascii=False, indent=2)
            f.write("\n")
        n_units = sum(len(v) for v in spec["unit_table"].values())
        n_fem = sum(len(v) for v in spec["velar_fem"].values())
        print(
            f"Wrote {out_path} "
            f"({n_units} unit entries, {n_fem} velar-fem, "
            f"max_len {spec['unit_enc_max_len']})"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
