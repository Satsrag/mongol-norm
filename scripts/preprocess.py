#!/usr/bin/env python3
"""
Generate flat, language-agnostic shape-rules JSON from mongfontbuilder data.

Run once per mongfontbuilder upgrade. Commit the output into
`mongol_norm/data/<LOCALE>.json` so downstream consumers (the Python runtime,
JS/Dart/Java ports) don't need mongfontbuilder installed.

Usage:
    python preprocess.py                 # generate all supported locales
    python preprocess.py MNG TOD         # generate specific locales
"""
import argparse
import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional

from importlib.metadata import PackageNotFoundError, version as _pkg_version

import mongfontbuilder
from fontTools import unicodedata as fu


def _mfb_version() -> str:
    try:
        return _pkg_version("mongfontbuilder")
    except PackageNotFoundError:
        return "unknown"

SCHEMA_VERSION = 1
LOCALES = ("MNG", "TOD", "SIB", "MCH")
POSITIONS = ("isol", "init", "medi", "fina")


def _load_raw() -> Dict[str, Any]:
    base = Path(mongfontbuilder.__file__).parent / "data"
    out: Dict[str, Any] = {}
    for name in ("variants", "aliases", "locales", "particles"):
        with (base / f"{name}.json").open(encoding="utf-8") as f:
            out[name] = json.load(f)
    return out


def _resolve_written(
    written: Any,
    char_name: str,
    locale: str,
    variants: Dict[str, Any],
    depth: int = 0,
) -> Optional[List[str]]:
    """
    Resolve a `written` field to a concrete list of written-unit names.

    `written` may be either a plain list of unit names (e.g. ["A", "B"]) or a
    VariantReference ([position, fvs, locale?]) that points at another variant
    whose `written` should be reused. References may chain.
    """
    if depth > 8 or written is None or not isinstance(written, list):
        return None

    if (
        len(written) >= 2
        and isinstance(written[0], str)
        and written[0] in POSITIONS
        and isinstance(written[1], int)
    ):
        ref_pos = written[0]
        ref_fvs = str(written[1])
        ref_locale = written[2] if len(written) > 2 and written[2] else locale
        pd = variants.get(char_name, {}).get(ref_pos, {}).get(ref_fvs, {})
        src = pd.get("locales", {}).get(ref_locale, {}).get("written") or pd.get("written")
        return _resolve_written(src, char_name, ref_locale, variants, depth + 1)

    return [str(x) for x in written]


def _resolve_positioned_written(
    written: Any,
    char_name: str,
    locale: str,
    variants: Dict[str, Any],
    position: str,
    depth: int = 0,
) -> Optional[List[Dict[str, str]]]:
    """Resolve written units while preserving the referenced unit position."""
    if depth > 8 or written is None or not isinstance(written, list):
        return None

    if (
        len(written) >= 2
        and isinstance(written[0], str)
        and written[0] in POSITIONS
        and isinstance(written[1], int)
    ):
        ref_pos = written[0]
        ref_fvs = str(written[1])
        ref_locale = written[2] if len(written) > 2 and written[2] else locale
        pd = variants.get(char_name, {}).get(ref_pos, {}).get(ref_fvs, {})
        src = pd.get("locales", {}).get(ref_locale, {}).get("written") or pd.get("written")
        return _resolve_positioned_written(
            src, char_name, ref_locale, variants, ref_pos, depth + 1
        )

    total = len(written)
    records: List[Dict[str, str]] = []
    for index, unit in enumerate(written):
        if total == 1:
            unit_position = position
        elif index == 0 and position in ("isol", "init"):
            unit_position = "init"
        elif index == total - 1 and position in ("isol", "fina"):
            unit_position = "fina"
        else:
            unit_position = "medi"
        records.append({"unit": str(unit), "position": unit_position})
    return records


def _resolve_alias(alias_data: Any, locale: str) -> Optional[str]:
    if isinstance(alias_data, str):
        return alias_data
    if isinstance(alias_data, dict):
        ns = locale[:3] if len(locale) > 3 else locale
        return alias_data.get(locale) or alias_data.get(ns)
    return None


def _char_cp(char_name: str) -> Optional[int]:
    try:
        return ord(fu.lookup(char_name))
    except (KeyError, ValueError):
        return None


def build_rules(locale: str, raw: Dict[str, Any]) -> Dict[str, Any]:
    variants = raw["variants"]
    aliases = raw["aliases"]
    locales = raw["locales"]
    particles = raw["particles"]

    letters: List[Dict[str, Any]] = []
    for char_name, pos_data in variants.items():
        cp = _char_cp(char_name)
        if cp is None:
            continue

        letter_variants: List[Dict[str, Any]] = []
        for pos in POSITIONS:
            if pos not in pos_data:
                continue
            for fvs_str, vdata in pos_data[pos].items():
                locale_block = vdata.get("locales", {})
                if locale not in locale_block:
                    continue
                locale_data = locale_block[locale]
                w_raw = locale_data.get("written") or vdata.get("written")
                written = _resolve_written(w_raw, char_name, locale, variants)
                positioned_written = (
                    _resolve_positioned_written(
                        w_raw, char_name, locale, variants, pos
                    )
                    if locale == "MNG"
                    else None
                )
                if (
                    written is None
                    or (locale == "MNG" and positioned_written is None)
                ):
                    continue
                variant = {
                    "position": pos,
                    "fvs": int(fvs_str),
                    "written": written,
                }
                if positioned_written is not None:
                    variant["positioned_written"] = positioned_written
                variant.update({
                    "default": bool(vdata.get("default", False)),
                    "conditions": list(locale_data.get("conditions", [])),
                    "archaic": bool(locale_data.get("archaic", False)),
                    "unrecommended": bool(locale_data.get("unrecommended", False)),
                })
                letter_variants.append(variant)

        if not letter_variants:
            continue

        entry: Dict[str, Any] = {"cp": cp, "name": char_name}
        alias = _resolve_alias(aliases.get(char_name), locale)
        if alias:
            entry["alias"] = alias
        entry["variants"] = letter_variants
        letters.append(entry)

    letters.sort(key=lambda x: x["cp"])

    cats = locales.get(locale, {}).get("categories", {})
    categories = {
        key: list(cats.get(key, []))
        for key in ("vowel", "consonant", "vowelMasculine", "vowelFeminine", "vowelNeuter")
    }

    return {
        "schema_version": SCHEMA_VERSION,
        "locale": locale,
        "generated_from": {
            "package": "mongfontbuilder",
            "version": _mfb_version(),
        },
        "letters": letters,
        "categories": categories,
        "particles": particles.get(locale, {}),
    }


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

    target_locales = args.locales or list(LOCALES)
    for loc in target_locales:
        if loc not in LOCALES:
            parser.error(f"unknown locale {loc!r}. Supported: {', '.join(LOCALES)}")

    raw = _load_raw()
    args.output_dir.mkdir(parents=True, exist_ok=True)

    for locale in target_locales:
        rules = build_rules(locale, raw)
        out_path = args.output_dir / f"{locale}.json"
        with out_path.open("w", encoding="utf-8") as f:
            json.dump(rules, f, ensure_ascii=False, indent=2)
        print(
            f"Wrote {out_path} "
            f"({len(rules['letters'])} letters, "
            f"{len(rules['particles'])} particles)"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
