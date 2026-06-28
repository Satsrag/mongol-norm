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

### Python API

```python
from mongol_norm import MongolianShaper

shaper = MongolianShaper(locale="MNG")  # Hudum Traditional Mongolian

# Shape: get written-unit sequence
shaper.shape("ᠰᠠᠢᠨ")
# → ['S', 'A', 'I', 'I', 'A']

# Compare: are two encodings visually identical?
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")
# → True

# Normalize: canonical Unicode (same shape ⟹ same Unicode)
shaper.normalize("ᠰᠡᠢᠨ")
# → 'ᠰᠠᠢᠠ'

# Full-text (per-word normalization, preserves non-Mongolian)
shaper.normalize_text("Hello ᠰᠡᠢᠨ world")
# → 'Hello ᠰᠠᠢᠠ world'
```

### CLI

After `pip install ./mongol-norm`, the `mongol-norm` command is on `PATH`.
Without installing: `python -m mongol_norm.shaper ...`.

```bash
# Inline text
mongol-norm shape 'ᠰᠠᠢᠨ'                  # → S+A+I+I+A
mongol-norm normalize 'ᠰᠡᠢᠨ'              # → ᠰᠠᠢᠠ
mongol-norm normalize-text 'Hello ᠰᠡᠢᠨ'    # mixed-script

# Pipe / stdin (use `-` as the text)
echo 'ᠰᠡᠢᠨ' | mongol-norm normalize -
cat doc.txt | mongol-norm normalize-text -

# File in / out
mongol-norm normalize-text -i in.txt -o out.txt

# Batch: one word per line in, one canonical per line out
mongol-norm normalize --batch -i words.txt -o canonical.txt
cat words.txt | mongol-norm shape --batch -    # one shape per line

# Visual-identity check (exit 0 if same, 1 if different)
mongol-norm same 'ᠰᠠᠢᠨ' 'ᠰᠡᠢᠨ'             # → true (exit 0)
```

**Tip:** `normalize` (single-word) skips non-Mongolian chars including
newlines, so a multi-line file fed to plain `normalize` is treated as
one giant concatenated word (slow and meaningless). Use `--batch` for
one-word-per-line files, or `normalize-text` for free-form text.

## How `normalize` works

`normalize` is a **pure function of shape**: any two encodings that shape
identically produce the same Unicode output, and the output always
round-trips — `shape(normalize(x)) == shape(x)`. It is also **prefix-stable**
(defined below). When these goals conflict the priority is
**round-trip > prefix-stable > shortest**.

Pipeline, per word:

1. **shape** the input into its written-unit sequence (the glyph forms).
2. **split** the shape at every MVS into *chains*.
3. **encode each chain**, right-to-left (so appending a suffix can't disturb
   the encoding of what precedes it), threading the already-encoded suffix as
   verification context:
   1. **partition + table lookup** — the primary path. Walk the chain
      left-to-right; at each position take the longest *required-multi* unit,
      else the single unit, else the longest available multi-unit, and look up
      `(position, written-unit) → (letter, FVS)` in the FVS-pinned table. Every
      value renders its unit **regardless of neighbours**, so this is a
      deterministic, O(N), prefix-stable function of the shape.
   2. **velar-feminine refinement** — a `G`/`Gx` velar's forward-coupled vowel
      (the following `a`/`o`/`u`) is swapped to its feminine partner
      (`e`/`oe`/`ue`), because a velar syllable written with the masculine
      vowel is shape-correct but ugly. Only forward coupling is applied
      (init/medi velar → following vowel); backward coupling would flip when a
      suffix is appended and break prefix-stability.
   3. **verify** — shape the candidate in full context (leading MVS + encoded
      suffix) and accept only if it equals the target chain shape.
   4. **gap chains** — a few chains can't be expressed by any per-unit letter:
      isolated nirugu-only units (`O`, `J`, `Dd`, `Ue`, …, which need a nirugu
      to suppress the leading tooth) and bowed-consonant + final-vowel finals.
      These fall back to an exhaustive structural search that may wrap a unit
      in nirugu. ~0.5% of corpus chains; the table covers the rest.
4. **particle substitution** post-pass — pin isolate `I` to `i+FVS1`, rewrite
   `MVS + bare-particle` to `MVS + particle+FVS` (so the form renders the same
   with or without the MVS), excluding chachlag.

**Prefix-stability**: if word *A* = word *B* + suffix and their shapes agree on
the shared prefix, the shared region encodes identically except the single
boundary unit whose position changes (final in *B* → medial in *A*). The
per-unit table delivers this because each unit's encoding depends only on its
own position, never on its neighbours.

### The selection method (how the table is built)

Each `(position, written-unit)` slot is filled by a **context-independence
battery**: candidate `(letter, FVS)` encodings are tried masculine-first and
bare-first, and the first one that renders *exactly* that written unit in
*every* probed neighbour context is pinned. This runs offline — see
`MongolianShaper.compute_normalize_tables()`.

### Portable table (other languages)

The whole table is exported as language-agnostic JSON, so a port needs only a
JSON parser plus the partition algorithm above — no shaping engine:

```
mongol-shape-data/mongol_shape_data/rules/MNG.normalize.json
```

The Python runtime loads this same file at startup (and falls back to running
the battery if the data package ships no spec). Schema and the consuming
algorithm are documented in the
[mongol-shape-data README](../mongol-shape-data/README.md#normalize-table-mngnormalizejson).
Regenerate with:

```sh
pip install -e ./mongol-norm
python mongol-shape-data/scripts/gen_normalize_table.py
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

# Run all tests (hand-written + round-trip/canonicity/prefix-stability
# + normalize-table export + 225 core-hud + 3513 eac-hud)
cd mongol-norm
python -m unittest tests.test_shaper tests.test_round_trip \
    tests.test_normalize_table tests.test_core_hud tests.test_eac_hud
```

## Requirements

- Python 3.9+ (CI matrix: 3.9 – 3.13)
- `mongol-shape-data` (installed automatically)

## License

SIL Open Font License 1.1 — consistent with upstream sources (UTN #57, mongfontbuilder).
