#!/usr/bin/env python3
"""Generate or verify the MNG compatibility trace and canonical fixtures."""
import argparse
import importlib.util
import json
import sys
import types
from pathlib import Path


# python/ (the package and the Python test helpers this script reuses) and the
# repository root above it, whose tests/golden holds the shared fixtures.
PYTHON_DIR = Path(__file__).resolve().parents[1]
ROOT = PYTHON_DIR.parent
TESTS = PYTHON_DIR / "tests"
GOLDEN = ROOT / "tests" / "golden"
sys.path.insert(0, str(PYTHON_DIR))

# Load this repository's test helpers explicitly. Some Python environments ship
# an unrelated top-level ``tests`` package, which must not affect generation.
tests_package = types.ModuleType("tests")
tests_package.__path__ = [str(TESTS)]
sys.modules["tests"] = tests_package


def _load_test_module(name):
    qualified = "tests." + name
    spec = importlib.util.spec_from_file_location(qualified, TESTS / (name + ".py"))
    if spec is None or spec.loader is None:
        raise ImportError("cannot load test helper {!r}".format(name))
    module = importlib.util.module_from_spec(spec)
    sys.modules[qualified] = module
    spec.loader.exec_module(module)
    return module


round_trip = _load_test_module("test_round_trip")
canonical_golden = _load_test_module("test_canonical_golden")

from mongol_norm import MongolianShaper  # noqa: E402


ALIASES = dict(round_trip._ALIAS_TO_CP)
PHASE_CASES = [
    ("iii1-chachlag", "t a l mvs a", "III.1.chachlag"),
    ("iii2a-marked", "ch u", "III.2a.o_u_oe_ue.marked"),
    ("iii2a-marked-gb-a", "d fvs1 ue", "III.2a.o_u_oe_ue.marked.GB.A"),
    ("iii2a-marked-gb-b", "h fvs2 ue", "III.2a.o_u_oe_ue.marked.GB.B"),
    ("iii2a-cluster-marked", "m n oe g e", "III.2a.oe_ue.cluster.marked"),
    ("iii2a-d-marked", "d a", "III.2a.d.marked"),
    ("iii2c-chachlag-onset", "s a i n mvs a", "III.2c.chachlag_onset"),
    ("iii2e-onset-devsger", "a n d a", "III.2e.n_t_d.onset_devsger"),
    ("iii2f-hg-harmony", "g a r", "III.2f.h_g.harmony"),
    ("iii2g-t-devsger", "t ee n", "III.2g.t.devsger"),
    ("iii2g-sh-dotless", "sh i n", "III.2g.sh.dotless"),
    ("iii2g-g-dotless", "s g a", "III.2g.g.dotless"),
    ("iii3-particle", "mvs y i n", "III.3.particle"),
    ("iii4-vowel-devsger", "a i n", "III.4.vowel_devsger"),
    ("iii5-post-bowed", "b a", "III.5.post_bowed"),
]


def _json_text(value):
    return json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"


def _canonical_jsonl_text(value):
    manifest = {key: item for key, item in value.items() if key != "vectors"}
    manifest["type"] = "manifest"
    lines = [json.dumps(manifest, ensure_ascii=False, sort_keys=True,
                        separators=(",", ":"))]
    lines.extend(
        json.dumps(dict({"type": "vector"}, **vector),
                   ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        for vector in value["vectors"]
    )
    return "\n".join(lines) + "\n"


def build_phase_trace(shaper):
    vectors = []
    for vector_id, aliases, witness_rule in PHASE_CASES:
        text = "".join(ALIASES[alias] for alias in aliases.split())
        codepoints = [ord(char) for char in text]
        vectors.append({
            "id": vector_id,
            "witness_rule": witness_rule,
            "input_cps": codepoints,
            "input_aliases": aliases,
            "expected": shaper.trace(text),
        })
    return {
        "schema": "mongol-norm-phase-trace/1",
        "locale": "MNG",
        "rules": shaper.rule_names(),
        "vectors": vectors,
    }


def build_canonical(shaper):
    groups = canonical_golden._shape_groups(shaper)
    vectors = []
    for index, (shape, word) in enumerate(sorted(groups.items()), 1):
        vectors.append({
            "id": "shape-{:04d}".format(index),
            "input_cps": [ord(char) for char in word],
            "shape": list(shape),
            "normalized_cps": [ord(char) for char in shaper.normalize(word)],
        })
    return {
        "schema": "mongol-norm-canonical-golden/1",
        "locale": "MNG",
        "canonical_version": shaper.canonical_version,
        "vectors": vectors,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true",
                        help="fail instead of writing when a fixture is stale")
    args = parser.parse_args()

    shaper = MongolianShaper(locale="MNG")
    generated = {
        GOLDEN / "mng-phase-trace-v1.json": _json_text(build_phase_trace(shaper)),
        GOLDEN / "mng-canonical-v1.jsonl": _canonical_jsonl_text(
            build_canonical(shaper)
        ),
    }

    stale = []
    for path, content in generated.items():
        current = path.read_text(encoding="utf-8") if path.exists() else None
        if current == content:
            print("fresh: {}".format(path.relative_to(ROOT)))
            continue
        stale.append(path)
        if not args.check:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
            print("wrote: {}".format(path.relative_to(ROOT)))

    if args.check and stale:
        for path in stale:
            print("stale: {}".format(path.relative_to(ROOT)), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
