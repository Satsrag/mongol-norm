"""
Bundled shaping data for mongol-norm (internal).

The JSON in `mongol_norm/data/` is the runtime data the shaper consumes:

  * `<LOCALE>.json`            — flat, language-agnostic shaping rules,
                                 generated from mongfontbuilder + UTN #57.
  * `<LOCALE>.normalize.json`  — the normalize table (per-(position,
                                 written-unit) FVS-pinned encodings),
                                 generated from this package's own shaper.

Both are produced by the dev-only scripts in `scripts/` and committed here, so
the runtime needs no extra dependency and other-language ports can read the raw
files directly (see docs/data-format.md).

These loaders are internal — import `MongolianShaper`, not this module.
"""
import json
from importlib.resources import files
from typing import Any, Dict

# Bump on incompatible changes to the shaping-rules JSON schema.
SCHEMA_VERSION = 1
SUPPORTED_LOCALES = ("MNG", "MNGx", "TOD", "TODx", "SIB", "MCH", "MCHx")

_DATA = files(__package__) / "data"


def _base(locale: str) -> str:
    return locale.removesuffix("x") if locale.endswith("x") else locale


def load_rules(locale: str) -> Dict[str, Any]:
    """Load the flat shaping-rules JSON for `locale`."""
    with (_DATA / f"{_base(locale)}.json").open(encoding="utf-8") as f:
        return json.load(f)


def rules_path(locale: str):
    """Filesystem path to the shaping-rules JSON for `locale`."""
    return _DATA / f"{_base(locale)}.json"


def load_normalize_table(locale: str) -> Dict[str, Any]:
    """
    Load the precomputed normalize table for `locale`. Raises FileNotFoundError
    if it has not been generated yet (the shaper then falls back to running the
    context-independence battery in-process).
    """
    with (_DATA / f"{_base(locale)}.normalize.json").open(encoding="utf-8") as f:
        return json.load(f)


def normalize_table_path(locale: str):
    """Filesystem path to the normalize-table JSON for `locale`."""
    return _DATA / f"{_base(locale)}.normalize.json"
