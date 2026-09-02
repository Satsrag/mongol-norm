"""
mongol-norm — shape-aware normalizer for Traditional Mongolian script.

The engine is the Rust crate ``mongol-norm`` (``crates/mongol-norm``), compiled
into the ``mongol_norm._native`` extension; :mod:`mongol_norm._api` wraps it.
"""
from ._api import MongolianShaper, NormalizationFallbackError
from ._native import version as _version

__version__ = _version()
__all__ = ["MongolianShaper", "NormalizationFallbackError"]
