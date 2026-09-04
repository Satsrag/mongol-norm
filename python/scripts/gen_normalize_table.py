#!/usr/bin/env python3
"""
Generate the normalize table JSON consumed by mongol-norm and other-language ports.

mongol-norm's normalize() turns any encoding of a word into one canonical
Unicode string (same shape -> same output). Its primary path is a per-(position,
written-unit) FVS-pinned table: for each shaping position (isol/init/medi/fina)
and each written unit (the glyph form), exactly one (letter, fvs) that renders
that unit regardless of neighbours.

Selecting those encodings — the context-independence "battery" — lives HERE, in
this build-time script, NOT in the runtime engine. The engine (the Rust crate at
the repository root, which the Python package wraps) compiles the table in from
the JSON this script emits (``python/scripts/gen_rust_tables.py``). Keeping the battery
out of the engine keeps the runtime lean and makes the JSON the single source of
truth for:

  * the Rust engine (``gen_rust_tables.py`` turns it into static tables), and
  * ports in other languages (only a JSON parser + the partition algorithm
    documented in the mongol-norm README are needed).

The battery uses only the public shaping API (``MongolianShaper.shape_detailed``);
the letter inventory and vowel categories come straight from the shaping-rules
JSON (``python/mongol_norm/data/<LOCALE>.json``, see docs/data-format.md).

Run it from the repository root after a change that affects shaping or the
selection battery, commit the regenerated JSON, then regenerate the Rust tables:

    python python/scripts/gen_normalize_table.py            # all supported locales
    python python/scripts/gen_normalize_table.py MNG        # specific locales
    python python/scripts/gen_normalize_table.py --check    # exit 1 if a bundled table is stale
    python python/scripts/gen_rust_tables.py                # JSON -> src/generated/

Output: python/mongol_norm/data/<LOCALE>.normalize.json  (commit it).
"""
import argparse
import json
import sys
from pathlib import Path

# python/ (the package, this script's siblings) and the repository root above it.
PYTHON_DIR = Path(__file__).resolve().parents[1]
ROOT = PYTHON_DIR.parent

# Make `mongol_norm` importable when run as `python python/scripts/gen_normalize_table.py`
# from the repository root (the extension must have been built into the package
# directory, e.g. with `cd python && maturin develop`).
sys.path.insert(0, str(PYTHON_DIR))

from mongol_norm import MongolianShaper  # noqa: E402
from mongol_norm._data import load_rules  # noqa: E402

# Locales with a normalization implementation in mongol-norm. Extend as more
# scripts gain normalize support.
LOCALES = ["MNG"]

# Canonical Unicode output policy. Bump this whenever normalize() may choose a
# different encoding for an existing supported shape.
CANONICAL_VERSION = "mng-canonical/2"

POSITIONS = ("isol", "init", "medi", "fina")

# Free Variation Selectors: the JSON records `fvs` as 0..4; the table stores the
# selector's codepoint (or None for a bare letter).
FVS_INT_TO_CP = {0: None, 1: 0x180B, 2: 0x180C, 3: 0x180D, 4: 0x180F}
MVS_CP = 0x180E
NIRUGU_CP = 0x180A
ZWJ_CP = 0x200D

# Velar feminine forms: the adjacent ambiguous vowel is encoded with its FEMININE
# letter for clean output (g+fvs2+o round-trips but looks wrong; oe is the
# linguistically-correct partner of a 'G' velar).
# velar 阴形:相邻歧义元音用阴性字母,输出更自然(g+fvs2+o 虽能还原但字难看;
# oe 才是 'G' velar 的语言学搭档)。
_VELAR_FEM_UNITS = frozenset({"G", "Gx"})
_MASC_TO_FEM_CP = {0x1820: 0x1821,   # a → e
                   0x1823: 0x1825,   # o → oe
                   0x1824: 0x1826}   # u → ue

# Context battery: neighbour letters used to probe context-independence.
# Cover masc vowel (a), fem vowel (e), masc round vowel (o), neutral (i),
# plain consonant (n), sibilant (s), velar (g), and a BOWED consonant (b) —
# without a true bowed neighbour the battery is blind to post_bowed effects
# (e.g. bare final a renders the swung Aa after b) and false-passes bare picks.
# 探针需含真弓形邻居(b),否则电池看不到 post_bowed 毒化,裸形会被误判安全。
CI_PROBE_LETTERS = (0x1820, 0x1821, 0x1823, 0x1822, 0x1828, 0x1830, 0x182D, 0x182A)


