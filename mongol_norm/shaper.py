"""
Backwards-compatible import path for the pre-0.1 pure-Python module.

The shaping engine now lives in the Rust crate ``crates/mongol-norm`` and is
exposed through :mod:`mongol_norm._api`; this module only re-exports the public
names it used to define (``mongol-norm`` console script included).
"""
from ._api import MongolianShaper, NormalizationFallbackError, main

__all__ = ["MongolianShaper", "NormalizationFallbackError", "main"]
