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
"""
import os
import subprocess
import sys
import unittest
from pathlib import Path

import mongol_norm
from mongol_norm import MongolianShaper, _native

# Directory containing the imported package (the repository root for the
# editable install, site-packages for a wheel).
PACKAGE_PARENT = Path(mongol_norm.__file__).resolve().parents[1]

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
