# mongol-norm data format

Flat, language-agnostic data for Traditional Mongolian shaping + normalization.

mongol-norm bundles pre-processed JSON in [`python/mongol_norm/data/`](../python/mongol_norm/data/) that encodes the **letter × position × FVS → written-unit** mapping plus vowel-harmony categories, shaping conditions, the MVS particle dictionary, and the normalize table — everything a UTN #57 shaper/normalizer needs, minus the algorithm itself.

**Audience:** anyone implementing a Mongolian shaper or normalizer in any language (JS, Dart, Java, C, PHP, …; Rust and Python are covered by the crate and its bindings). The JSON has no language-specific structure — it is generated once (see [Regenerating](#regenerating)) and committed; there is no separate data package to install.

**How mongol-norm itself uses it:** the runtime — the Rust crate at the repository root ([`src/`](../src/)), which the Python package wraps — does not read this JSON. `python/scripts/gen_rust_tables.py` compiles it into static Rust tables (`src/generated/`); the JSON is the input of that generator and of the other tooling (`python/scripts/gen_normalize_table.py` writes `MNG.normalize.json`, the tests read the files through `mongol_norm._data`). The wheel still ships the files for that tooling.

The rules are derived from:
- [UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) — Unicode technical note defining the shaping algorithm.
- [mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) — the machine-readable variant data authored alongside UTN #57.

---

## What's in `python/mongol_norm/data/`

```
python/mongol_norm/data/
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

The files ship inside the `mongol-norm` wheel for tooling — the shaper itself does not
read them (its tables are compiled in). To read them directly, the internal loaders the
scripts and tests use are available:

```python
from mongol_norm._data import load_rules, load_normalize_table
rules = load_rules("MNG")             # -> dict
table = load_normalize_table("MNG")   # -> dict
```

To canonicalize a written-unit sequence without first supplying nominal Unicode,
use the public API instead of the internal data loaders:

```python
from mongol_norm import MongolianShaper

shaper = MongolianShaper(locale="MNG")
shaper.normalize_written_units(["B", "Aa"])
# -> "ᠪᠠ᠋"

shaper.normalize_written_units(["S", "A", "I", "I", "N", "Mvs", "Aa"])
```

The input must be an ordered `Sequence[str]` using the same unit vocabulary
returned by `shape()`. Every written-unit name is PascalCase; structural controls
are `Mvs`, `Nirugu`, and `Zwj`. The nine duplicate encodings (see the normalize
algorithm below) are still *accepted* here and unified before encoding, so data
captured from an older `shape()` keeps working; they simply never come back out of
`shape()`. Old lowercase and all-uppercase control aliases
are rejected. Positions are inferred from order
and structural context—the API does not accept explicit position records and
never infers or inserts a structural control. In particular, output contains ZWJ
only when the request contains `Zwj`. An empty sequence returns an empty string.
Malformed outer input or a non-string item raises `TypeError`; unknown units and
sequences that cannot reshape to the exact requested units raise `ValueError`.
There is no partial-output or first-candidate fallback.

For callers that carry authoritative HUD written-unit positions, use the record
API:

```python
shaper.normalize_positioned_written_units([
    {"unit": "B", "position": "init"},
    {"unit": "Aa", "position": "fina"},
])
```

Every item must be a built-in dict containing exactly the string fields `unit`
and `position`. `position` is the written unit's position in the authoritative HUD
inventory, not the emitted Unicode letter's joining topology. Resolved variant
references preserve that distinction: isolated Unicode FA borrows `F:init`, so a
lone `F:init` record encodes as bare `U+1839`; `F:isol` is not a valid pair and is
rejected. Letter positions are `isol`, `init`, `medi`, or `fina`; structural units
`Mvs` and `Nirugu` require `control`. Explicit `Zwj` input is rejected, but the
encoder may insert ZWJ in its Unicode output when a valid HUD position needs
joining context. Borrowed forms with a bare candidate remain bare; temporarily,
`I:isol` and `I:init` both use the plain `I` canonical without inserted ZWJ. The
sole singleton-initial exception is `O:init`: it reuses the U+1824 U+180B prefix
selected by canonical `O:init, A:fina`, then adds the trailing U+200D. The
generator combines unchanged plain shaping
traces with source `positioned_written` metadata to verify exact positions and
MVS-boundary alternatives. The normalizer consults that inventory (compiled into the
Rust tables); public `shape()` remains plain and contains no position records.
A wrong outer/record/field type raises `TypeError`; wrong keys, unit, position,
structural context, exact encoding, or more than 1024 records raises `ValueError`.
This word-level API currently has no CLI subcommand.

The CLI equivalent of the plain `normalize_written_units()` API accepts either
uniquely segmented compact
PascalCase units or explicit `+` separators:

```sh
mongol-norm normalize-written-units 'B+Aa'
mongol-norm normalize-written-units 'BZwj'
echo 'B+Aa' | mongol-norm normalize-written-units -
mongol-norm normalize-written-units --batch -i units.txt -o canonical.txt
```

Batch input contains one compact or `+`-joined sequence per line. Compact input
fails closed if it has more than one valid segmentation; use `+` to resolve the
boundary. For example, `AAaBZwj` segments as `A+Aa+B+Zwj`, then undergoes the
same exact-shape encodability validation as sequence input. Unit names cannot be
empty or contain surrounding whitespace.

### Rust (the engine)

The Rust crate at the repository root ([`src/`](../src/)) — the engine the Python
package runs — does not read the JSON at runtime. `python/scripts/gen_rust_tables.py` turns these
files into static Rust tables (`src/generated/*.rs`: the `WrittenUnit` /
`Condition` / `Alias` enums, the per-locale shaping tables and the MNG normalize table), and
`python/tests/test_rust_twin.py` fails the Python suite whenever the committed tables are stale:

```sh
python python/scripts/gen_rust_tables.py          # regenerate after changing the JSON
python python/scripts/gen_rust_tables.py --check  # what CI runs
```

### Any other language

Grab the raw files directly:

- From the wheel/sdist on PyPI (`mongol_norm/data/`), or
- From the repo: [`python/mongol_norm/data/*.json`](../python/mongol_norm/data/)

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
      "positioned_written": [
        {"unit": "A", "position": "init"},
        {"unit": "A", "position": "fina"}
      ],
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
| `positioned_written` | array of record\|missing | For MNG normalization, the same resolved units with their authoritative HUD positions preserved. References retain the referenced position, so FA `isol` has `[{"unit":"F","position":"init"}]`, not a fabricated `F:isol`. Other locales currently omit this normalization-only field. |
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
- **Reference implementation:** [`src/`](../src/) — dependency-free Rust: `token.rs` (tokenization, structural positions), `rules.rs` (the five phases, one function per rule), `shaper.rs` (variant resolution; `shape` / `same_shape` / `shape_detailed` / `trace`), `normalize.rs` (the normalize algorithm), `written_units.rs` (the written-unit input APIs). The Python package calls exactly this code through the binding crate in `python/`.

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

The reference implementation builds them in two steps: `python/scripts/gen_rust_tables.py` derives the flat tables from the JSON at generation time, and `src/shaper.rs` indexes them when a `Shaper` is created — read both to see exactly how.

---

## Normalize table (`MNG.normalize.json`)

Alongside the shape rules, `python/mongol_norm/data/` ships a **normalize table** for locales that support normalization (currently `MNG`). Where the shape rules drive *letter → glyph*, this table drives the reverse used by canonicalization: *written-unit → the one `(letter, FVS)` that renders it independent of context*.

It exists so other languages can implement mongol-norm's `normalize` for covered written-unit chains (same supported shape → same Unicode) with **only a JSON parser** — no shaping engine, no search. mongol-norm's own engine consumes this exact file too, compiled into `src/generated/mng_normalize.rs` by `python/scripts/gen_rust_tables.py`. An uncovered chain is outside this table contract; the Python API raises `NormalizationFallbackError` by default and preserves the input only when called explicitly with `strict=False` (the Rust API: `Error::NormalizationFallback`, or `normalize_allow_fallback`).

```python
from mongol_norm._data import load_normalize_table
tbl = load_normalize_table("MNG")   # -> dict
```

### Schema

```json
{
  "schema": "mongol-normalize-table/1",
  "canonical_version": "mng-canonical/2",
  "locale": "MNG",
  "unit_enc_max_len": 3,
  "positioned_units": [
    {"unit": "F", "position": "init"},
    {"unit": "I", "position": "isol"}
  ],
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
| `canonical_version` | Version of the exact shape → canonical Unicode selection policy. Persist this alongside normalized index keys; a changed value means stored keys may need rebuilding. |
| `unit_table[pos][unit]` | The pinned encoding for a written `unit` at `pos` (`isol`/`init`/`medi`/`fina`). `unit` is a `+`-joined written-unit tuple — single (`"A"`) or multi (`"A+O+I"`). Value: `letter` (alias), `cp` (hex codepoint), `fvs` (hex codepoint or `null`). |
| `unit_enc_max_len` | Longest written-unit tuple in `unit_table`; bounds the multi-unit lookahead during partition. |
| `positioned_units` | Complete valid HUD `(unit, position)` inventory used to validate positioned requests. Encoding reuses `unit_table` through `normalize_written_units()`; incomplete chain edges are represented by implicit ZWJ. |
| `velar_fem[pos][unit]` | The feminine encoding of a single vowel unit, used by the velar-feminine refinement. |
| `velar_fem_units` | Units that trigger that refinement (`G`, `Gx`). |
| `masc_to_fem` | Masculine→feminine vowel alias map the refinement applies. |
| `constants` | Hex codepoints for MVS / Nirugu / ZWJ / FVS1–4. |
| `ci_probe_letters` | The neighbour letters the selection battery probed (provenance; not needed at runtime). |

`cp`/`fvs` are **hex strings** (`"1820"`, or `null` for no FVS) — parse with base 16.

### Consuming it (the normalize algorithm)

Build a `(pos, tuple(unit.split("+"))) → (cp, fvs)` index, then per word:

1. `shape()` the word (needs the shape rules). Structural characters — MVS, nirugu, ZWJ — appear verbatim in the shape as PascalCase `Mvs`/`Nirugu`/`Zwj` tokens. Split the shape at these tokens into chains and copy the tokens through unchanged. A letter directly next to a joiner (`Nirugu`/`Zwj`) looks its unit up at the shifted position (e.g. a lone unit between two nirugus is `medi`, not `isol`).
1a. **Unify the duplicate encodings.** Nine written units render as exactly the same ink as a sequence of other units, so a port that leaves them in will produce two canonical texts for one visible word (ᠠᠷᠠᠳ vs ᠠᠷᠠᠤᠠ). Positions are the chain slots of step 1 — a nirugu/ZWJ neighbour pads the chain, so a unit next to one can be final even though something precedes it.

   First **expand**, in one left-to-right pass over each chain:

   | unit | position | replace with |
   |---|---|---|
   | `Dd` | `medi`, `fina` (its only positions) | `O A` |
   | `H`  | `medi` | `A A` |
   | `Hx` | `medi` | `N N` |
   | `Cr` | `init` | `O O` |

   Then inspect the final adjacent pair **once** and contract only in its verified context. The shorter form is canonical; this is not an unconditional fixed-point rewrite. Here *position* is the position the merged unit takes in the shortened chain:

   | pair | merged position | replace with |
   |---|---|---|
   | `A Aa` | `isol` | `A` |
   | `A Aa`, immediately preceded by a bowed written unit in the same chain | `fina` | `Aa` |
   | `O Aa` | `fina` | `B2` |
   | `I Aa` | `fina` | `G` |

   The complete Hudum bowed written unit set is `B P F G Gx K K2`, from the [Hudum ligated variants](https://mongfontbuilder.pages.dev/hudum/) and [upstream required ligatures](https://github.com/Kushim-Jiang/mongfontbuilder/blob/7d5fc1cdaf8210f675c16699a8eaeb71aa1e80ca/data/ligatures.ts), also reflected in `src/rules.rs`'s post-bowed classes. `Aa:fina` has a tooth immediately after a bowed written unit, but no tooth otherwise. Thus `B A Aa → B Aa`, while `N A Aa`, `A A Aa`, and `B A A Aa` remain intact. Joiner padding supplies position, not a bowed written unit; never match across structural tokens. The independent whole-chain `A:init Aa:fina → A:isol` rule stays.

   Initial/final `H`/`Hx`, a lone `Cr:isol`, and a lone `A` are unchanged. One expansion pass suffices: neither direction emits expansion targets or exposes initial/final `H`/`Hx` as medial. All contractions end the chain; `A/B2/G` cannot contract again, and a contracted `Aa` follows a bowed written unit rather than `A/O/I`. This proves idempotence without repeated deletion. In particular `Dd Aa → O A Aa` cannot then contract: `O` is not a bowed written unit. See the 168 421-input exhaustion and context regressions in `src/duplicates.rs`.

   UTN #57 and GB/T 25914-2023 keep all nine as distinct units — their EAC vectors spell ᠠᠷᠭᠠᠯ `A A R Hx A L` — so a *shaping* conformance test must compare against the pre-unification sequence (mongol-norm exposes it as the non-public `Shaper::shape_raw`). Reference: [`src/duplicates.rs`](../src/duplicates.rs).
2. For each chain, left-to-right, pick at each position the single unit if the table has it, else the longest multi-unit entry present; emit `cp` (+ `fvs` when non-null).
3. Velar-feminine refinement: for an `init`/`medi` `G`/`Gx`, if the following vowel is a masculine `a`/`o`/`u`, replace it with the `velar_fem` encoding of that unit.
4. Verify by reshaping. The table is total over the reference corpus (FVS-first selection leaves no gap chains); if a shape ever misses the table, fail closed (raise), or return the input unchanged only when the caller opted in (`strict=False` / `--allow-fallback`); never mis-encode.

Full reference: [`src/normalize.rs`](../src/normalize.rs) — `canonical_for_shape`, `unit_encode_chain`, `unit_partition`, `apply_velar_fem`.

---

## Regenerating

The JSON in `python/mongol_norm/data/` is generated and committed. The scripts live in
`python/scripts/` but locate the repository from their own path, so run them **from the
repository root**, in a virtualenv where the extension is built (`cd python && pip install
'maturin>=1.15,<2' && maturin develop --locked --features testing`, see the README), after
the relevant upstream/code change.

Shape rules (when bumping `mongfontbuilder`):

```sh
pip install 'mongfontbuilder>=0.10.6'          # the [preprocess] extra
python python/scripts/preprocess.py            # all locales
python python/scripts/preprocess.py MNG TOD    # specific
```

The script reads `mongfontbuilder/lib/mongfontbuilder/data/*.json` directly (bypassing cattrs, which would strip the `unrecommended` field from `VariantLocaleData`). Output goes to `python/mongol_norm/data/`.

Normalize table (after a change to shaping or the selection battery — no extra
dependency, it drives the package's own shaper, i.e. the compiled engine, so rebuild the
extension first when the shaping rules changed):

```sh
python python/scripts/gen_normalize_table.py        # all locales
python python/scripts/gen_normalize_table.py MNG    # specific
```

Rust tables (after any JSON change — regenerate them, rebuild the extension with
`maturin develop --locked --features testing` from `python/`, then run `cargo test
--workspace` from the root). Mind the loop: the engine's tables come from the JSON and the
normalize table comes from the engine, so a shape-rule change is `gen_rust_tables.py` →
`maturin develop` → `gen_normalize_table.py` → `gen_rust_tables.py` again (the normalize
table is compiled in too) → `maturin develop`:

```sh
python python/scripts/gen_rust_tables.py            # regenerate src/generated/
python python/scripts/gen_rust_tables.py --check    # CI freshness check
```

Compatibility fixtures (`tests/golden/`, after an intentional shaping or canonical change):

```sh
python python/scripts/gen_compat_goldens.py          # regenerate both fixtures
python python/scripts/gen_compat_goldens.py --check  # CI freshness check
```

Commit the regenerated JSONs along with a changelog note referencing the source version.

## Schema versioning

`schema_version: 1` is the initial schema. Incompatible changes (field removal, type changes, semantic shifts) increment this. Additive changes (new optional fields) do not.

Consumers should check `schema_version` on load and fail loudly on unknown values.

## License

The shaping data is derived from UTN #57 and the mongfontbuilder project. Use under the SIL Open Font License 1.1 — consistent with the upstream sources.
