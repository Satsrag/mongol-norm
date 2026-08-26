"""
mongol-norm — shape-aware normalizer for Traditional Mongolian script.
"""
from .shaper import MongolianShaper, NormalizationFallbackError

__version__ = "0.0.4"
__all__ = ["MongolianShaper", "NormalizationFallbackError"]
