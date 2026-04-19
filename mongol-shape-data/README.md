# mongol-shape-data

Flat, language-agnostic shaping rules for Traditional Mongolian script.

This package ships pre-processed JSON that encodes the **letter × position × FVS → written-unit** mapping plus vowel-harmony categories, shaping conditions, and the MVS particle dictionary — everything a UTN #57 shaper needs, minus the algorithm itself.

**Audience:** anyone implementing a Mongolian shaper in any language (Python, JS, Dart, Java, C, PHP, …). The JSON has no Python-specific structure.

The rules are derived from:
- [UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) — Unicode technical note defining the shaping algorithm.
- [mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) — the machine-readable variant data authored alongside UTN #57.

---

## What's in the package

```
mongol_shape_data/
└── rules/
    ├── MNG.json   — Hudum (Traditional Mongolian)
    ├── TOD.json   — Todo
    ├── SIB.json   — Sibe
    └── MCH.json   — Manchu
```

Each file is self-contained: one JSON document with every piece of data needed to shape that locale.

Size: 45–60 KB each.

## Getting the JSON

### Python

```sh
pip install mongol-shape-data
```

```python
from mongol_shape_data import load_rules
rules = load_rules("MNG")  # -> dict
```

### Any other language

Grab the raw files directly:

- From the released GitHub artifact, or
- From the repo: [`mongol_shape_data/rules/*.json`](./mongol_shape_data/rules/)

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

This package is **data only**. For the algorithm, see:

- **Spec:** [UTN #57 v4, section 3 ("Shaping")](https://www.unicode.org/notes/tn57/tn57-4.html) — the 5-step Mongolian-specific shaping phase.
- **Reference implementation:** [`mongol-norm/mongol_norm/shaper.py`](../mongol-norm/mongol_norm/shaper.py) in this repo — ~1900 lines of Python, readable.

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

## Regenerating

For maintainers bumping `mongfontbuilder`:

```sh
pip install -e "mongol-shape-data[preprocess]"
python mongol-shape-data/scripts/preprocess.py           # all locales
python mongol-shape-data/scripts/preprocess.py MNG TOD   # specific
```

The script reads `mongfontbuilder/lib/mongfontbuilder/data/*.json` directly (bypassing cattrs, which would strip the `unrecommended` field from `VariantLocaleData`). Output goes to `mongol_shape_data/rules/`.

Commit the regenerated JSONs along with a changelog note referencing the mongfontbuilder version.

## Schema versioning

`schema_version: 1` is the initial schema. Incompatible changes (field removal, type changes, semantic shifts) increment this. Additive changes (new optional fields) do not.

Consumers should check `schema_version` on load and fail loudly on unknown values.

## License

The shaping data is derived from UTN #57 and the mongfontbuilder project. Use under the SIL Open Font License 1.1 — consistent with the upstream sources.
