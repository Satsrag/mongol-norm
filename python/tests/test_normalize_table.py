"""
Tests for the exportable normalize table (the per-unit FVS-pinned spec that the
Rust engine's generated tables and other-language ports consume as JSON).

The selection battery lives in python/scripts/gen_normalize_table.py (build-time).
It must serialize losslessly, and the freshly-computed spec must match the bundled
JSON (`python/mongol_norm/data/MNG.normalize.json`) — the file
`python/scripts/gen_rust_tables.py` compiles into the engine
(`python/tests/test_rust_twin.py` checks that step is fresh, and the canonical
golden proves what the compiled table encodes).
导出归一化表的测试:生成器在 python/scripts;序列化无损、且现算的 spec 与随包 JSON 一致
(该 JSON 再由 gen_rust_tables.py 编译进 Rust 引擎)。
"""
import json
import subprocess
import sys
import unittest

from mongol_norm import MongolianShaper
from mongol_norm._data import load_normalize_table, normalize_table_path
from tests._support import PYTHON_DIR, REPO_ROOT

# The selection battery / spec generator lives in python/scripts/ (build-time only).
sys.path.insert(0, str(PYTHON_DIR / "scripts"))
import gen_normalize_table as gen  # noqa: E402


def _rebuild_unit_enc(spec):
    """Reconstruct the {(pos, written_tuple): (cp, fvs_cp)} dict from a spec."""
    out = {}
    for pos, entries in spec["unit_table"].items():
        for wkey, v in entries.items():
            written = tuple(wkey.split("+"))
            cp = int(v["cp"], 16)
            fvs = int(v["fvs"], 16) if v["fvs"] else None
            out[(pos, written)] = (cp, fvs)
    return out


class TestNormalizeTableExport(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.s = MongolianShaper(locale="MNG")
        cls.spec = gen.compute_normalize_tables(cls.s)  # runs the battery once
        cls.bundled = load_normalize_table("MNG")

    def test_compute_returns_json_serializable_spec(self):
        spec = self.spec
        self.assertEqual(spec["locale"], "MNG")
        self.assertEqual(spec["canonical_version"], "mng-canonical/2")
        self.assertIn("schema", spec)
        for key in ("unit_table", "positioned_units",
                    "velar_fem", "velar_fem_units", "masc_to_fem",
                    "constants", "unit_enc_max_len"):
            self.assertIn(key, spec, f"spec missing {key!r}")
        self.assertNotIn("positioned_unit_table", spec)
        self.assertNotIn("positioned_left_mvs_table", spec)
        self.assertNotIn("positioned_unit_enc_max_len", spec)
        for pos in ("isol", "init", "medi", "fina"):
            self.assertIn(pos, spec["unit_table"])
        # must be JSON-serializable as-is
        json.dumps(spec)

    def test_shaper_exposes_loaded_canonical_version(self):
        self.assertEqual(self.s.canonical_version, "mng-canonical/2")
        self.assertEqual(self.s.canonical_version, self.bundled["canonical_version"])
        self.assertEqual(self.s.canonical_version, gen.CANONICAL_VERSION)

    def test_spec_matches_bundled_table(self):
        """A freshly-computed spec matches the bundled table (guards against a
        stale committed JSON — the engine's tables are generated from it)."""
        self.assertEqual(self.spec, self.bundled)

    def test_bundled_table_is_byte_fresh(self):
        """The committed file is exactly what the generator writes (`--check`)."""
        self.assertEqual(
            normalize_table_path("MNG").read_text(encoding="utf-8"),
            gen.spec_text(self.spec),
        )
        result = subprocess.run(
            [sys.executable, str(PYTHON_DIR / "scripts" / "gen_normalize_table.py"), "--check"],
            cwd=str(REPO_ROOT),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            encoding="utf-8",
            timeout=300,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("fresh:", result.stdout)

    def test_positioned_inventory_matches_hud_contract(self):
        pairs = {
            (record["unit"], record["position"])
            for record in self.spec["positioned_units"]
        }
        self.assertEqual(len(pairs), 95)
        self.assertIn(("F", "init"), pairs)
        self.assertIn(("I", "isol"), pairs)
        self.assertNotIn(("F", "isol"), pairs)
        self.assertNotIn(("Zwj", "control"), pairs)
        self.assertEqual(
            pairs,
            {(record["unit"], record["position"])
             for record in self.bundled["positioned_units"]},
        )

    def test_pinned_entries_render_their_unit_in_the_engine(self):
        """Every table entry is a (letter, FVS) that shapes to exactly its
        written unit at its position — the property the battery selects for,
        re-checked against the compiled engine with a neutral neighbour."""
        probe = "ᠨ"  # n: a plain consonant on either side
        for (position, written), (cp, fvs_cp) in _rebuild_unit_enc(self.spec).items():
            with self.subTest(position=position, written=written):
                letter = chr(cp) + (chr(fvs_cp) if fvs_cp is not None else "")
                left = probe if position in ("medi", "fina") else ""
                right = probe if position in ("init", "medi") else ""
                details = self.s.shape_detailed(left + letter + right)
                detail = details[1 if left else 0]
                self.assertEqual(detail["position"], position)
                self.assertEqual(tuple(detail["written"]), written)


if __name__ == "__main__":
    unittest.main()
