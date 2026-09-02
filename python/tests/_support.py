"""
Shared helpers for the test-suite (not a test module: discovery only collects
``test_*.py``; import it as ``tests._support``).

* :func:`run_cli` runs the ``mongol-norm`` command line in a subprocess via
  ``python -m mongol_norm.shaper``. The CLI is the Rust crate's and writes to
  the process's real file descriptors, so in-process capture cannot observe
  it. The child runs from the directory that contains the imported
  ``mongol_norm`` package, so it imports exactly the package under test —
  the editable ``maturin develop`` install and an installed wheel alike.
* The ``testing`` hook: the extension exposes
  ``_shaper_with_empty_normalize_table`` only when built with
  ``maturin develop --features testing``. Tests that need it are decorated
  with :data:`needs_testing_hook` and use :func:`empty_table_shaper`. Set
  ``MONGOL_NORM_REQUIRE_TESTING_HOOK=1`` (CI does) to make a missing hook a
  hard error at import time instead of silent skips.
* Repository layout: this suite lives in ``python/tests`` next to the package
  and the generator scripts (:data:`PYTHON_DIR`); the parent directory is the
  Rust crate's root (:data:`REPO_ROOT`), whose ``tests/`` holds the corpus and
  golden fixtures once, shared with the Rust integration tests
  (:func:`fixture_path`).
"""
import os
import subprocess
import sys
import unittest
from pathlib import Path

import mongol_norm
from mongol_norm import MongolianShaper, _native

# Directory containing the imported package (`python/` for the editable
# `maturin develop` install, site-packages for a wheel).
PACKAGE_PARENT = Path(mongol_norm.__file__).resolve().parents[1]

# `python/` (this suite, the package, the scripts) and the repository root above
# it (the Rust crate: Cargo.toml, src/, tests/ with the shared fixtures).
PYTHON_DIR = Path(__file__).resolve().parents[1]
REPO_ROOT = PYTHON_DIR.parent
FIXTURES_DIR = REPO_ROOT / "tests"


def fixture_path(*parts):
    """Path of a shared fixture, e.g. ``fixture_path("data", "core-hud.tsv")``."""
    return FIXTURES_DIR.joinpath(*parts)


HAS_TESTING_HOOK = hasattr(_native, "_shaper_with_empty_normalize_table")
if os.environ.get("MONGOL_NORM_REQUIRE_TESTING_HOOK") and not HAS_TESTING_HOOK:
    raise AssertionError(
        "MONGOL_NORM_REQUIRE_TESTING_HOOK is set but mongol_norm._native has no "
        "_shaper_with_empty_normalize_table: build the extension with "
        "`maturin develop --features testing`"
    )

needs_testing_hook = unittest.skipUnless(
    HAS_TESTING_HOOK, "the extension was built without `--features testing`"
)


def empty_table_shaper(locale="MNG"):
    """A shaper whose normalize table is empty, so every chain falls back."""
    return MongolianShaper._wrap(_native._shaper_with_empty_normalize_table(locale))


def run_cli(*args, stdin=None, timeout=60):
    """Run ``mongol-norm <args>`` in a subprocess; returns the CompletedProcess."""
    return subprocess.run(
        [sys.executable, "-m", "mongol_norm.shaper", *args],
        input=stdin,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        encoding="utf-8",
        timeout=timeout,
        cwd=str(PACKAGE_PARENT),
    )
