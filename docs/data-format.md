# mongol-norm data format · 数据格式

[English](#english) | [中文](#中文)

---

<a id="english"></a>
## English

Flat, language-agnostic data for Traditional Mongolian shaping + normalization.

mongol-norm bundles pre-processed JSON in [`python/mongol_norm/data/`](../python/mongol_norm/data/) that encodes the **letter × position × FVS → written-unit** mapping plus vowel-harmony categories, shaping conditions, the MVS particle dictionary, and the normalize table — everything a UTN #57 shaper/normalizer needs, minus the algorithm itself.

**Audience:** anyone implementing a Mongolian shaper or normalizer in any language (JS, Dart, Java, C, PHP, …; Rust and Python are covered by the crate and its bindings). The JSON has no language-specific structure — it is generated once (see [Regenerating](#regenerating)) and committed; there is no separate data package to install.

**How mongol-norm itself uses it:** the runtime — the Rust crate at the repository root ([`src/`](../src/)), which the Python package wraps — does not read this JSON. `python/scripts/gen_rust_tables.py` compiles it into static Rust tables (`src/generated/`); the JSON is the input of that generator and of the other tooling (the Rust example `examples/gen_normalize_table`, run by `python/scripts/gen_normalize_table.py`, writes `MNG.normalize.json` from `MNG.json`; the tests read the files through `mongol_norm._data`). The wheel still ships the files for that tooling.

The rules are derived from:
- [UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) — Unicode technical note defining the shaping algorithm.
- [mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) — the machine-readable variant data authored alongside UTN #57.

---

### What's in `python/mongol_norm/data/`

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

### Getting the JSON

#### Python

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

#### Rust (the engine)

The Rust crate at the repository root ([`src/`](../src/)) — the engine the Python
package runs — does not read the JSON at runtime. `python/scripts/gen_rust_tables.py` turns these
files into static Rust tables (`src/generated/*.rs`: the `WrittenUnit` /
`Condition` / `Alias` enums, the per-locale shaping tables and the MNG normalize table), and
`python/tests/test_rust_twin.py` fails the Python suite whenever the committed tables are stale:

```sh
python python/scripts/gen_rust_tables.py          # regenerate after changing the JSON
python python/scripts/gen_rust_tables.py --check  # what CI runs
```

#### Any other language

Grab the raw files directly:

- From the wheel/sdist on PyPI (`mongol_norm/data/`), or
- From the repo: [`python/mongol_norm/data/*.json`](../python/mongol_norm/data/)

Bundle the file with your package. Parse with any JSON library.

---

### JSON schema

#### Top level

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

#### `letters[]`

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

#### `letters[].variants[]`

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

#### `categories`

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

#### `particles`

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

#### Conditions

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

### Algorithm

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

### Recommended runtime indexes

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

### Normalize table (`MNG.normalize.json`)

Alongside the shape rules, `python/mongol_norm/data/` ships a **normalize table** for locales that support normalization (currently `MNG`). Where the shape rules drive *letter → glyph*, this table drives the reverse: the tables of mongol-norm's **online normalize encoder** (policy `mng-canonical/3`; the design is in [`docs/internals.md`](internals.md), "Normalization strategy"). For every `(position, written units)` it lists the letters that render them in preference order, says when a context-dependent letter may be *committed* — written before the rest of the word is known — and which letters can end a word.

With this file, the shape rules and the algorithm below, a port reproduces mongol-norm's `normalize` byte for byte: the shaper computes the input's shape, the encoder itself never shapes and never searches beyond a few units. mongol-norm's own engine consumes this exact file, compiled into `src/generated/mng_normalize.rs` by `python/scripts/gen_rust_tables.py`. A shape the tables cannot encode is outside this contract; the Python API raises `NormalizationFallbackError` by default and preserves the input only when called explicitly with `strict=False` (the Rust API: `Error::NormalizationFallback`, or `normalize_allow_fallback`). No reachable shape is known to hit this.

```python
from mongol_norm._data import load_normalize_table
tbl = load_normalize_table("MNG")   # -> dict
```

#### Schema

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

##### Context tables

`{"projection": [component names], "default": value, "rows": [[key, value], …]}` gives a value per context. The key of a context under a projection concatenates the projected components' values in `context_components` order, each in its bit width (`key = (key << bits) | value`), written in hex. The value is the row with that key (rows are sorted by key), or `default`. An empty projection is a constant.

#### The context

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

#### Preference

The options of a group are tried in this order: without an FVS first, then `n` first where it applies, then by rank, then by FVS (1–4). **Rank** is `cp × 2`, except that `g` (`182D`) ranks `182C × 2 − 1`, just before `h`; when the harmony is feminine, `e`, `oe`, `ue` rank one below `a`, `o`, `u`. **`n` first** applies to the option `n` rendering `A` when the letter ends its chain right after a vowel — the previous token is a letter and that letter a vowel — and only for the word's final letter and for the last letter before a structural token. With masculine or no harmony and no `n`-first, this is the order of `candidates`.

#### Lookups

- **allows(p, letter)**: `p` is 0, or the letter is in class `p`.
- **feasible(p, u)** — may promise `p` > 0 be made before next unit `u`? Yes when some candidate renders exactly `[u]` at `medi` or `fina`, and at each of those two positions where one does, one whose letter is in class `p` does too.
- **robust(ctx, c, next, p)** — may `c` be committed before `next` (the unit after its span, or the structural token that closes it) with promise `p`? `"always"`: iff `p` is 0. `"never"`: no. A table: bit `next` of `mask_sets[value][p]`.
- **final(ctx, units)** — the letter that ends the word with `units`: the position is `isol` if chain-first, else `fina`; take the group's `valid` mask (none: no final letter), and return the first option in preference order whose bit is set and whose letter `allows(ctx.promise)`.
- **cost** — 1 for a bare letter, 2 with an FVS.

#### Consuming it (the normalize algorithm)

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

### Regenerating

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

### Schema versioning

`schema_version: 1` is the initial schema. Incompatible changes (field removal, type changes, semantic shifts) increment this. Additive changes (new optional fields) do not.

Consumers should check `schema_version` on load and fail loudly on unknown values.

The normalize table carries its own `schema` string instead: `mongol-normalize-table/2` holds the
online encoder's tables (`mng-canonical/3`); `/1` was the per-unit FVS-pinned table of
`mng-canonical/1` and `/2`.

### License

The shaping data is derived from UTN #57 and the mongfontbuilder project. Use under the SIL Open Font License 1.1 — consistent with the upstream sources.

---

<a id="中文"></a>
## 中文

传统蒙古文整形与规范化用的扁平、与编程语言无关的数据。

mongol-norm 在 [`python/mongol_norm/data/`](../python/mongol_norm/data/) 里附带预处理好的 JSON，内容包括
**字母 × 位置 × FVS → 书写单元** 的映射，以及元音和谐类别、整形条件、MVS 粒子词典和规范化表——实现一个
UTN #57 整形器/规范化器所需的全部数据，只差算法本身。

**读者：** 用任何语言（JS、Dart、Java、C、PHP……；Rust 和 Python 已由本 crate 及其绑定覆盖）实现蒙古文整形器
或规范化器的人。JSON 没有任何语言特定的结构——它生成一次（见[重新生成](#zh-regenerating)）后提交到仓库；
不需要另外安装数据包。

**mongol-norm 自己怎么用它：** 运行时——仓库根目录的 Rust crate（[`src/`](../src/)），Python 包就是对它的封装——
不读取这些 JSON。`python/scripts/gen_rust_tables.py` 把它们编译成静态 Rust 表（`src/generated/`）；JSON 是这个
生成器以及其他工具的输入（Rust example `examples/gen_normalize_table` 由 `python/scripts/gen_normalize_table.py`
调用，从 `MNG.json` 生成 `MNG.normalize.json`；测试通过 `mongol_norm._data` 读取这些文件）。wheel 仍然附带
这些文件供工具使用。

规则来源：
- [UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) —— 定义整形算法的 Unicode 技术说明。
- [mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) —— 与 UTN #57 一同编写的机器可读变体数据。

---

### `python/mongol_norm/data/` 里有什么

```
python/mongol_norm/data/
├── MNG.json            — 回鹘式（传统蒙古文）整形规则
├── MNG.normalize.json  — 回鹘式规范化表（见下文）
├── TOD.json            — 托忒文
├── SIB.json            — 锡伯文
└── MCH.json            — 满文
```

每个 `<LOCALE>.json` 都是自包含的：一个 JSON 文档里有整形该 locale 所需的全部数据。`<LOCALE>.normalize.json`
（目前只有 `MNG`）额外支持*规范化*——见[规范化表](#zh-normalize-table)。

大小：整形规则每个 45–60 KB；规范化表约 95 KB。

### 获取 JSON

#### Python

这些文件随 `mongol-norm` wheel 发布，供工具使用——整形器本身不读取它们（它的表是编译进去的）。要直接读取，
可以用脚本和测试使用的内部加载函数：

```python
from mongol_norm._data import load_rules, load_normalize_table
rules = load_rules("MNG")             # -> dict
table = load_normalize_table("MNG")   # -> dict
```

如果想在不提供名义 Unicode 的情况下规范化一个书写单元序列，请使用公开 API，而不是内部的数据加载函数：

```python
from mongol_norm import MongolianShaper

shaper = MongolianShaper(locale="MNG")
shaper.normalize_written_units(["B", "Aa"])
# -> "ᠪᠠ"

shaper.normalize_written_units(["S", "A", "I", "I", "N", "Mvs", "Aa"])
```

输入必须是有序的 `Sequence[str]`，使用与 `shape()` 返回值相同的单元词汇。所有书写单元名都是 PascalCase；
结构控制符是 `Mvs`、`Nirugu` 和 `Zwj`。九个重复编码（见下文规范化算法）在这里仍然*接受*，并在编码前统一，
所以从旧版 `shape()` 得到的数据照常可用；只是 `shape()` 不会再输出它们。旧的全小写和全大写控制符别名会被
拒绝。位置根据顺序和结构上下文推断——这个 API 不接受显式的位置记录，也从不推断或插入结构控制符。特别地，
只有请求中含有 `Zwj` 时输出才含 ZWJ。空序列返回空字符串。外层输入格式错误或含非字符串元素时抛 `TypeError`；
未知单元以及无法整形回完全相同单元序列的输入抛 `ValueError`。没有部分输出或"取第一个候选"之类的回退。

如果调用方掌握权威的 HUD 书写单元位置，请使用记录形式的 API：

```python
shaper.normalize_positioned_written_units([
    {"unit": "B", "position": "init"},
    {"unit": "Aa", "position": "fina"},
])
```

每个元素必须是内置 dict，并且恰好包含字符串字段 `unit` 和 `position`。`position` 是该书写单元在权威 HUD
清单里的位置，而不是输出的 Unicode 字母的连写拓扑。解析后的变体引用保留这一区别：独立形的 Unicode FA 借用
`F:init`，所以单独一条 `F:init` 记录编码为裸的 `U+1839`；`F:isol` 不是合法组合，会被拒绝。字母的位置是
`isol`、`init`、`medi` 或 `fina`；结构单元 `Mvs` 和 `Nirugu` 要求 `control`。显式的 `Zwj` 输入会被拒绝，
但当某个合法的 HUD 位置需要连写上下文时，编码器可能在输出的 Unicode 里插入 ZWJ。有裸字母候选的借用形式保持
裸写：单独一条 `init` 记录，如果其裸字母读回来就是这条记录——所有辅音都是如此，它们的独立形借用词首单元——
则编码时不加 ZWJ。单独一条 `init` 记录，如果其裸字母读回来会是另一条记录，就在后面加一个 U+200D：`A:init`
和 `I:init`（它们的裸写同时也是 `A:isol` / `I:isol` 的写法，后两者保持裸写），以及没有裸写形式的 `O:init`。
生成器把未改动的普通整形追踪与源数据中的 `positioned_written` 元数据结合起来，验证精确的位置以及 MVS 边界上的
替代形式。规范化器查询这份清单（已编译进 Rust 表）；公开的 `shape()` 仍然是普通序列，不含位置记录。外层/
记录/字段类型错误抛 `TypeError`；键名、单元、位置、结构上下文、精确编码有误，或记录超过 1024 条，抛
`ValueError`。这个词级 API 目前没有对应的 CLI 子命令。

普通 `normalize_written_units()` API 的 CLI 版本接受无歧义切分的紧凑 PascalCase 单元，或者用 `+` 显式分隔：

```sh
mongol-norm normalize-written-units 'B+Aa'
mongol-norm normalize-written-units 'BZwj'
echo 'B+Aa' | mongol-norm normalize-written-units -
mongol-norm normalize-written-units --batch -i units.txt -o canonical.txt
```

批量输入每行一个紧凑或 `+` 连接的序列。紧凑输入如果有不止一种合法切分就报错退出；用 `+` 指明边界即可。例如
`AAaBZwj` 切分为 `A+Aa+B+Zwj`，之后与序列输入一样做精确 shape 可编码性校验。单元名不能为空，也不能带首尾空白。

#### Rust（引擎）

仓库根目录的 Rust crate（[`src/`](../src/)）——Python 包运行的就是这个引擎——运行时不读取 JSON。
`python/scripts/gen_rust_tables.py` 把这些文件转成静态 Rust 表（`src/generated/*.rs`：`WrittenUnit` /
`Condition` / `Alias` 枚举、各 locale 的整形表和 MNG 规范化表），一旦提交的表过期，
`python/tests/test_rust_twin.py` 就会让 Python 测试失败：

```sh
python python/scripts/gen_rust_tables.py          # 修改 JSON 后重新生成
python python/scripts/gen_rust_tables.py --check  # CI 运行的检查
```

#### 其他语言

直接获取原始文件：

- 从 PyPI 上的 wheel/sdist（`mongol_norm/data/`），或
- 从仓库：[`python/mongol_norm/data/*.json`](../python/mongol_norm/data/)

把文件打包进你的项目，用任意 JSON 库解析即可。

---

### JSON 格式

#### 顶层

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

| 字段 | 类型 | 含义 |
|---|---|---|
| `schema_version` | int | 格式有不兼容变更时递增。当前为 `1`。 |
| `locale` | string | `MNG`、`TOD`、`SIB`、`MCH` 之一。 |
| `generated_from` | object | 来源——生成此文件的 mongfontbuilder 版本。 |
| `letters` | array | 每个字母的数据（见下文）。 |
| `categories` | object | 用于元音和谐和辅音逻辑的音系类别。 |
| `particles` | object | 整形流程第 3 步使用的、以 MVS 开头的粒子。 |

#### `letters[]`（字母）

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

| 字段 | 类型 | 含义 |
|---|---|---|
| `cp` | int | Unicode 码位（十进制）。例如 `6176` = U+1820。 |
| `name` | string | Unicode 字符名。 |
| `alias` | string\|缺失 | 该码位在当前 locale 中的简短音位名（`"a"`、`"e"`、`"h"`……）。没有定义别名时可能缺失。 |
| `variants[]` | array | 该字母在此 locale 中的全部整形变体。 |

#### `letters[].variants[]`（变体）

| 字段 | 类型 | 含义 |
|---|---|---|
| `position` | string | `isol`、`init`、`medi`、`fina` 之一。 |
| `fvs` | int | `0` = 无 FVS；`1..4` = FVS1..FVS4（Unicode U+180B、U+180C、U+180D、U+180F）。 |
| `written` | 字符串数组 | 该变体渲染成的**书写单元**序列。书写单元是字形原子的不透明标识符（例如 `"A"`、`"Aa"`、`"Bg"`、`"Ix"`）。你的整形器不需要解释它们——只把它们当字符串比较和拼接。 |
| `positioned_written` | 记录数组\|缺失 | 用于 MNG 规范化：同样的解析后单元，但保留其权威 HUD 位置。引用保留被引用的位置，所以 FA 的 `isol` 是 `[{"unit":"F","position":"init"}]`，而不是虚构的 `F:isol`。其他 locale 目前不含这个仅用于规范化的字段。 |
| `default` | bool | 没有 FVS/条件适用时使用的变体为 `true`。每个 `(cp, position)` 恰好一个默认变体。 |
| `conditions` | 字符串数组 | 选中此变体的命名整形条件。见下文[条件](#zh-conditions)。 |
| `archaic` | bool | 按 UTN #57 为古旧变体。计算反向映射（规范化器）时跳过。 |
| `unrecommended` | bool | 不推荐使用的变体。计算反向映射时跳过。 |

#### `categories`（类别）

```json
{
  "vowel":          ["a", "e", "i", "o", "u", "oe", "ue"],
  "consonant":      ["n", "ng", "b", ...],
  "vowelMasculine": ["a", "o", "u"],
  "vowelFeminine":  ["e", "oe", "ue"],
  "vowelNeuter":    ["i"]
}
```

按音系分组的别名（与 `letters[].alias` 相同）。用于：

- 第 2 步（Syllabic），根据相邻元音类别指定条件。
- 规范化器的元音和谐判断（阳性词与阴性词）。

#### `particles`（粒子）

```json
{
  "u u":      [0],
  "ue ue":    [0],
  "mvs a ch a": [1]
}
```

- **Key**——以空格分隔的别名序列，以 `mvs`（或一个元音别名）开头。
- **Value**——该段内应获得 `"particle"` 条件的 token 下标列表（从 0 开始）。

用于整形流程第 3 步。把每个以 MVS 开头的段与这些 key 匹配；匹配成功时，给列出的下标加上 `"particle"` 条件。

<a id="zh-conditions"></a>
#### 条件

整形算法给每个 token 指定一个 `condition`，然后查找该 `(cp, position)` 的哪个 FVS 变体的 `conditions` 列表里
有这个条件。MNG 的条件词汇如下：

```
chachlag              — MVS 之后的后缀形式
chachlag_onset        — chachlag 的起首
chachlag_onset_gb     — GB 特有的 chachlag 起首
onset                 — 词首辅音
masculine_onset       — 阳性和谐词中的 onset
devsger               — "连接齿"形式
masculine_devsger     — 阳性上下文中的 devsger
vowel_devsger         — 元音之后的 i（双齿）
feminine              — 阴性和谐上下文
marked                — 显式标记的变体
dotless               — 无点形式
particle              — 由第 3 步根据粒子词典设置
post_bowed            — 圆头辅音（G、B、K、P、F）之后的元音
```

其他 locale 可能使用不同的子集。

---

### 算法

本文档**只描述数据**。算法见：

- **规范：** [UTN #57 v4 第 3 节（"Shaping"）](https://www.unicode.org/notes/tn57/tn57-4.html)——五步的蒙古文专用
  整形阶段。
- **参考实现：** [`src/`](../src/)——无依赖的 Rust：`token.rs`（分词、结构位置）、`rules.rs`（五个阶段，每条规则
  一个函数）、`shaper.rs`（变体解析；`shape` / `same_shape` / `shape_detailed` / `trace`）、`normalize.rs` 和
  `encoder.rs`（规范化算法）、`written_units.rs`（书写单元输入 API）。Python 包通过 `python/` 下的绑定 crate
  调用的正是这些代码。

五个步骤概要：

1. **Chachlag**——给 MVS 之后的字母打上后缀形式的条件。
2. **Syllabic**——根据音系上下文（元音类别、相邻辅音、词中位置）给每个字母指定条件。
3. **Particle**——把以 MVS 开头的段与 `particles` 词典匹配；给命中的下标打上 `"particle"`。
4. **Devsger**——元音之后词中位置的 `i` 得到 `"vowel_devsger"`（渲染为双齿）。
5. **Post-bowed**——圆头辅音（G、B、K、P、F）之后的元音得到 `"post_bowed"`。

打好条件的每个 token 再通过扫描 `conditions` 列表解析到具体的 FVS 变体。

---

### 推荐的运行时索引

随包提供的 JSON 是唯一数据源；每个移植应当自己建立内存索引。常见的有：

```
cp_to_alias           cp  -> alias
alias_to_cp           alias -> cp
variant_by_key        (cp, position, fvs) -> variant
default_by_pos        (cp, position) -> variant (default one)
condition_to_fvs      (cp, position, condition) -> fvs
reverse_map           (position, tuple(written)) -> (cp, fvs)
```

参考实现分两步建立它们：`python/scripts/gen_rust_tables.py` 在生成时从 JSON 派生扁平表，`src/shaper.rs` 在
创建 `Shaper` 时为它们建立索引——两者都读一下就能看清具体做法。

---

<a id="zh-normalize-table"></a>
### 规范化表（`MNG.normalize.json`）

除整形规则外，`python/mongol_norm/data/` 还为支持规范化的 locale（目前是 `MNG`）提供一份**规范化表**。整形规则
驱动的是*字母 → 字形*，这张表驱动反方向：它是 mongol-norm **在线规范化编码器**的表（策略 `mng-canonical/3`；
设计见 [`docs/internals.md`](internals.md#中文) 的"规范化策略"）。对每个 `(位置, 书写单元)`，它按偏好顺序列出
能渲染它们的字母，说明依赖上下文的字母何时可以*提交*——即在词的其余部分未知时就写出——以及哪些字母可以结束
一个词。

有了这个文件、整形规则和下面的算法，移植就能逐字节复现 mongol-norm 的 `normalize`：整形器计算输入的 shape，
编码器本身从不整形，也不做超过几个单元的搜索。mongol-norm 自己的引擎使用的正是这个文件，由
`python/scripts/gen_rust_tables.py` 编译成 `src/generated/mng_normalize.rs`。表无法编码的 shape 不在这份契约
之内；Python API 默认抛 `NormalizationFallbackError`，只有显式传入 `strict=False` 时才原样返回输入（Rust API：
`Error::NormalizationFallback`，或 `normalize_allow_fallback`）。目前没有已知的可达 shape 会遇到这种情况。

```python
from mongol_norm._data import load_normalize_table
tbl = load_normalize_table("MNG")   # -> dict
```

#### 格式

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

| 字段 | 含义 |
|---|---|
| `canonical_version` | "shape → 规范 Unicode" 精确选择策略的版本。把它和规范化后的索引 key 一起保存；这个值变了，说明已存的 key 可能需要重建。 |
| `constants` | MVS / nirugu / ZWJ / FVS1–4 的十六进制码位。 |
| `context_components` | 编码器上下文各分量的 `[名字, 位数]`，按 key 打包顺序排列（见[上下文](#zh-context)）。 |
| `promises` | 承诺类别 1–5，按顺序：`[名字, 字母别名]`。承诺 0 表示"无承诺"。 |
| `particle_nodes` | 粒子 key 的前缀树（见[上下文](#zh-context)）。节点 0（`""`）是词首，节点 1（`"M:"`）是 MVS 之后一段的开头。其他每个节点的名字 = 父节点名字 + 该字母的别名（`"M:y i"` 是 `"M:y"` 经 `i` 到达的子节点；`"u"` 是 `""` 经 `u` 的子节点，`"M:u"` 是 `"M:"` 的子节点）。 |
| `written_sequences` | 所有候选的 `written`，已排序；上下文分量 `prev_written` 是它的下标（从 1 开始）。 |
| `mask_units` | 下一单元掩码的位序：第 `i` 位对应 `mask_units[i]`（即 `WrittenUnit` 的顺序，包含结构符）。 |
| `mask_sets` | 稳健性答案的公共池：每项六个十六进制掩码，对应承诺 0–5。某位为 1 表示"在下一个单元是它时、带这个承诺可以提交"。 |
| `candidates` | 所有编码选项：`letter`（别名）、`cp`、`fvs`（十六进制码位，裸字母为 `null`）、`position`（`isol`/`init`/`medi`/`fina`）、`units`（它渲染出的书写单元，用 `+` 连接；`written` 是引擎对同一序列的命名），以及 `robust`：`"always"`（任何上下文都可以提交，但不能带承诺）、`"never"`，或一张值为 `mask_sets` 下标的[上下文表](#zh-context-tables)。同一 `(position, units)` 组的选项连续排列，并按基础偏好顺序排序（见[偏好](#zh-preference)）。 |
| `finals` | `isol`/`fina` 组里哪些选项可以结束一个词：`valid` 是该组选项上的十六进制掩码（第 `i` 位是组内第 `i` 个候选），或者由这种掩码组成的上下文表。这里没有的组永远不能结束一个词。 |
| `known_units` | 该 locale 的字母能渲染出的所有书写单元，包括九个重复编码；加上 `Mvs`、`Nirugu`、`Zwj` 就是 `normalize_written_units()` 的词汇表。 |
| `positioned_units` | 完整的合法 HUD `(unit, position)` 清单，用来校验带位置的请求（`normalize_positioned_written_units()`，它通过 `normalize_written_units()` 编码；链边缘不完整时用隐式 ZWJ 表示）。 |

`cp`、`fvs`、掩码和行 key 都是**十六进制字符串**——按 16 进制解析。

<a id="zh-context-tables"></a>
##### 上下文表

`{"projection": [分量名], "default": 值, "rows": [[key, 值], …]}` 给每个上下文一个值。一个上下文在某个投影下的
key，是把投影中各分量的值按 `context_components` 的顺序拼接起来，每个分量占其固定位数
（`key = (key << bits) | value`），写成十六进制。值取 key 相同的那一行（rows 按 key 排序），找不到就取
`default`。投影为空时就是一个常量。

<a id="zh-context"></a>
#### 上下文

编码器维护一份关于已提交文本的上下文。每提交一个字母或结构符之后，它包含：

| 分量 | 值 |
|---|---|
| `prev_letter` | 最后提交的字母（跳过结构符）：`cp − 0x1820 + 1`；第一个字母之前为 `0`。 |
| `prev_fvs` | 它的 FVS：`1`–`4`，裸字母为 `0`。 |
| `prev_position` | 它的位置：`isol` 1、`init` 2、`medi` 3、`fina` 4。 |
| `prev_written` | 它的 `written`，即 `written_sequences` 中的下标（从 1 开始）。 |
| `prev_token` | 最后提交的 token：`0` 无、`1` 字母、`2` MVS、`3` nirugu、`4` ZWJ。 |
| `mvs_since_prev` | 最后一个字母之后出现过 MVS 时为 `1`。 |
| `cluster` | 自上一个元音以来的字母是"一个词首辅音后接一个或多个词中辅音"时为 `1`（III.2a cluster `marked`）。 |
| `masculine` | MVS 段内向前最近的元音是位于词首或词中的 `a`、`o` 或 `u` 时为 `1`（III.2f）。 |
| `particle` | MVS 段目前走到的粒子前缀树节点；一旦不可能匹配任何 key，则为 `1023`（III.3）。 |
| `promise` | 最后一个字母做出的承诺（`0`–`5`）。 |

初始时所有分量都是 `0`（粒子节点为词首）。以承诺 `p` 提交候选 `c` 时：

- `cluster`：元音打断 cluster；`init` 位置的辅音开始一个 cluster；`medi` 位置的辅音把已开始的 cluster 延伸为
  "词首 + 词中"（只有这时分量为 `1`）；其他情况打断它。
- `masculine`：`e oe ue ee` 清零；`init`/`medi` 位置的 `a o u` 置 1；其他字母保持不变。
- `particle`：裸字母沿自己的字母走到子节点，没有子节点则变为 `1023`；带 FVS 的字母变为 `1023`；`1023` 保持不变。
- `prev_*` 描述 `c`；`prev_token` = 字母；`mvs_since_prev` = 0；`promise` = `p`。

结构符把 `promise` 置 0，把 `prev_token` 设为该结构符；MVS 还会把 `mvs_since_prev` 置 1、打断 cluster、把
`masculine` 清零，并把 `particle` 设为节点 1。

编码器还跟踪一个**元音和谐**，它不是 key 的分量：`a o u` 使其为阳性，`e oe ue ee` 使其为阴性，其他字母和所有
结构符保持不变。它只用于给同样短的字母排序。

**位置。** 当 `prev_token` 为"无"或 MVS 时，下一个字母是*链首*。后面还有同一条链的字母时，提交的字母在链首
取 `init`，否则取 `medi`。结构符前的最后一个字母：在 MVS 前取 `isol`/`fina`，在 nirugu 或 ZWJ 前取
`init`/`medi`（分别对应链首 / 非链首）。词的最后一个字母取 `isol`/`fina`。

<a id="zh-preference"></a>
#### 偏好

一组选项按以下顺序尝试：先无 FVS 的，然后在适用时把 `n` 排在最前，再按 rank，最后按 FVS（1–4）。**rank** 为
`cp × 2`，但 `g`（`182D`）的 rank 是 `182C × 2 − 1`，正好排在 `h` 之前；元音和谐为阴性时，`e`、`oe`、`ue` 的
rank 分别比 `a`、`o`、`u` 小 1。**`n` 优先**只适用于渲染 `A` 的 `n` 选项，条件是该字母紧跟在元音后面结束它的链
（前一个 token 是字母且该字母是元音），并且只用于词的最后一个字母和结构符前的最后一个字母。元音和谐为阳性或
未定、且不适用 `n` 优先时，这个顺序就是 `candidates` 中的顺序。

#### 查询

- **allows(p, letter)**：`p` 为 0，或该字母属于类别 `p`。
- **feasible(p, u)**——能否在下一个单元 `u` 前做出承诺 `p`（`p` > 0）？当某个候选在 `medi` 或 `fina` 恰好渲染出
  `[u]`，并且在这两个位置中每个有候选能渲染它的位置上，都有字母属于类别 `p` 的候选能渲染它时，可以。
- **robust(ctx, c, next, p)**——`c` 能否在 `next`（它所覆盖单元之后的那个单元，或结束它的结构符）之前带承诺 `p`
  提交？`"always"`：当且仅当 `p` 为 0。`"never"`：不能。上下文表：看 `mask_sets[值][p]` 的第 `next` 位。
- **final(ctx, units)**——用 `units` 结束这个词的字母：链首时位置为 `isol`，否则为 `fina`；取该组的 `valid`
  掩码（没有则不存在词尾字母），按偏好顺序返回第一个对应位为 1 且字母满足 `allows(ctx.promise)` 的选项。
- **cost**——裸字母为 1，带 FVS 为 2。

#### 使用方法（规范化算法）

对每个词：

1. 对词做 `shape()`（需要整形规则）。结构字符——MVS、nirugu、ZWJ——在 shape 中原样作为 PascalCase 的
   `Mvs`/`Nirugu`/`Zwj` 出现，并原样复制到输出。
2. **统一重复编码。** 有九个书写单元渲染出的墨迹和另一串单元完全相同，所以没有统一它们的移植会给同一个可见的
   词产生两个规范文本（ᠠᠷᠠᠳ 与 ᠠᠷᠠᠤᠠ）。位置指链内的位置——链是结构符之间的字母，nirugu/ZWJ 邻居会给链补位，
   所以挨着它的单元即使前面还有东西，也可能是词尾形。

   先**展开**，对每条链从左到右扫描一遍：

   | 单元 | 位置 | 替换为 |
   |---|---|---|
   | `Dd` | `medi`、`fina`（它只有这两个位置） | `O A` |
   | `H`  | `medi` | `A A` |
   | `Hx` | `medi` | `N N` |
   | `Cr` | `init` | `O O` |

   然后**只检查一次**最后相邻的一对，只在验证过的上下文里收缩。较短的形式是规范形式；这不是无条件迭代到
   不动点的改写。这里的*位置*指合并后的单元在缩短后的链里的位置：

   | 单元对 | 合并后的位置 | 替换为 |
   |---|---|---|
   | `A Aa` | `isol` | `A` |
   | `A Aa`，且在同一条链里紧跟在一个圆头书写单位之后 | `fina` | `Aa` |
   | `O Aa` | `fina` | `B2` |
   | `I Aa` | `fina` | `G` |

   回鹘式完整的圆头书写单位集合是 `B P F G Gx K K2`，来自[回鹘式连写变体](https://mongfontbuilder.pages.dev/hudum/)
   和[上游必需连字](https://github.com/Kushim-Jiang/mongfontbuilder/blob/7d5fc1cdaf8210f675c16699a8eaeb71aa1e80ca/data/ligatures.ts)，
   `src/rules.rs` 的 post-bowed 类也与之一致。`Aa:fina` 紧跟在圆头书写单位之后时带一个齿，否则没有齿。所以
   `B A Aa → B Aa`，而 `N A Aa`、`A A Aa`、`B A A Aa` 保持不变。连接符补位只提供位置，不算圆头书写单位；
   永远不要跨结构符匹配。另一条独立的整链规则 `A:init Aa:fina → A:isol` 保留。

   词首/词尾的 `H`/`Hx`、单独的 `Cr:isol` 和单独的 `A` 不变。展开一遍就够了：两个方向都不会产生展开目标，
   也不会让词首/词尾的 `H`/`Hx` 变成词中。所有收缩都在链尾结束；`A/B2/G` 不能再次收缩，收缩得到的 `Aa` 前面是
   圆头书写单位而不是 `A/O/I`。这就证明了无需反复删除也能幂等。特别地，`Dd Aa → O A Aa` 之后不能再收缩：`O`
   不是圆头书写单位。见 `src/duplicates.rs` 中 168 421 个输入的穷举和上下文回归测试。

   UTN #57 和 GB/T 25914-2023 把这九个都当作不同的单元——它们的 EAC 向量把 ᠠᠷᠭᠠᠯ 写作 `A A R Hx A L`——所以
   *整形*一致性测试必须与统一之前的序列比较（mongol-norm 通过非公开的 `Shaper::shape_raw` 提供它）。参考：
   [`src/duplicates.rs`](../src/duplicates.rs)。
3. 从左到右编码统一后的 shape。除最后一个字母外，每个字母都被提交——追加到输出且不再修改——这正是编码前缀稳定
   的原因：

   ```text
   encode(shape):
     out = "", ctx = 初始上下文, covered = 0
     for end = 1 .. len(shape):
       u = shape[end - 1]
       if u 是结构符:
         steps = plan(ctx, shape[covered : end - 1], token = u)   # 无解：该 shape 不在覆盖范围内
         提交 steps; out += u; 用 u 更新 ctx; covered = end
       else if final(ctx, shape[covered : end]) 存在:
         等待                                     # 还有一个词尾字母能结束这个词
       else if plan(ctx, shape[covered : end], token = 无) 成功:
         提交它的 steps; covered += 这些字母覆盖的单元数
       else:
         等待                                     # 目前还不是任何 shape 的前缀
     if covered < len(shape):
       out += final(ctx, shape[covered :])        # 不存在：该 shape 不在覆盖范围内
     return out
   ```

   提交一步就是追加它的字母（`cp`，若 `fvs` 不为 null 再加 `fvs`），并用它的承诺更新上下文。`plan` 寻找代价最小
   （总代价，包括词尾字母）的方式，从 `pending` 的开头提交字母，使剩下的部分是一个词尾字母；如果后面是结构符，
   则使什么都不剩下：

   ```text
   plan(ctx, pending, token):
     if pending 为空: return (代价 0, 无步骤) if token else 无
     best = (cost(final(ctx, pending)), 无步骤) if 没有 token 且该词尾字母存在
     for span = min(3, len(pending)) 递减到 1:
       closing = token if span == len(pending) else 无
       if span == len(pending) 且没有 token: continue       # 最后一个字母保持待定
       position = closing 时为 token 的收尾位置，否则为 init / medi
       next = closing if closing else pending[span]
       for option in group(position, pending[: span])，按偏好顺序:
         if not allows(ctx.promise, option.letter): continue
         if best 且 cost(option) >= best.cost: break
         for p in (option 是裸字母且不是 closing 时为 0 .. 5，否则只有 0):
           if p > 0 and not feasible(p, next): continue
           if not robust(ctx, option, next, p): continue
           sub = plan(用 option 和 p 更新后的 ctx, pending[span :], token)
           if sub:
             if 没有 best 或 cost(option) + sub.cost < best.cost:
               best = (cost(option) + sub.cost, [option 与 p] + sub.steps)
             转到下一个 span                                # 这个 span 最便宜的安全选项
     return best
   ```

   组不存在时跳过该 span。为 `best` 尝试的词尾字母和 `n` 优先规则看到的是 plan 已推进到的上下文。

参考实现：[`src/encoder.rs`](../src/encoder.rs)（`encode`、`plan`、上下文）和 [`src/normalize.rs`](../src/normalize.rs)
（表索引、`preference`、入口函数）。表由 [`examples/gen_normalize_table`](../examples/gen_normalize_table/) 生成——
见[重新生成](#zh-regenerating)。

---

<a id="zh-regenerating"></a>
### 重新生成

`python/mongol_norm/data/` 中的 JSON 是生成后提交的。脚本放在 `python/scripts/`，但会根据自身路径定位仓库，所以要
**在仓库根目录**、在已构建扩展的虚拟环境中运行
（`cd python && pip install 'maturin>=1.15,<2' && maturin develop --locked --features testing`，
见 `docs/development.md`），在相应的上游或代码修改之后执行。

整形规则（升级 `mongfontbuilder` 时）：

```sh
pip install 'mongfontbuilder>=0.10.6'          # [preprocess] extra
python python/scripts/preprocess.py            # 全部 locale
python python/scripts/preprocess.py MNG TOD    # 指定 locale
```

脚本直接读取 `mongfontbuilder/lib/mongfontbuilder/data/*.json`（绕过 cattrs，因为它会去掉 `VariantLocaleData`
的 `unrecommended` 字段）。输出到 `python/mongol_norm/data/`。

规范化表（修改整形或编码器之后）。生成器是 Rust example `examples/gen_normalize_table`：它直接驱动 crate 的整形
引擎——约 4 900 万次探测整形，release 模式约 15 秒——需要的是 `cargo`，而不是扩展。`gen_normalize_table.py`
负责调用它：

```sh
python python/scripts/gen_normalize_table.py           # = cargo run --release --example gen_normalize_table
python python/scripts/gen_normalize_table.py --check   # CI 新鲜度检查
```

Rust 表（任何 JSON 修改之后——重新生成它们，在 `python/` 用 `maturin develop --locked --features testing` 重新
构建扩展，然后在根目录运行 `cargo test --workspace`）。注意这里有个循环：引擎的表来自 JSON，而规范化表来自
引擎，所以修改整形规则的流程是 `gen_rust_tables.py` → `gen_normalize_table.py` → 再次 `gen_rust_tables.py`
（规范化表也要编译进去；重复直到 `gen_normalize_table.py --check` 通过）→ `maturin develop`：

```sh
python python/scripts/gen_rust_tables.py            # 重新生成 src/generated/
python python/scripts/gen_rust_tables.py --check    # CI 新鲜度检查
```

兼容性测试数据（`tests/golden/`，在有意修改整形或规范结果之后）：

```sh
python python/scripts/gen_compat_goldens.py          # 重新生成两个测试数据文件
python python/scripts/gen_compat_goldens.py --check  # CI 新鲜度检查
```

提交重新生成的 JSON 时，附上注明来源版本的更新说明。

### 格式版本

`schema_version: 1` 是初始格式。不兼容的变更（删除字段、改变类型、改变语义）会递增它；增量变更（新增可选字段）
不会。

使用方应在加载时检查 `schema_version`，遇到未知值时明确报错。

规范化表则使用自己的 `schema` 字符串：`mongol-normalize-table/2` 是在线编码器的表（`mng-canonical/3`）；`/1`
是 `mng-canonical/1` 和 `/2` 使用的逐单元 FVS 钉死表。

### 许可

整形数据源自 UTN #57 和 mongfontbuilder 项目。依照 SIL Open Font License 1.1 使用——与上游来源一致。
