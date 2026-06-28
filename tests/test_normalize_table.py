"""
Tests for the exportable normalize table (the per-unit FVS-pinned spec that
other-language ports consume as JSON).

The selection logic (the context-independence battery) is the source of truth.
`compute_normalize_tables()` must serialize it losslessly, and the shaper must
be able to LOAD that serialized form and behave identically to computing it.
导出归一化表的测试:compute 序列化无损,且 shaper 从序列化形式加载后行为一致。
"""
import json
import unittest

from mongol_norm.shaper import MongolianShaper


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
        spec = self.s.compute_normalize_tables()
        self.assertEqual(spec["locale"], "MNG")
        self.assertIn("schema", spec)
        for key in ("unit_table", "velar_fem", "velar_fem_units",
                    "masc_to_fem", "constants", "unit_enc_max_len"):
            self.assertIn(key, spec, f"spec missing {key!r}")
        for pos in ("isol", "init", "medi", "fina"):
            self.assertIn(pos, spec["unit_table"])
        # must be JSON-serializable as-is
        json.dumps(spec)

    def test_spec_reconstructs_battery_unit_enc(self):
        """The exported spec rebuilds EXACTLY the battery-computed table."""
        self.s._build_unit_enc()
        spec = self.s.compute_normalize_tables()
        self.assertEqual(_rebuild_unit_enc(spec), dict(self.s._unit_enc))

    def test_shaper_loads_spec_identically(self):
        """A shaper populated from the spec equals one built by the battery."""
        spec = self.s.compute_normalize_tables()
        loaded = MongolianShaper(locale="MNG")
        loaded._load_normalize_tables(spec)

        baseline = MongolianShaper(locale="MNG")
        baseline._build_unit_enc()

        self.assertEqual(dict(loaded._unit_enc), dict(baseline._unit_enc))
        self.assertEqual(dict(loaded._unit_enc_fem),
                         dict(baseline._unit_enc_fem))
        self.assertEqual(set(loaded._required_multi),
                         set(baseline._required_multi))
        self.assertEqual(loaded._unit_enc_max_len,
                         baseline._unit_enc_max_len)

    def test_normalize_identical_when_loaded_from_spec(self):
        """normalize() output is identical whether table is loaded or computed."""
        spec = self.s.compute_normalize_tables()
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
        spec = self.s.compute_normalize_tables()
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
