#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
The Rust core (crates/mongol-norm) is a twin of this package.

Two invariants keep the twins in lockstep:
  * the generated Rust tables (src/generated/*.rs) are exactly what
    scripts/gen_rust_tables.py produces from mongol_norm/data/*.json;
  * the workspace version equals the Python package version, and the
    member crate (crates/mongol-norm) inherits it rather than pinning
    its own.
"""
import re
import subprocess
import sys
import unittest
from pathlib import Path

import mongol_norm

ROOT = Path(__file__).resolve().parents[1]


def _toml_version(path, section):
    """The `version = "..."` of `[section]` (regex, not tomllib: Python 3.9 support)."""
    text = path.read_text(encoding="utf-8")
    header = re.search(r"^\[" + re.escape(section) + r"\]\s*$", text, re.MULTILINE)
    if header is None:
        raise AssertionError("no [{}] section in {}".format(section, path))
    body = text[header.end():]
    next_header = re.search(r"^\[", body, re.MULTILINE)
    if next_header is not None:
        body = body[:next_header.start()]
    match = re.search(r'^version\s*=\s*"([^"]+)"', body, re.MULTILINE)
    if match is None:
        raise AssertionError("no version field in [{}] of {}".format(section, path))
    return match.group(1)


@unittest.skipUnless(
    (ROOT / "Cargo.toml").exists(),
    "the Rust twin (Cargo.toml, crates/) is not part of the sdist",
)
class TestRustTwin(unittest.TestCase):
    def test_generated_tables_are_fresh(self):
        result = subprocess.run(
            [sys.executable, str(ROOT / "scripts" / "gen_rust_tables.py"), "--check"],
            cwd=str(ROOT),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            universal_newlines=True,
        )
        self.assertEqual(
            result.returncode, 0,
            "generated Rust tables are stale; run scripts/gen_rust_tables.py\n"
            + result.stdout + result.stderr,
        )

    def test_crate_version_matches_package_version(self):
        self.assertEqual(
            _toml_version(ROOT / "Cargo.toml", "workspace.package"), mongol_norm.__version__
        )
        self.assertEqual(_toml_version(ROOT / "pyproject.toml", "project"), mongol_norm.__version__)
        crate_manifest = ROOT / "crates" / "mongol-norm" / "Cargo.toml"
        self.assertRegex(
            crate_manifest.read_text(encoding="utf-8"),
            re.compile(r"^version\.workspace = true$", re.MULTILINE),
            "{} must inherit the workspace version (version.workspace = true), "
            "not pin its own".format(crate_manifest),
        )

    def test_crate_license_files_are_copies_of_the_root_files(self):
        # `cargo package` cannot reach ../.., so the crate ships copies; keep them identical.
        for name in ("LICENSE", "NOTICE"):
            self.assertEqual(
                (ROOT / "crates" / "mongol-norm" / name).read_bytes(),
                (ROOT / name).read_bytes(),
                "crates/mongol-norm/{0} differs from the repository {0}".format(name),
            )


if __name__ == "__main__":
    unittest.main()
