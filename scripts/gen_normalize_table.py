#!/usr/bin/env python3
"""
Generate the normalize table JSON consumed by mongol-norm and other-language ports.

mongol-norm's normalize() turns any encoding of a word into one canonical
Unicode string (same shape -> same output). Its primary path is a per-(position,
written-unit) FVS-pinned table: for each shaping position (isol/init/medi/fina)
and each written unit (the glyph form), exactly one (letter, fvs) that renders
that unit regardless of neighbours.

Selecting those encodings — the context-independence "battery" — lives HERE, in
this build-time script, NOT in the runtime shaper. The runtime only loads the
JSON this script emits. Keeping the battery out of the shaper keeps the runtime
lean and makes the JSON the single source of truth for:

  * the Python runtime (mongol_norm loads it at startup), and
  * ports in other languages (only a JSON parser + the partition algorithm
    documented in the mongol-norm README are needed).

Run it from the mongol-norm package after a change that affects shaping or the
selection battery, and commit the regenerated JSON:

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

from mongol_norm.shaper import (  # noqa: E402
    MongolianShaper, CANONICAL_VERSION, FVS_INT_TO_CP,
    MVS_CP, NIRUGU_CP, ZWJ_CP,
)

# Locales with a normalization implementation in mongol-norm. Extend as more
# scripts gain normalize support.
LOCALES = ["MNG"]

# Context battery: neighbour letters used to probe context-independence.
# Cover masc vowel (a), fem vowel (e), masc round vowel (o), neutral (i),
# plain consonant (n), sibilant (s), velar (g), and a BOWED consonant (b) —
# without a true bowed neighbour the battery is blind to post_bowed effects
# (e.g. bare final a renders the swung Aa after b) and false-passes bare picks.
# 探针需含真弓形邻居(b),否则电池看不到 post_bowed 毒化,裸形会被误判安全。
CI_PROBE_LETTERS = (0x1820, 0x1821, 0x1823, 0x1822, 0x1828, 0x1830, 0x182D, 0x182A)


def _is_context_independent(shaper, position, written, cp, fvs_cp):
    """
    True iff letter (cp, fvs_cp) placed at `position` produces exactly `written`
    in EVERY probed neighbour context (and at least one probe lands it there).
    """
    letter = chr(cp) + (chr(fvs_cp) if fvs_cp is not None else '')
    target_written = list(written)
    landed = False
    left_options = [''] if position in ('isol', 'init') else [chr(p) for p in CI_PROBE_LETTERS]
    right_options = [''] if position in ('isol', 'fina') else [chr(p) for p in CI_PROBE_LETTERS]
    for left in left_options:
        for right in right_options:
            text = left + letter + right
            try:
                details = shaper.shape_detailed(text)
            except Exception:
                return False
            # target letter is the token right after the (0- or 1-letter) left context
            target_index = 1 if left else 0
            if target_index >= len(details):
                continue
            detail = details[target_index]
            if detail.get('position') != position:
                continue
            landed = True
            if list(detail.get('written') or []) != target_written:
                return False
    return landed


def compute_unit_tables(shaper):
    """
    Run the context-independence battery against `shaper` and return
    (unit_table, feminine_table, max_length). This is the SELECTION method —
    the single source of truth the exported JSON encodes.
    """
    if not hasattr(shaper, '_candidates_map'):
        shaper._build_candidates_map()
    table = {}
    for (position, written), candidates in shaper._candidates_map.items():
        # MASCULINE-first for vowels; context-independence is the gate. Sort
        # masc-first, FVS-FIRST, cp asc, fvs asc, keep the FIRST that passes.
        # FVS-first because an FVS exists precisely to pin a form against
        # context; bare letters are context-sensitive by design and only
        # acceptable when no FVS'd candidate renders the unit.
        # FVS 优先:FVS 的意义就是隔离 context;裸字母天生受上下文感染,
        # 只有没有可用 FVS 候选时才退而用裸形。
        ordered_candidates = sorted(
            ((candidate['cp'], FVS_INT_TO_CP.get(candidate['fvs']))
             for candidate in candidates),
            key=lambda pair: (
                shaper.cp_to_alias.get(pair[0]) in shaper.feminine_vowels,
                pair[0], pair[1] is None, pair[1] or 0,
            )
        )
        seen = set()
        for cp, fvs_cp in ordered_candidates:
            if (cp, fvs_cp) in seen:
                continue
            seen.add((cp, fvs_cp))
            if _is_context_independent(shaper, position, written, cp, fvs_cp):
                table[(position, written)] = (cp, fvs_cp)
                break
    # Hand-pins (battery should already pick these; insurance):
    #   'G' → g+fvs2 (the only all-position context-independent G);
    #   fina 'J' → j+fvs2 (absent from candidates_map but battery-proven).
    g_fvs2 = (0x182D, 0x180C)
    for position in ('isol', 'init', 'medi', 'fina'):
        if _is_context_independent(shaper, position, ('G',), *g_fvs2):
            table[(position, ('G',))] = g_fvs2
    j_fvs2 = (0x1835, 0x180C)
    if ('fina', ('J',)) not in table and _is_context_independent(shaper, 'fina', ('J',), *j_fvs2):
        table[('fina', ('J',))] = j_fvs2
    # User spelling rule: an isolated 'I' chain is written i+FVS1, never the
    # battery's bare-first pick j (both render the lone double-tooth I).
    # 用户拼写规则:孤立 'I' 写 i+FVS1,不用电池按裸形优先选出的 j。
    i_fvs1 = (0x1822, 0x180B)
    if _is_context_independent(shaper, 'isol', ('I',), *i_fvs1):
        table[('isol', ('I',))] = i_fvs1
    max_length = max((len(written) for (_, written) in table), default=1)

    # Feminine alternative table for the velar-fem refinement: for each single-
    # unit slot, a FEMININE (e/oe/ue) context-independent encoding producing the
    # same unit, if one exists (a naive cp-swap can change the unit).
    feminine_vowel_cps = {shaper.alias_to_cp[alias] for alias in shaper.feminine_vowels
                          if alias in shaper.alias_to_cp}
    feminine_table = {}
    for (position, written), candidates in shaper._candidates_map.items():
        if len(written) != 1:
            continue
        ordered_candidates = sorted(
            ((candidate['cp'], FVS_INT_TO_CP.get(candidate['fvs']))
             for candidate in candidates if candidate['cp'] in feminine_vowel_cps),
            key=lambda pair: (pair[0], pair[1] is None, pair[1] or 0)
        )
        seen = set()
        for cp, fvs_cp in ordered_candidates:
            if (cp, fvs_cp) in seen:
                continue
            seen.add((cp, fvs_cp))
            if _is_context_independent(shaper, position, written, cp, fvs_cp):
                feminine_table[(position, written)] = (cp, fvs_cp)
                break

    return table, feminine_table, max_length


def compute_normalize_tables(shaper):
    """
    Run the battery and return a JSON-serializable spec of the normalize tables
    — the artifact the runtime and other-language ports consume.
    """
    table, feminine_table, max_length = compute_unit_tables(shaper)

    def encode_entry(cp, fvs_cp):
        return {
            "letter": shaper.cp_to_alias.get(cp, ""),
            "cp": f"{cp:04X}",
            "fvs": (f"{fvs_cp:04X}" if fvs_cp is not None else None),
        }

    def group_by_position(mapping):
        grouped = {}
        for (position, written), (cp, fvs_cp) in mapping.items():
            grouped.setdefault(position, {})["+".join(written)] = encode_entry(cp, fvs_cp)
        # stable ordering → readable, diff-friendly file
        return {position: dict(sorted(grouped[position].items()))
                for position in ('isol', 'init', 'medi', 'fina') if position in grouped}

    return {
        "schema": "mongol-normalize-table/1",
        "canonical_version": CANONICAL_VERSION,
        "locale": shaper.locale,
        "description": (
            "Per-(position, written-unit) FVS-pinned encoding table for "
            "normalize. Each value is a (letter, fvs) that renders exactly "
            "the written unit at that position regardless of neighbours. "
            "See the mongol-norm README for the consuming algorithm."
        ),
        "unit_enc_max_len": max_length,
        "constants": {
            "MVS": f"{MVS_CP:04X}",
            "NIRUGU": f"{NIRUGU_CP:04X}",
            "ZWJ": f"{ZWJ_CP:04X}",
            "FVS1": "180B", "FVS2": "180C",
            "FVS3": "180D", "FVS4": "180F",
        },
        "ci_probe_letters": [shaper.cp_to_alias.get(c, f"{c:04X}") for c in CI_PROBE_LETTERS],
        "velar_fem_units": sorted(shaper._VELAR_FEM_UNITS),
        "masc_to_fem": {shaper.cp_to_alias.get(k): shaper.cp_to_alias.get(v)
                        for k, v in shaper._MASC_TO_FEM_CP.items()},
        "unit_table": group_by_position(table),
        "velar_fem": group_by_position(feminine_table),
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

    args.output_dir.mkdir(parents=True, exist_ok=True)

    for locale in target_locales:
        shaper = MongolianShaper(locale=locale)
        spec = compute_normalize_tables(shaper)
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
