#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
The Rust crate at the repository root is the engine behind this package: the
binding crate (python/Cargo.toml) compiles it into ``mongol_norm._native``.

Two invariants keep the layers in lockstep:
  * the generated Rust tables (src/generated/*.rs) are exactly what
    python/scripts/gen_rust_tables.py produces from python/mongol_norm/data/*.json;
  * ``[workspace.package].version`` in the root Cargo.toml is the only version
    literal: the crate and the binding crate inherit it, pyproject.toml declares
    the package version dynamic (maturin reads it through the binding crate) and
    ``mongol_norm.__version__`` reports that same string.
"""
import re
import subprocess
import sys
import unittest

import mongol_norm
from tests._support import PYTHON_DIR, REPO_ROOT


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
    (REPO_ROOT / "Cargo.toml").exists() and (REPO_ROOT / "src").is_dir(),
    "the repository checkout (root Cargo.toml, src/) is not available",
)
class TestRustTwin(unittest.TestCase):
    def test_generated_tables_are_fresh(self):
        result = subprocess.run(
            [sys.executable, str(PYTHON_DIR / "scripts" / "gen_rust_tables.py"), "--check"],
            cwd=str(REPO_ROOT),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            encoding="utf-8",
            timeout=300,
        )
        self.assertEqual(
            result.returncode, 0,
            "generated Rust tables are stale; run python/scripts/gen_rust_tables.py\n"
            + result.stdout + result.stderr,
        )

    def test_crate_version_matches_package_version(self):
        root_manifest = REPO_ROOT / "Cargo.toml"
        self.assertEqual(
            _toml_version(root_manifest, "workspace.package"), mongol_norm.__version__
        )
        project = _toml_section(PYTHON_DIR / "pyproject.toml", "project")
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
        # The crate (root package) and the binding crate both inherit the literal.
        for manifest in (root_manifest, PYTHON_DIR / "Cargo.toml"):
            package = _toml_section(manifest, "package")
            self.assertRegex(
                package,
                re.compile(r"^version\.workspace = true$", re.MULTILINE),
                "{} must inherit the workspace version (version.workspace = true), "
                "not pin its own".format(manifest),
            )
            self.assertNotRegex(
                package,
                re.compile(r"^version\s*=", re.MULTILINE),
                "{} must not pin a version literal of its own".format(manifest),
            )

    def test_crate_package_includes_the_license_files(self):
        # The crate is the repository root, so `cargo package` ships the repository's
        # own LICENSE / NOTICE / README.md: they must stay in the `include` list.
        package = _toml_section(REPO_ROOT / "Cargo.toml", "package")
        match = re.search(r"^include\s*=\s*\[([^\]]*)\]", package, re.MULTILINE)
        self.assertIsNotNone(match, "the root Cargo.toml [package] must list `include`")
        included = set(re.findall(r'"([^"]+)"', match.group(1)))
        for name in ("README.md", "LICENSE", "NOTICE"):
            self.assertIn(name, included, "Cargo.toml `include` must list {}".format(name))
            self.assertTrue((REPO_ROOT / name).is_file(), "{} is missing".format(name))

    def test_python_license_files_are_copies_of_the_root_files(self):
        # maturin only picks up license files next to pyproject.toml (an include
        # pattern cannot reach `../LICENSE`), so python/ carries copies; keep them
        # byte-identical to the repository's files.
        for name in ("LICENSE", "NOTICE"):
            self.assertEqual(
                (PYTHON_DIR / name).read_bytes(),
                (REPO_ROOT / name).read_bytes(),
                "python/{0} differs from the repository {0}".format(name),
            )


if __name__ == "__main__":
    unittest.main()
