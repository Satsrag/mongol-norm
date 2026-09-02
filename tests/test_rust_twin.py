#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
The Rust core (crates/mongol-norm) is the engine behind this package: the
binding crate (crates/mongol-norm-py) compiles it into ``mongol_norm._native``.

Two invariants keep the layers in lockstep:
  * the generated Rust tables (src/generated/*.rs) are exactly what
    scripts/gen_rust_tables.py produces from mongol_norm/data/*.json;
  * ``[workspace.package].version`` in the root Cargo.toml is the only version
    literal: both member crates inherit it, pyproject.toml declares the
    package version dynamic (maturin reads it through the binding crate) and
    ``mongol_norm.__version__`` reports that same string.
"""
import re
import subprocess
import sys
import unittest
from pathlib import Path

import mongol_norm

ROOT = Path(__file__).resolve().parents[1]


def _toml_section(path, section):
    """The body of `[section]` (regex, not tomllib: Python 3.9 support)."""
    text = path.read_text(encoding="utf-8")
    header = re.search(r"^\[" + re.escape(section) + r"\]\s*$", text, re.MULTILINE)
    if header is None:
        raise AssertionError("no [{}] section in {}".format(section, path))
    body = text[header.end():]
    next_header = re.search(r"^\[", body, re.MULTILINE)
    if next_header is not None:
        body = body[:next_header.start()]
    return body


def _toml_version(path, section):
    """The `version = "..."` of `[section]`."""
    match = re.search(r'^version\s*=\s*"([^"]+)"', _toml_section(path, section), re.MULTILINE)
    if match is None:
        raise AssertionError("no version field in [{}] of {}".format(section, path))
    return match.group(1)


@unittest.skipUnless(
    (ROOT / "Cargo.toml").exists(),
    "the Rust workspace (Cargo.toml, crates/) is not part of this checkout",
)
class TestRustTwin(unittest.TestCase):
    def test_generated_tables_are_fresh(self):
        result = subprocess.run(
            [sys.executable, str(ROOT / "scripts" / "gen_rust_tables.py"), "--check"],
            cwd=str(ROOT),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            encoding="utf-8",
            timeout=300,
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
        project = _toml_section(ROOT / "pyproject.toml", "project")
        self.assertRegex(
            project,
            re.compile(r'^dynamic\s*=\s*\[\s*"version"\s*\]', re.MULTILINE),
            "pyproject.toml must declare `dynamic = [\"version\"]` (maturin reads "
            "the version from the workspace)",
        )
        self.assertNotRegex(
            project,
            re.compile(r"^version\s*=", re.MULTILINE),
            "pyproject.toml must not pin a version literal of its own",
        )
        for crate in ("mongol-norm", "mongol-norm-py"):
            crate_manifest = ROOT / "crates" / crate / "Cargo.toml"
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
