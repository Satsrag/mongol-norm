"""Byte-for-byte canonical Unicode golden vectors for MNG corpus shapes."""
import json
import unittest
from unittest import mock

from mongol_norm import MongolianShaper
from tests._support import fixture_path
from tests.test_round_trip import (
    INLINE_CASES,
    _ALIAS_TO_CP,
    _aliases_to_words,
    _load_tsv,
)


_GOLDEN = fixture_path("golden", "mng-canonical-v1.jsonl")


def _all_corpus_words():
    for _label, aliases in INLINE_CASES:
        for word in _aliases_to_words(aliases):
            if word:
                yield word
    for filename in ("core-hud.tsv", "eac-hud.tsv"):
        for index, aliases, _expected in _load_tsv(filename):
            tokens = aliases.split()
            unknown = sorted({token for token in tokens
                              if token != "space" and token not in _ALIAS_TO_CP})
            if unknown:
                raise ValueError(
                    "{}:{} unknown aliases: {}".format(
                        filename, index, ", ".join(unknown)
                    )
                )
            for word in _aliases_to_words(aliases):
                if word:
                    yield word


def _shape_groups(shaper):
    groups = {}
    for word in _all_corpus_words():
        shape = tuple(shaper.shape(word))
        current = groups.get(shape)
        if current is None or tuple(map(ord, word)) < tuple(map(ord, current)):
            groups[shape] = word
    return groups


class TestMNGCanonicalGolden(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.shaper = MongolianShaper(locale="MNG")
        with _GOLDEN.open(encoding="utf-8") as stream:
            records = [json.loads(line) for line in stream if line.strip()]
        cls.golden = records[0]
        cls.vectors = records[1:]
        if cls.golden.get("type") != "manifest":
            raise ValueError("first canonical golden record must be a manifest")
        if any(record.get("type") != "vector" for record in cls.vectors):
            raise ValueError("canonical golden contains a non-vector record")

    def test_fixture_metadata(self):
        self.assertEqual(self.golden["schema"], "mongol-norm-canonical-golden/1")
        self.assertEqual(self.golden["locale"], "MNG")
        self.assertEqual(self.golden["canonical_version"], "mng-canonical/2")
        self.assertEqual(self.shaper.canonical_version,
                         self.golden["canonical_version"])

    def test_fixture_covers_every_current_corpus_shape_group(self):
        corpus_shapes = set(_shape_groups(self.shaper))
        golden_shapes = {tuple(vector["shape"])
                         for vector in self.vectors}
        self.assertEqual(golden_shapes, corpus_shapes)

    def test_fixture_has_frozen_unique_cardinality(self):
        # 1993 before the duplicate encodings were folded out of `shape`: two of those raw
        # groups were the same visible word spelled two ways (`Nirugu H Nirugu` =
        # `Nirugu A A Nirugu`, and `A Dd` = `A O A`), so they merged.
        self.assertEqual(len(self.vectors), 1991)
        self.assertEqual(
            [vector["id"] for vector in self.vectors],
            ["shape-{:04d}".format(index) for index in range(1, 1992)],
        )
        self.assertEqual(len({tuple(vector["shape"])
                              for vector in self.vectors}), 1991)

    def test_canonical_codepoints_are_frozen(self):
        for vector in self.vectors:
            with self.subTest(vector=vector["id"]):
                text = "".join(chr(cp) for cp in vector["input_cps"])
                self.assertEqual(self.shaper.shape(text), vector["shape"])
                self.assertEqual(
                    [ord(char) for char in self.shaper.normalize(text)],
                    vector["normalized_cps"],
                )


class TestCanonicalCorpusInput(unittest.TestCase):
    def test_unknown_alias_fails_instead_of_shrinking_coverage(self):
        bad_row = [(1, "a typo_alias", [])]
        with mock.patch(__name__ + "._load_tsv", return_value=bad_row):
            with self.assertRaisesRegex(ValueError, "unknown aliases"):
                list(_all_corpus_words())


if __name__ == "__main__":
    unittest.main()
