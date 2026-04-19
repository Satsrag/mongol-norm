# mongol-norm (Python)

Shape-aware normalizer for Traditional Mongolian script. Python reference implementation of the UTN #57 v4 shaping pipeline — no font files, no HarfBuzz required.

> Background, motivation, and the "why five encodings of *sain* look identical" story is in the [root README](../README.md). This page is the Python install/usage only.

## Install

```sh
pip install mongol-norm
```

This pulls in [`mongol-shape-data`](../mongol-shape-data/) automatically.

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
pytest mongol-norm/tests/
```

## Requirements

- Python 3.6+
- `mongol-shape-data` (installed automatically)

## License

SIL Open Font License 1.1 — consistent with upstream sources (UTN #57, mongfontbuilder).