class LocaleData:
    """
    The letter-level views of the shaping-rules JSON the battery needs — the
    lookups the pre-0.1 pure-Python shaper built at load time (``cp_to_alias``,
    ``alias_to_cp``, ``feminine_vowels``, ``candidates_map``), rebuilt here from
    ``mongol_norm._data.load_rules`` so the runtime engine stays out of it.
    电池所需的字母级视图,从 shaping-rules JSON 重建(与旧纯 Python shaper 一致)。
    """

    def __init__(self, locale):
        self.rules = load_rules(locale)
        # Alias: cp ↔ alias string for this locale (aliases are locale-dependent
        # because the same codepoint may represent different phonemes in MNG vs
        # TOD vs SIB). / 别名是区域相关的。
        self.cp_to_alias = {}
        self.alias_to_cp = {}
        for letter in self.rules["letters"]:
            alias = letter.get("alias")
            if alias:
                self.cp_to_alias[letter["cp"]] = alias
                self.alias_to_cp[alias] = letter["cp"]
        categories = self.rules.get("categories", {})
        self.feminine_vowels = set(categories.get("vowelFeminine", []))
        self.candidates_map = _build_candidates_map(self.rules)


def _build_candidates_map(rules):
    """
    Build (position, written) → [(cp, fvs), ...]: every encoding that may
    render `written` at `position` (fvs is the JSON's 0..4 index).
    构建 (position, written) → [(cp, fvs), ...] 候选编码列表。

    Each variant contributes TWO candidates / 每个变体贡献两个候选:
      1. (cp, fvs)  — the explicit FVS encoding from data
      2. (cp, 0)    — the BARE encoding, which can produce the same
                      written when a runtime rule fires the matching
                      condition (e.g. bare `i.medi` after vowel →
                      vowel_devsger → ('I','I') even though data only
                      records this written under fvs=2)
    Bare encodings are added only when missing; the battery's shape()
    verification filters out combos whose rules don't actually fire.
    裸编码会被加入(若不存在):某些 shape 只能通过运行时规则触发
    condition 才能从裸字母得到(如元音后的 i.medi 经 vowel_devsger
    变成 ('I','I'),数据里只在 fvs=2 下记录)。

    Includes ALL variants for the locale — including archaic and
    unrecommended ones — so every shape that shape() can produce
    has at least one candidate encoding.
    包含此 locale 下所有变体(含 archaic / unrecommended),保证 shape()
    能产出的每个 shape 都至少有一个候选编码。
    """
    candidates_map = {}
    for letter in rules["letters"]:
        cp = letter["cp"]
        for variant in letter["variants"]:
            position = variant["position"]
            if position not in POSITIONS:
                continue
            written = tuple(str(unit) for unit in (variant.get("written") or ()))
            if not written:
                continue
            candidates_map.setdefault((position, written), []).append(
                (cp, int(variant["fvs"]))
            )

    # Add bare (fvs=0) encoding for any cp that appears in a slot but only
    # under non-zero FVS. shape() verification will weed out cases where
    # context doesn't fire the needed rule.
    # 为只在非零 FVS 出现的 cp 补加裸编码,shape() 校验会筛掉上下文不触发
    # 所需规则的情况。
    for candidates in candidates_map.values():
        bare_cps = {cp for cp, fvs in candidates if fvs == 0}
        candidates.extend(
            (cp, 0) for cp in sorted({cp for cp, _fvs in candidates} - bare_cps)
        )
    return candidates_map


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
            details = shaper.shape_detailed(left + letter + right)
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


def compute_positioned_units(rules):
    """Return the valid source-declared ``(unit, position)`` inventory."""
    positioned_units = set()
    for letter in rules["letters"]:
        for variant in letter["variants"]:
            records = variant.get("positioned_written")
            if not records:
                continue
            positioned_units.update(
                (record["unit"], record["position"])
                for record in records
            )
    return sorted(positioned_units)


