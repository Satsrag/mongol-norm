# mongol-norm (Python)

[![Test](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml/badge.svg)](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml)

Shape-aware normalizer for Traditional Mongolian script. Python reference implementation of the UTN #57 v4 shaping pipeline — no font files, no HarfBuzz required.

Verified against `mongfontbuilder/core-hud.tsv` (177/177) and the GB/T 25914-2023 `eac-hud.tsv` suite (3507/3507, with 5 UTN ↔ EAC xfail cases matching mongfontbuilder's own `pytest.mark.xfail` set).

> Background, motivation, and the "why five encodings of *sain* look identical" story is in the [root README](../README.md). This page is the Python install/usage only.

## Install

Neither package is on PyPI yet — install both from this repo:

```sh
git clone https://github.com/Satsrag/mongol-norm
cd mongol-norm
pip install ./mongol-shape-data
pip install ./mongol-norm
```

## Usage

```python
from mongol_norm import MongolianShaper

shaper = MongolianShaper(locale="MNG")  # Hudum Traditional Mongolian

# Shape: get written-unit sequence
shaper.shape("ᠰᠠᠢᠨ")
# → ['S', 'A', 'I', 'I', 'A']

# Compare: are two encodings visually identical?
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")
# → True

# Normalize: canonical bare Unicode
shaper.normalize("ᠰᠡᠢᠨ")
# → 'ᠰᠠᠢᠨ'

# Full-text (per-word normalization, preserves non-Mongolian)
shaper.normalize_text("Hello ᠰᠡᠢᠨ world")
# → 'Hello ᠰᠠᠢᠨ world'
```

## Supported locales

| Locale | Script | Status |
|---|---|---|
| `MNG` | Hudum (Traditional Mongolian) | ✅ Full shaping + normalization |
| `TOD` | Todo | ⬜ Shaping rules present; normalization WIP |
| `SIB` | Sibe | ⬜ Shaping rules present; normalization WIP |
| `MCH` | Manchu | ⬜ Shaping rules present; normalization WIP |

## Development

```sh
git clone https://github.com/Satsrag/mongol-norm
cd mongol-norm
pip install -e "./mongol-shape-data"
pip install -e "./mongol-norm[dev]"

# Run all tests (113 hand-written + 177 core-hud + 3507 eac-hud)
cd mongol-norm
python -m unittest tests.test_shaper tests.test_core_hud tests.test_eac_hud
```

## Requirements

- Python 3.9+ (CI matrix: 3.9 – 3.13)
- `mongol-shape-data` (installed automatically)

## License

SIL Open Font License 1.1 — consistent with upstream sources (UTN #57, mongfontbuilder).
