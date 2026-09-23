# mongol-norm data format

Flat, language-agnostic data for Traditional Mongolian shaping + normalization.

mongol-norm bundles pre-processed JSON in [`python/mongol_norm/data/`](../python/mongol_norm/data/) that encodes the **letter × position × FVS → written-unit** mapping plus vowel-harmony categories, shaping conditions, the MVS particle dictionary, and the normalize table — everything a UTN #57 shaper/normalizer needs, minus the algorithm itself.

**Audience:** anyone implementing a Mongolian shaper or normalizer in any language (JS, Dart, Java, C, PHP, …; Rust and Python are covered by the crate and its bindings). The JSON has no language-specific structure — it is generated once (see [Regenerating](#regenerating)) and committed; there is no separate data package to install.

**How mongol-norm itself uses it:** the runtime — the Rust crate at the repository root ([`src/`](../src/)), which the Python package wraps — does not read this JSON. `python/scripts/gen_rust_tables.py` compiles it into static Rust tables (`src/generated/`); the JSON is the input of that generator and of the other tooling (the Rust example `examples/gen_normalize_table`, run by `python/scripts/gen_normalize_table.py`, writes `MNG.normalize.json` from `MNG.json`; the tests read the files through `mongol_norm._data`). The wheel still ships the files for that tooling.

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

Size: shape rules 45–60 KB each; the normalize table ~95 KB.

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
# -> "ᠪᠠ"

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
joining context. Borrowed forms with a bare candidate remain bare: a lone `init`
record whose bare letter reads back as that record — every consonant, whose isolated
form borrows the initial unit — encodes without ZWJ. A lone `init` record whose bare
letter would read back as another record takes a trailing U+200D: `A:init` and
`I:init`, whose bare spelling is also that of `A:isol` / `I:isol` (which stay bare),
and `O:init`, which has no bare spelling. The
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
- **Reference implementation:** [`src/`](../src/) — dependency-free Rust: `token.rs` (tokenization, structural positions), `rules.rs` (the five phases, one function per rule), `shaper.rs` (variant resolution; `shape` / `same_shape` / `shape_detailed` / `trace`), `normalize.rs` and `encoder.rs` (the normalize algorithm), `written_units.rs` (the written-unit input APIs). The Python package calls exactly this code through the binding crate in `python/`.

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

Alongside the shape rules, `python/mongol_norm/data/` ships a **normalize table** for locales that support normalization (currently `MNG`). Where the shape rules drive *letter → glyph*, this table drives the reverse: the tables of mongol-norm's **online normalize encoder** (policy `mng-canonical/3`; the design is in [`docs/internals.md`](internals.md), "Normalization strategy"). For every `(position, written units)` it lists the letters that render them in preference order, says when a context-dependent letter may be *committed* — written before the rest of the word is known — and which letters can end a word.

With this file, the shape rules and the algorithm below, a port reproduces mongol-norm's `normalize` byte for byte: the shaper computes the input's shape, the encoder itself never shapes and never searches beyond a few units. mongol-norm's own engine consumes this exact file, compiled into `src/generated/mng_normalize.rs` by `python/scripts/gen_rust_tables.py`. A shape the tables cannot encode is outside this contract; the Python API raises `NormalizationFallbackError` by default and preserves the input only when called explicitly with `strict=False` (the Rust API: `Error::NormalizationFallback`, or `normalize_allow_fallback`). No reachable shape is known to hit this.

```python
from mongol_norm._data import load_normalize_table
tbl = load_normalize_table("MNG")   # -> dict
```

### Schema

```json
{
  "schema": "mongol-normalize-table/2",
  "canonical_version": "mng-canonical/3",
  "locale": "MNG",
  "constants": { "MVS": "180E", "NIRUGU": "180A", "ZWJ": "200D", "FVS1": "180B", "...": "..." },
  "context_components": [["prev_letter", 6], ["prev_fvs", 3], "..."],
  "promises": [["vowel", ["a", "e", "i", "o", "u", "oe", "ue", "ee"]], "..."],
  "particle_nodes": ["", "M:", "u", "u u", "...", "M:y i n", "..."],
  "written_sequences": ["A", "A+A", "A+G", "..."],
  "mask_units": ["A", "Aa", "Ah", "..."],
  "mask_sets": [["100000000000000", "0", "0", "0", "0", "0"], "..."],
  "candidates": [
    { "letter": "i", "cp": "1822", "fvs": null, "position": "medi", "units": "I", "written": "I",
      "robust": { "projection": ["prev_letter", "prev_written"], "default": 2, "rows": [["101", 1], "..."] } },
    { "letter": "i", "cp": "1822", "fvs": "180D", "position": "medi", "units": "I", "written": "I",
      "robust": "always" }
  ],
  "finals": [
    { "position": "fina", "units": "N", "valid": "2" },
    { "position": "isol", "units": "A+O",
      "valid": { "projection": ["prev_token"], "default": "1f", "rows": [["2", "1d"]] } }
  ],
  "known_units": ["A", "Aa", "..."],
  "positioned_units": [{ "unit": "F", "position": "init" }, { "unit": "I", "position": "isol" }]
}
```

| field | meaning |
|---|---|
| `canonical_version` | Version of the exact shape → canonical Unicode selection policy. Persist this alongside normalized index keys; a changed value means stored keys may need rebuilding. |
| `constants` | Hex code points of MVS / nirugu / ZWJ / FVS1–4. |
| `context_components` | `[name, bits]` of the encoder context's components, in key-packing order ([The context](#the-context)). |
| `promises` | Promise classes 1–5, in order: `[name, letter aliases]`. Promise 0 is "none". |
| `particle_nodes` | The particle-key trie ([The context](#the-context)). Node 0 (`""`) is the word start, node 1 (`"M:"`) the start of a segment after an MVS. Every other node is named by its parent's name plus its letter's alias (`"M:y i"` is the child of `"M:y"` by `i`; `"u"` the child of `""` by `u`, `"M:u"` of `"M:"`). |
| `written_sequences` | Every candidate's `written`, sorted; the context's `prev_written` is a 1-based index into it. |
| `mask_units` | The written units in the bit order of a next-unit mask: bit `i` is `mask_units[i]` (the `WrittenUnit` order, structural tokens included). |
| `mask_sets` | A pool of robustness answers: six hex masks each, one per promise 0–5. A set bit means "may be committed before this next unit, making this promise". |
| `candidates` | Every encoding option: `letter` (alias), `cp`, `fvs` (hex code point, or `null` for a bare letter), `position` (`isol`/`init`/`medi`/`fina`), `units` (the `+`-joined written units it renders; `written` is the same sequence as the engine names it), and `robust`: `"always"` (may be committed in every context, without a promise), `"never"`, or a [context table](#context-tables) whose values index `mask_sets`. The options of one `(position, units)` group are contiguous and in the base preference order ([Preference](#preference)). |
| `finals` | Which options of an `isol`/`fina` group can end a word: `valid`, a hex mask over the group's options (bit `i` is the group's `i`-th candidate), or a context table of such masks. A group missing here never ends a word. |
| `known_units` | Every written unit the locale's letters render, the nine duplicate encodings included; with `Mvs`, `Nirugu` and `Zwj` it is the vocabulary of `normalize_written_units()`. |
| `positioned_units` | Complete valid HUD `(unit, position)` inventory used to validate positioned requests (`normalize_positioned_written_units()`, which encodes through `normalize_written_units()`; incomplete chain edges are represented by implicit ZWJ). |

`cp`, `fvs`, masks and row keys are **hex strings** — parse with base 16.

#### Context tables

`{"projection": [component names], "default": value, "rows": [[key, value], …]}` gives a value per context. The key of a context under a projection concatenates the projected components' values in `context_components` order, each in its bit width (`key = (key << bits) | value`), written in hex. The value is the row with that key (rows are sorted by key), or `default`. An empty projection is a constant.

### The context

The encoder keeps a context of the text committed so far. After each committed letter or structural token it holds:

| component | value |
|---|---|
| `prev_letter` | The last committed letter (structural tokens skipped): `cp − 0x1820 + 1`; `0` before the first. |
| `prev_fvs` | Its FVS: `1`–`4`, `0` if bare. |
| `prev_position` | Its position: `isol` 1, `init` 2, `medi` 3, `fina` 4. |
| `prev_written` | Its `written`, as a 1-based index into `written_sequences`. |
| `prev_token` | The last committed token: `0` none, `1` letter, `2` MVS, `3` nirugu, `4` ZWJ. |
| `mvs_since_prev` | `1` if an MVS came after the last letter. |
| `cluster` | `1` if the letters since the last vowel are an initial consonant followed by one or more medial consonants (III.2a cluster `marked`). |
| `masculine` | `1` if the nearest vowel back in the MVS segment is `a`, `o` or `u` in initial or medial position (III.2f). |
| `particle` | The particle-trie node the MVS segment has reached, or `1023` once it can match no key (III.3). |
| `promise` | The promise the last letter made (`0`–`5`). |

It starts with every component `0` (the particle node is the word start). Committing candidate `c` with promise `p`:

- `cluster`: a vowel breaks the cluster; a consonant at `init` starts one; a consonant at `medi` extends a started one to "initial + medial" (the component is `1` only then); anything else breaks it.
- `masculine`: `e oe ue ee` clear it; `a o u` at `init`/`medi` set it; other letters keep it.
- `particle`: a bare letter moves to the child node by its letter, or to `1023` if there is none; a letter with an FVS goes to `1023`; `1023` stays.
- `prev_*` describe `c`; `prev_token` = letter; `mvs_since_prev` = 0; `promise` = `p`.

A structural token sets `promise` = 0 and `prev_token` to the token; an MVS also sets `mvs_since_prev` = 1, breaks the cluster, clears `masculine` and sets `particle` to node 1.

The encoder also tracks a **harmony**, which is not a key component: `a o u` make it masculine, `e oe ue ee` feminine, other letters and all tokens keep it. It only orders equally short letters.

**Positions.** The next letter is *chain-first* when `prev_token` is none or MVS. A letter committed with more letters of its chain to follow is `init` if chain-first, else `medi`. The last letter before a structural token is `isol`/`fina` before an MVS and `init`/`medi` before a nirugu or ZWJ (chain-first / not). The word's last letter is `isol`/`fina`.

### Preference

The options of a group are tried in this order: without an FVS first, then `n` first where it applies, then by rank, then by FVS (1–4). **Rank** is `cp × 2`, except that `g` (`182D`) ranks `182C × 2 − 1`, just before `h`; when the harmony is feminine, `e`, `oe`, `ue` rank one below `a`, `o`, `u`. **`n` first** applies to the option `n` rendering `A` when the letter ends its chain right after a vowel — the previous token is a letter and that letter a vowel — and only for the word's final letter and for the last letter before a structural token. With masculine or no harmony and no `n`-first, this is the order of `candidates`.

### Lookups

- **allows(p, letter)**: `p` is 0, or the letter is in class `p`.
- **feasible(p, u)** — may promise `p` > 0 be made before next unit `u`? Yes when some candidate renders exactly `[u]` at `medi` or `fina`, and at each of those two positions where one does, one whose letter is in class `p` does too.
- **robust(ctx, c, next, p)** — may `c` be committed before `next` (the unit after its span, or the structural token that closes it) with promise `p`? `"always"`: iff `p` is 0. `"never"`: no. A table: bit `next` of `mask_sets[value][p]`.
- **final(ctx, units)** — the letter that ends the word with `units`: the position is `isol` if chain-first, else `fina`; take the group's `valid` mask (none: no final letter), and return the first option in preference order whose bit is set and whose letter `allows(ctx.promise)`.
- **cost** — 1 for a bare letter, 2 with an FVS.

### Consuming it (the normalize algorithm)

Per word:

1. `shape()` the word (needs the shape rules). Structural characters — MVS, nirugu, ZWJ — appear verbatim in the shape as PascalCase `Mvs`/`Nirugu`/`Zwj` tokens; they are copied through to the output unchanged.
2. **Unify the duplicate encodings.** Nine written units render as exactly the same ink as a sequence of other units, so a port that leaves them in will produce two canonical texts for one visible word (ᠠᠷᠠᠳ vs ᠠᠷᠠᠤᠠ). Positions are chain slots — a chain is the letters between structural tokens, and a nirugu/ZWJ neighbour pads the chain, so a unit next to one can be final even though something precedes it.

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
3. Encode the unified shape left to right. Every letter but the last is committed — appended to the output and never revised — which is what makes the encoding prefix-stable:

   ```text
   encode(shape):
     out = "", ctx = start, covered = 0
     for end = 1 .. len(shape):
       u = shape[end - 1]
       if u is structural:
         steps = plan(ctx, shape[covered : end - 1], token = u)   # none: the shape is uncovered
         commit steps; out += u; update ctx with u; covered = end
       else if final(ctx, shape[covered : end]) exists:
         wait                                   # one final letter can still end the word
       else if plan(ctx, shape[covered : end], token = none) succeeds:
         commit its steps; covered += the units they span
       else:
         wait                                   # not a prefix of any shape yet
     if covered < len(shape):
       out += final(ctx, shape[covered :])      # none: the shape is uncovered
     return out
   ```

   Committing a step appends its letter (`cp`, then `fvs` if not null) and updates the context with its promise. `plan` finds the cheapest way — total cost, the final letter included — to commit letters from the front of `pending` so that the rest is one final letter or, before a token, so that nothing is left:

   ```text
   plan(ctx, pending, token):
     if pending is empty: return (cost 0, no steps) if token else none
     best = (cost(final(ctx, pending)), no steps) if no token and that final exists
     for span = min(3, len(pending)) down to 1:
       closing = token if span == len(pending) else none
       if span == len(pending) and no token: continue       # the last letter stays pending
       position = the close position of token if closing, else init / medi
       next = closing if closing else pending[span]
       for option in group(position, pending[: span]), in preference order:
         if not allows(ctx.promise, option.letter): continue
         if best and cost(option) >= best.cost: break
         for p in (0 .. 5 if option is bare and not closing, else 0):
           if p > 0 and not feasible(p, next): continue
           if not robust(ctx, option, next, p): continue
           sub = plan(ctx updated with option and p, pending[span :], token)
           if sub:
             if not best or cost(option) + sub.cost < best.cost:
               best = (cost(option) + sub.cost, [option with p] + sub.steps)
             next span                                    # the cheapest safe option of this span
     return best
   ```

   A missing group skips the span. The final letter tried for `best` and the `n`-first preference see the context the plan has reached.

Reference implementation: [`src/encoder.rs`](../src/encoder.rs) (`encode`, `plan`, the context) and [`src/normalize.rs`](../src/normalize.rs) (the table indexes, `preference`, the entry points). The tables are generated by [`examples/gen_normalize_table`](../examples/gen_normalize_table/) — see [Regenerating](#regenerating).

---

## Regenerating

The JSON in `python/mongol_norm/data/` is generated and committed. The scripts live in
`python/scripts/` but locate the repository from their own path, so run them **from the
repository root**, in a virtualenv where the extension is built (`cd python && pip install
'maturin>=1.15,<2' && maturin develop --locked --features testing`, see `docs/development.md`), after
the relevant upstream/code change.

Shape rules (when bumping `mongfontbuilder`):

```sh
pip install 'mongfontbuilder>=0.10.6'          # the [preprocess] extra
python python/scripts/preprocess.py            # all locales
python python/scripts/preprocess.py MNG TOD    # specific
```

The script reads `mongfontbuilder/lib/mongfontbuilder/data/*.json` directly (bypassing cattrs, which would strip the `unrecommended` field from `VariantLocaleData`). Output goes to `python/mongol_norm/data/`.

Normalize table (after a change to shaping or to the encoder). The generator is the Rust example
`examples/gen_normalize_table`: it drives the crate's shaping engine directly — about 49 million
probe shapes, some 15 s in release mode — and needs `cargo`, not the extension.
`gen_normalize_table.py` runs it:

```sh
python python/scripts/gen_normalize_table.py           # = cargo run --release --example gen_normalize_table
python python/scripts/gen_normalize_table.py --check   # CI freshness check
```

Rust tables (after any JSON change — regenerate them, rebuild the extension with
`maturin develop --locked --features testing` from `python/`, then run `cargo test
--workspace` from the root). Mind the loop: the engine's tables come from the JSON and the
normalize table comes from the engine, so a shape-rule change is `gen_rust_tables.py` →
`gen_normalize_table.py` → `gen_rust_tables.py` again (the normalize table is compiled in too;
repeat until `gen_normalize_table.py --check` passes) → `maturin develop`:

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

The normalize table carries its own `schema` string instead: `mongol-normalize-table/2` holds the
online encoder's tables (`mng-canonical/3`); `/1` was the per-unit FVS-pinned table of
`mng-canonical/1` and `/2`.

## License

The shaping data is derived from UTN #57 and the mongfontbuilder project. Use under the SIL Open Font License 1.1 — consistent with the upstream sources.