def compute_unit_tables(shaper, data=None):
    """
    Run the context-independence battery against `shaper` and return
    (unit_table, feminine_table, max_length). This is the SELECTION method —
    the single source of truth the exported JSON encodes.
    """
    if data is None:
        data = LocaleData(shaper.locale)
    table = {}
    for (position, written), candidates in data.candidates_map.items():
        # MASCULINE-first for vowels; context-independence is the gate. Sort
        # masc-first, FVS-FIRST, cp asc, fvs asc, keep the FIRST that passes.
        # FVS-first because an FVS exists precisely to pin a form against
        # context; bare letters are context-sensitive by design and only
        # acceptable when no FVS'd candidate renders the unit.
        # FVS 优先:FVS 的意义就是隔离 context;裸字母天生受上下文感染,
        # 只有没有可用 FVS 候选时才退而用裸形。
        ordered_candidates = sorted(
            {(cp, FVS_INT_TO_CP[fvs]) for cp, fvs in candidates},
            key=lambda pair: (
                data.cp_to_alias.get(pair[0]) in data.feminine_vowels,
                pair[0], pair[1] is None, pair[1] or 0,
            )
        )
        for cp, fvs_cp in ordered_candidates:
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
    feminine_vowel_cps = {data.alias_to_cp[alias] for alias in data.feminine_vowels
                          if alias in data.alias_to_cp}
    feminine_table = {}
    for (position, written), candidates in data.candidates_map.items():
        if len(written) != 1:
            continue
        ordered_candidates = sorted(
            {(cp, FVS_INT_TO_CP[fvs]) for cp, fvs in candidates
             if cp in feminine_vowel_cps},
            key=lambda pair: (pair[0], pair[1] is None, pair[1] or 0)
        )
        for cp, fvs_cp in ordered_candidates:
            if _is_context_independent(shaper, position, written, cp, fvs_cp):
                feminine_table[(position, written)] = (cp, fvs_cp)
                break

    return table, feminine_table, max_length


def compute_normalize_tables(shaper):
    """
    Run the battery and return a JSON-serializable spec of the normalize tables
    — the artifact the Rust engine's tables and other-language ports consume.
    """
    data = LocaleData(shaper.locale)
    table, feminine_table, max_length = compute_unit_tables(shaper, data)
    positioned_units = compute_positioned_units(data.rules)

    def encode_entry(cp, fvs_cp):
        return {
            "letter": data.cp_to_alias.get(cp, ""),
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
        "positioned_units": [
            {"unit": unit, "position": position}
            for unit, position in positioned_units
        ],
        "constants": {
            "MVS": f"{MVS_CP:04X}",
            "NIRUGU": f"{NIRUGU_CP:04X}",
            "ZWJ": f"{ZWJ_CP:04X}",
            "FVS1": "180B", "FVS2": "180C",
            "FVS3": "180D", "FVS4": "180F",
        },
        "ci_probe_letters": [data.cp_to_alias.get(c, f"{c:04X}") for c in CI_PROBE_LETTERS],
        "velar_fem_units": sorted(_VELAR_FEM_UNITS),
        "masc_to_fem": {data.cp_to_alias.get(k): data.cp_to_alias.get(v)
                        for k, v in _MASC_TO_FEM_CP.items()},
        "unit_table": group_by_position(table),
        "velar_fem": group_by_position(feminine_table),
    }


def spec_text(spec):
    """The exact file content written for `spec` (what `--check` compares)."""
    return json.dumps(spec, ensure_ascii=False, indent=2) + "\n"


def _display(path):
    """`path` relative to the repository root when it lies inside it."""
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path)


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
        default=PYTHON_DIR / "mongol_norm" / "data",
        help="Output directory (default: python/mongol_norm/data/).",
    )
    parser.add_argument(
        "--check", action="store_true",
        help="fail instead of writing when a bundled table is stale",
    )
    args = parser.parse_args()

    target_locales = args.locales or list(LOCALES)
    for loc in target_locales:
        if loc not in LOCALES:
            parser.error(f"unknown locale {loc!r}. Supported: {', '.join(LOCALES)}")

    if not args.check:
        args.output_dir.mkdir(parents=True, exist_ok=True)

    stale = []
    for locale in target_locales:
        shaper = MongolianShaper(locale=locale)
        spec = compute_normalize_tables(shaper)
        content = spec_text(spec)
        out_path = args.output_dir / f"{locale}.normalize.json"
        if args.check:
            current = (out_path.read_text(encoding="utf-8")
                       if out_path.exists() else None)
            if current == content:
                print(f"fresh: {_display(out_path)}")
            else:
                stale.append(out_path)
            continue
        with out_path.open("w", encoding="utf-8") as f:
            f.write(content)
        n_units = sum(len(v) for v in spec["unit_table"].values())
        n_fem = sum(len(v) for v in spec["velar_fem"].values())
        print(
            f"Wrote {_display(out_path)} "
            f"({n_units} unit entries, {n_fem} velar-fem, "
            f"max_len {spec['unit_enc_max_len']})"
        )

    if stale:
        for path in stale:
            print(f"stale: {_display(path)}", file=sys.stderr)
        print("regenerate with: python python/scripts/gen_normalize_table.py "
              "(then python/scripts/gen_rust_tables.py)", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
