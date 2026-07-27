"""
Tests for the exportable normalize table (the per-unit FVS-pinned spec that
other-language ports consume as JSON).

The selection battery lives in scripts/gen_normalize_table.py (build-time). It
must serialize losslessly, the shaper must LOAD that serialized form, and the
freshly-computed spec must match the bundled JSON the runtime loads.
导出归一化表的测试:生成器在 scripts;序列化无损、shaper 能加载、且现算的
spec 与随包 JSON 一致。
"""
import json
import sys
import unittest
from pathlib import Path

from mongol_norm.shaper import MongolianShaper

# The selection battery / spec generator lives in scripts/ (build-time only).
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "scripts"))
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

    def test_compute_returns_json_serializable_spec(self):
        spec = gen.compute_normalize_tables(self.s)
        self.assertEqual(spec["locale"], "MNG")
        self.assertEqual(spec["canonical_version"], "mng-canonical/1")
        self.assertIn("schema", spec)
        for key in ("unit_table", "velar_fem", "velar_fem_units",
                    "masc_to_fem", "constants", "unit_enc_max_len"):
            self.assertIn(key, spec, f"spec missing {key!r}")
        for pos in ("isol", "init", "medi", "fina"):
            self.assertIn(pos, spec["unit_table"])
        # must be JSON-serializable as-is
        json.dumps(spec)

    def test_shaper_exposes_loaded_canonical_version(self):
        self.assertEqual(self.s.canonical_version, "mng-canonical/1")

    def test_loader_rejects_unexpected_canonical_version(self):
        spec = gen.compute_normalize_tables(self.s)
        spec["canonical_version"] = "mng-canonical/999"
        loaded = MongolianShaper(locale="MNG")
        with self.assertRaisesRegex(ValueError, "unsupported canonical version"):
            loaded._load_normalize_tables(spec)

    def test_spec_matches_bundled_table(self):
        """A freshly-computed spec matches the bundled table the shaper loads
        (guards against a stale committed JSON)."""
        self.s._build_unit_enc()  # loads mongol_norm/data/MNG.normalize.json
        spec = gen.compute_normalize_tables(self.s)
        self.assertEqual(_rebuild_unit_enc(spec), dict(self.s._unit_enc))

    def test_shaper_loads_spec_identically(self):
        """A shaper populated from a fresh spec equals one loaded from the
        bundled JSON."""
        spec = gen.compute_normalize_tables(self.s)
        loaded = MongolianShaper(locale="MNG")
        loaded._load_normalize_tables(spec)

        baseline = MongolianShaper(locale="MNG")
        baseline._build_unit_enc()

        self.assertEqual(dict(loaded._unit_enc), dict(baseline._unit_enc))
        self.assertEqual(dict(loaded._unit_enc_fem),
                         dict(baseline._unit_enc_fem))
        self.assertEqual(loaded._unit_enc_max_len,
                         baseline._unit_enc_max_len)

    def test_normalize_identical_when_loaded_from_spec(self):
        """normalize() output is identical whether table is loaded or computed."""
        spec = gen.compute_normalize_tables(self.s)
        loaded = MongolianShaper(locale="MNG")
        loaded._load_normalize_tables(spec)
        baseline = MongolianShaper(locale="MNG")
        samples = ["ᠰᠠᠢᠨ", "ᠡᠭᠦᠨ", "ᠮᠣᠩᠭᠣᠯ", "ᠨᠣᠮ", "ᠪᠠᠶᠠᠨ", "ᠲᠩᠷᠢ"]
        for w in samples:
            self.assertEqual(loaded.normalize(w), baseline.normalize(w),
                             f"mismatch on {w!r}")

    def test_loaded_shaper_handles_particles(self):
        """
        A shaper whose tables came from the spec (battery never ran, so the
        candidates map was never built as a side effect) must still apply
        particle substitution — which needs the candidates map. Regression:
        the load path used to leave _candidates_map unset, crashing normalize
        on particle words.
        从 spec 加载的 shaper(电池没跑,candidates 未作为副作用构建)仍须能做
        particle 替换。回归:加载路径曾遗漏 _candidates_map,particle 词崩溃。
        """
        spec = gen.compute_normalize_tables(self.s)
        loaded = MongolianShaper(locale="MNG")
        loaded._load_normalize_tables(spec)
        self.assertFalse(hasattr(loaded, "_candidates_map"))
        baseline = MongolianShaper(locale="MNG")
        # particle / chachlag stressors that route through particle substitution
        for w in ["ᠮᠣᠩᠭᠣᠯ ᠤᠨ", "ᠡᠭᠦᠨ ᠦ", "ᠨᠠᠮ ᠠ"]:
            self.assertEqual(loaded.normalize_text(w),
                             baseline.normalize_text(w), f"mismatch on {w!r}")


if __name__ == "__main__":
    unittest.main()
