# mongol-norm data format

Flat, language-agnostic data for Traditional Mongolian shaping + normalization.

mongol-norm bundles pre-processed JSON in [`mongol_norm/data/`](../mongol_norm/data/) that encodes the **letter × position × FVS → written-unit** mapping plus vowel-harmony categories, shaping conditions, the MVS particle dictionary, and the normalize table — everything a UTN #57 shaper/normalizer needs, minus the algorithm itself.

**Audience:** anyone implementing a Mongolian shaper or normalizer in any language (Python, JS, Dart, Java, C, PHP, …). The JSON has no Python-specific structure — it is generated once (see [Regenerating](#regenerating)) and committed; there is no separate data package to install.

The rules are derived from:
- [UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) — Unicode technical note defining the shaping algorithm.
- [mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) — the machine-readable variant data authored alongside UTN #57.

---

## What's in `mongol_norm/data/`

```
mongol_norm/data/
├── MNG.json            — Hudum (Traditional Mongolian) shape rules
├── MNG.normalize.json  — Hudum normalize table (see below)
├── TOD.json            — Todo
├── SIB.json            — Sibe
└── MCH.json            — Manchu
```

Each `<LOCALE>.json` is self-contained: one JSON document with every piece of data needed to shape that locale. A `<LOCALE>.normalize.json` (currently `MNG` only) additionally supports *normalization* — see [Normalize table](#normalize-table-mngnormalizejson).

Size: shape rules 45–60 KB each; the normalize table ~16 KB.

## Getting the JSON

### Python

The data ships inside `mongol-norm`; the shaper loads it for you. To read it
directly, internal helpers are available:

```python
from mongol_norm._data import load_rules, load_normalize_table
rules = load_rules("MNG")             # -> dict
table = load_normalize_table("MNG")   # -> dict
```

### Any other language

Grab the raw files directly:

- From the released GitHub artifact, or
- From the repo: [`mongol_norm/data/*.json`](../mongol_norm/data/)

Bundle the file with your package. Parse with any JSON library.

---

## JSON schema

### Top level

```json
{
  "schema_version": 1,
  "locale": "MNG",
  "generated_from": { "package": "mongfontbuilder", "version": "0.10.6" },
  "letters": [ ... ],
  "categories": { ... },
  "particles": { ... }
}
```

| field | type | meaning |
|---|---|---|
| `schema_version` | int | Increments on incompatible schema changes. Current: `1`. |
| `locale` | string | One of `MNG`, `TOD`, `SIB`, `MCH`. |
| `generated_from` | object | Provenance — which mongfontbuilder version produced this file. |
| `letters` | array | Per-letter data (see below). |
| `categories` | object | Phonological categories for vowel-harmony and consonant logic. |
| `particles` | object | MVS-headed particles used by step 3 of the pipeline. |

### `letters[]`

```json
{
  "cp": 6176,
  "name": "MONGOLIAN LETTER A",
  "alias": "a",
  "variants": [
    {
      "position": "isol",
      "fvs": 3,
      "written": ["A", "A"],
      "default": true,
      "conditions": [],
      "archaic": false,
      "unrecommended": false
    }
  ]
}
```

| field | type | meaning |
|---|---|---|
| `cp` | int | Unicode codepoint (decimal). E.g. `6176` = U+1820. |
| `name` | string | Unicode character name. |
| `alias` | string\|missing | Short phonemic name for this codepoint in the current locale (`"a"`, `"e"`, `"h"`, …). May be missing when no alias is defined. |
| `variants[]` | array | All shaping variants that apply to this letter in this locale. |

### `letters[].variants[]`

| field | type | meaning |
|---|---|---|
| `position` | string | One of `isol`, `init`, `medi`, `fina`. |
| `fvs` | int | `0` = no FVS; `1..4` = FVS1..FVS4 (Unicode U+180B, U+180C, U+180D, U+180F). |
| `written` | array of string | The sequence of **written units** this variant renders to. Written units are opaque identifiers for glyph atoms (e.g. `"A"`, `"Aa"`, `"Bg"`, `"Ix"`). Your shaper does not need to interpret them — they're compared/concatenated as strings. |
| `default` | bool | `true` if this is the variant used when no FVS/condition applies. Exactly one default per `(cp, position)`. |
| `conditions` | array of string | Named shaping conditions that select this variant. See [Conditions](#conditions) below. |
| `archaic` | bool | Variant is archaic per UTN #57. Skip when computing the reverse map (normalizer). |
| `unrecommended` | bool | Variant is discouraged. Skip when computing the reverse map. |

### `categories`

```json
{
  "vowel":          ["a", "e", "i", "o", "u", "oe", "ue"],
  "consonant":      ["n", "ng", "b", ...],
  "vowelMasculine": ["a", "o", "u"],
  "vowelFeminine":  ["e", "oe", "ue"],
  "vowelNeuter":    ["i"]
}
```

Aliases (as in `letters[].alias`) classified into phonological groups. Used by:

- Step 2 (Syllabic) to assign conditions based on neighboring vowel class.
- Normalizer vowel-harmony detection (masculine vs. feminine words).

### `particles`

```json
{
  "u u":      [0],
  "ue ue":    [0],
  "mvs a ch a": [1]
}
```

- **Key** — space-separated alias sequence starting with `mvs` (or a vowel alias).
- **Value** — list of token indices (0-based) within the segment that should receive `"particle"` condition.

Used by step 3 of the shaping pipeline. Match each MVS-headed segment against the keys; on match, apply `"particle"` condition to the listed indices.

### Conditions

The shape algorithm assigns a `condition` to each token, then looks up which FVS variant for that `(cp, position)` has the condition in its `conditions` list. The condition vocabulary for MNG is:

```
chachlag              — suffix form after MVS
chachlag_onset        — start of a chachlag
chachlag_onset_gb     — GB-specific chachlag onset
onset                 — word-initial consonant
masculine_onset       — onset in masculine-harmony word
devsger               — "connecting tooth" form
masculine_devsger     — devsger in masculine context
vowel_devsger         — i after a vowel (double tooth)
feminine              — feminine-harmony context
marked                — explicitly marked variant
dotless               — unpointed form
particle              — set by step 3 from particle dictionary
post_bowed            — vowel after a bowed consonant (G, B, K, P, F)
```

Other locales may expose a different subset.

---

## Algorithm

This document is **data only**. For the algorithm, see:

- **Spec:** [UTN #57 v4, section 3 ("Shaping")](https://www.unicode.org/notes/tn57/tn57-4.html) — the 5-step Mongolian-specific shaping phase.
- **Reference implementation:** [`mongol_norm/shaper.py`](../mongol_norm/shaper.py) — readable pure Python.

The 5 steps summarized:

1. **Chachlag** — tag letters after MVS with suffix-form conditions.
2. **Syllabic** — assign per-letter conditions from phonological context (vowel class, neighboring consonants, word position).
3. **Particle** — match MVS-headed segments against the `particles` dictionary; tag hit indices with `"particle"`.
4. **Devsger** — `i` in medial position after a vowel gets `"vowel_devsger"` (renders as double tooth).
5. **Post-bowed** — vowels after bowed consonants (G, B, K, P, F) get `"post_bowed"`.

Each tagged token then resolves to a concrete FVS variant by scanning `conditions` lists.

---

## Recommended runtime indexes

The shipped JSON is the source of truth; each port should build its own in-memory indexes. Typical ones:

```
cp_to_alias           cp  -> alias
alias_to_cp           alias -> cp
variant_by_key        (cp, position, fvs) -> variant
default_by_pos        (cp, position) -> variant (default one)
condition_to_fvs      (cp, position, condition) -> fvs
reverse_map           (position, tuple(written)) -> (cp, fvs)
```

The reference Python implementation (`shaper.py`) shows exactly how these are built.

---

## Normalize table (`MNG.normalize.json`)

Alongside the shape rules, `mongol_norm/data/` ships a **normalize table** for locales that support normalization (currently `MNG`). Where the shape rules drive *letter → glyph*, this table drives the reverse used by canonicalization: *written-unit → the one `(letter, FVS)` that renders it independent of context*.

It exists so other languages can implement mongol-norm's `normalize` (same shape → same Unicode) with **only a JSON parser** — no shaping engine, no search. The Python runtime loads this exact file too.

```python
from mongol_norm._data import load_normalize_table
tbl = load_normalize_table("MNG")   # -> dict
```

### Schema

```json
{
  "schema": "mongol-normalize-table/1",
  "locale": "MNG",
  "unit_enc_max_len": 3,
  "constants": { "MVS": "180E", "NIRUGU": "180A", "FVS1": "180B", "...": "..." },
  "velar_fem_units": ["G", "Gx"],
  "masc_to_fem": { "a": "e", "o": "oe", "u": "ue" },
  "unit_table": {
    "isol": { "A": { "letter": "a", "cp": "1820", "fvs": "180B" } },
    "init": { "...": {} }, "medi": { "...": {} }, "fina": { "...": {} }
  },
  "velar_fem": { "fina": { "O": { "letter": "oe", "cp": "1825", "fvs": "180C" } } }
}
```

| field | meaning |
|---|---|
| `unit_table[pos][unit]` | The pinned encoding for a written `unit` at `pos` (`isol`/`init`/`medi`/`fina`). `unit` is a `+`-joined written-unit tuple — single (`"A"`) or multi (`"A+O+I"`). Value: `letter` (alias), `cp` (hex codepoint), `fvs` (hex codepoint or `null`). |
| `unit_enc_max_len` | Longest written-unit tuple in `unit_table`; bounds the multi-unit lookahead during partition. |
| `velar_fem[pos][unit]` | The feminine encoding of a single vowel unit, used by the velar-feminine refinement. |
| `velar_fem_units` | Units that trigger that refinement (`G`, `Gx`). |
| `masc_to_fem` | Masculine→feminine vowel alias map the refinement applies. |
| `constants` | Hex codepoints for MVS / Nirugu / ZWJ / FVS1–4. |
| `ci_probe_letters` | The neighbour letters the selection battery probed (provenance; not needed at runtime). |

`cp`/`fvs` are **hex strings** (`"1820"`, or `null` for no FVS) — parse with base 16.

### Consuming it (the normalize algorithm)

Build a `(pos, tuple(unit.split("+"))) → (cp, fvs)` index, then per word:

1. `shape()` the word (needs the shape rules). Structural characters — MVS, nirugu, ZWJ — appear verbatim in the shape as `mvs`/`nirugu`/`zwj` tokens. Split the shape at these tokens into chains and copy the tokens through unchanged. A letter directly next to a joiner (nirugu/zwj) looks its unit up at the shifted position (e.g. a lone unit between two nirugus is `medi`, not `isol`).
2. For each chain, left-to-right, pick at each position the single unit if the table has it, else the longest multi-unit entry present; emit `cp` (+ `fvs` when non-null).
3. Velar-feminine refinement: for an `init`/`medi` `G`/`Gx`, if the following vowel is a masculine `a`/`o`/`u`, replace it with the `velar_fem` encoding of that unit.
4. Verify by reshaping. The table is total over the reference corpus (FVS-first selection leaves no gap chains); if a shape ever misses the table, return the input unchanged rather than mis-encode.

Full reference: [`mongol_norm/shaper.py`](../mongol_norm/shaper.py) — `_unit_encode_chain`, `_unit_partition`, `_apply_velar_fem`.

---

## Regenerating

The JSON in `mongol_norm/data/` is generated and committed. Run these from the
`mongol-norm` package directory after the relevant upstream/code change.

Shape rules (when bumping `mongfontbuilder`):

```sh
pip install -e ".[preprocess]"
python scripts/preprocess.py           # all locales
python scripts/preprocess.py MNG TOD   # specific
```

The script reads `mongfontbuilder/lib/mongfontbuilder/data/*.json` directly (bypassing cattrs, which would strip the `unrecommended` field from `VariantLocaleData`). Output goes to `mongol_norm/data/`.

Normalize table (after a change to shaping or the selection battery — no extra
dependency, it uses this package's own shaper):

```sh
python scripts/gen_normalize_table.py        # all locales
python scripts/gen_normalize_table.py MNG    # specific
```

Commit the regenerated JSONs along with a changelog note referencing the source version.

## Schema versioning

`schema_version: 1` is the initial schema. Incompatible changes (field removal, type changes, semantic shifts) increment this. Additive changes (new optional fields) do not.

Consumers should check `schema_version` on load and fail loudly on unknown values.

## License

The shaping data is derived from UTN #57 and the mongfontbuilder project. Use under the SIL Open Font License 1.1 — consistent with the upstream sources.
