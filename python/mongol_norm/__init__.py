"""
mongol-norm — Traditional Mongolian (Hudum) same-shape comparison and
normalization under UTN #57 (GB/T 25914—2023).

The engine is the Rust crate ``mongol-norm`` (the repository root), compiled
into the ``mongol_norm._native`` extension; :mod:`mongol_norm._api` wraps it.
"""
from ._api import MongolianShaper, NormalizationFallbackError
from ._native import version as _version

__version__ = _version()
__all__ = ["MongolianShaper", "NormalizationFallbackError"]
