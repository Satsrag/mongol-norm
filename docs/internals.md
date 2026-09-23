# How mongol-norm works

Background for contributors and porters. Users only need the [README](../README.md).

> **Beta.** `shape` is considered stable. The `normalize` output (the `mng-canonical/3` policy
> described below) encodes by glyph shape, not the standard phonetic (nominal-character) spelling —
> e.g. ᠮᠣᠩᠭᠣᠯ (`MA+O+ANG+GA+O+LA`) → ᠮᠣᠩᠨ᠋ᠨᠣᠯ (`MA+O+ANG+NA+FVS1+NA+O+LA`) — and may change in a later release.

The normalizer implements a **lightweight Mongolian shaping engine** — equivalent to what HarfBuzz
does with a font file, but using only the rule data from
[UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) and the
[mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) project. No font files needed.

## Shaping pipeline (UTN #57 v4 Mongolian-specific phase)

1. **Chachlag** — Suffix forms for A/E after MVS (Mongolian Vowel Separator)
2. **Syllabic** — Consonant/vowel context: onset, devsger, marked, masculine/feminine harmony, dotless
3. **Particle** — MVS particle dictionary lookup for specific suffix words
4. **Devsger** — I after a vowel (vowel_devsger) gets double-tooth form: `I → I+I`
5. **Post-bowed** — Vowel forms change after bowed consonants (G, B, K, P, F)

## Duplicate encodings

`shape` promises to be a fingerprint of the *visible* word — "same shape ⟹ same `normalize`" is
this crate's whole reason to exist. Nine written units break that promise: they render as exactly
the same ink as a sequence of other units, so two spellings of one word shaped differently.

Five are unified by **expanding** the single unit into the pair:

| duplicate | public shape | witness pair |
|---|---|---|
| `Dd:medi` | `O:medi A:medi` | ᠣᠳᠪᠣ / ᠠ᠋ᠣᠣᠠᠪᠣ᠋ |
| `Dd:fina` | `O:medi A:fina` | ᠠᠷᠠᠳ / ᠠᠷᠠᠤᠠ |
| `H:medi`  | `A:medi A:medi` | ᠪᠠᠭᠰᠢ / ᠪᠠᠠᠠᠰᠢ |
| `Hx:medi` | `N:medi N:medi` | ᠠᠷᠭᠠᠯ / ᠠ᠋ᠠᠷᠨ᠋ᠨ᠋ᠠᠯ |
| `Cr:init` | `O:init O:medi` | ᡂ᠊ / ᠤ᠋ᠤ᠊ |

Four other forms unify by **contraction**, choosing the shorter canonical form only in the
verified position and context. `Aa:fina` has a tooth immediately after a bowed written unit and no tooth
elsewhere; repeated insertion/deletion of `A` would not preserve the ink:

| pair | public shape | witness pair |
|---|---|---|
| `A Aa` spanning a whole chain | `A:isol`  | ᠡ / ᠡᠠ᠋ |
| `bowed written unit A:medi Aa:fina` | `bowed written unit Aa:fina` | ᠪᠠ / ᠪᠠᠠ᠋ |
| `O Aa` ending a chain         | `B2:fina` | ᠊ᠪ᠋ / ᠊ᠤᠠ᠋ |
| `I Aa` ending a chain         | `G:fina`  | ᠊ᠭ / ᠊ᠢᠠ᠋ |

Position is part of every rule, and it is the *chain* position — a chain being the letters
between structural units, with a nirugu or ZWJ neighbour padding the chain the way it pads the
rendering. That is why the `B2` and `G` witnesses are written with a leading nirugu: it is what
makes the unit final. Forms outside a verified pair are left alone — initial and final `H`/`Hx` are
distinct ink, a lone `Cr` chain is `Cr:isol` rather than the verified `Cr:init`, and a lone `A`
chain is already canonical.

**Terminology.** UTN #57 uses **bowed written units** (圆头书写单位) and names the corresponding lookup **Post-bowed**; see [UTN #57 revision 4](https://www.unicode.org/notes/tn57/utn57-mong-4.pdf).

**Context.** The complete Hudum bowed written unit set is **B, P, F, G, Gx, K, K2**, not every consonant.
Sources: the [Hudum written-unit ligated variants](https://mongfontbuilder.pages.dev/hudum/),
[required ligatures](https://github.com/Kushim-Jiang/mongfontbuilder/blob/7d5fc1cdaf8210f675c16699a8eaeb71aa1e80ca/data/ligatures.ts)
(`BAa`, `PAa`, `FAa`, `GAa`, `GxAa`, `KAa`, `K2Aa`), and `src/rules.rs`'s post-bowed classes.
The bowed written unit must immediately precede the medial `A` in the same chain: `B A Aa → B Aa`, but
`N A Aa`, `A A Aa`, and `B A A Aa` stay unchanged. Joiner padding is not a bowed written unit and does not
let this rule cross structural tokens. The independent whole-chain `A:init Aa:fina → A:isol`
rule is unchanged.

**Termination and idempotence.** Expand once, then inspect the final pair once. Expansion emits
no expansion targets, and contraction cannot expose an initial/final `H`/`Hx` as medial.
Every contraction ends the chain: `A`, `B2`, and `G` do not end in `Aa`; a contracted `Aa` is
preceded by a bowed written unit, not `A`/`O`/`I`, so it cannot contract again. Idempotence follows from these
guards, not from forcing the text to a fixed point. The 20-unit alphabet (all bowed written units, expansion
targets and structural tokens included) is exhausted through length four: **168 421 inputs**.
Corpus written-unit re-encoding also reshapes to itself.

Expansion does not license a non-bowed written unit contraction: ᠲᠡᠳ᠌ᠡ᠋ (`T A Dd Aa`) becomes
`T A O A Aa`, **not** `T A B2`, because `O` is not a bowed written unit. The expansion of `B H Aa` likewise
leaves `B A A Aa`. Both interactions are regression-tested.

Everything user-facing sees the unified sequence: `same_shape`, `normalize` and the written-unit
encoders. `normalize_written_units` still *accepts* the duplicates as input and unifies them, so
existing caller data keeps working. ᠠᠷᠠᠳ and ᠠᠷᠠᠤᠠ are one visible word, and now one shape and one
canonical text.

UTN #57 and GB/T 25914-2023 keep all nine as distinct written units — their EAC vectors spell
ᠠᠷᠭᠠᠯ as `A A R Hx A L` — and the engine still produces them. The standard's own sequence stays
reachable through `Shaper::shape_raw` (Python: `MongolianShaper._shape_raw`), which is what the
conformance suites compare against. It is **not part of the public contract**: it is
`#[doc(hidden)]` and may change to unify further duplicates without a major bump.

`shape_detailed` and `trace`'s `written_by_token` report each token's own units, so they are raw
too — unification is a whole-word rewrite that no single token can carry. `trace`'s `shape` field
is the public, unified sequence.

## Normalization strategy

Within the normalize tables' domain, `normalize` is a **pure function of shape**: any two encodings
that shape identically produce the same Unicode output, and the output round-trips —
`shape(normalize(x)) == shape(x)`. It is also **prefix-stable**: the encoding of a word's
shape-prefix is a prefix of the word's encoding, apart from its last letter. When these goals
conflict the priority is **round-trip > prefix-stable > shortest**.

Per word, `normalize` shapes the input (duplicate encodings unified, see above) and encodes the
written units left to right with the **online encoder** (`src/encoder.rs`). Structural characters —
MVS, nirugu, ZWJ — appear in the shape verbatim as PascalCase `Mvs` / `Nirugu` / `Zwj` tokens and
are copied through; they are also the evidence for a neighbour's init/medi/fina form.

**Prefix-stability is online encoding.** Every letter but the last is *committed*: appended to the
output and never revised. The last letter stays *pending* until the word ends. An encoder that works
this way is prefix-stable by construction, whichever letters it picks, so the design question is
only which letter to commit, and when.

**A committed letter must be right-robust.** When a letter is committed, everything left of it is
known — it is committed text. What is unknown is the rest of the word. A letter may be committed only
if it renders its own written units, and leaves every committed letter's units intact, for *every*
continuation the encoder may still write. The pending last letter needs no robustness: it is chosen
in its actual, complete context.

**Promises.** The encoder writes the next letter too, so a committed bare letter may constrain it.
A medial bare `n` renders `N` only before a vowel (elsewhere a plain tooth, `A`), so it may be
committed with the promise "the next letter is a vowel" instead of as `n+FVS1`; the next letter is
then chosen from that class. Five classes
cover the rules that look right: *vowel*, *vowel but not `ee`* (`t` before `ee` takes its devsger
form), *masculine vowel*, *feminine or neutral vowel*, *consonant or `ee`*.

**Lazy commitment.** A pending tail waits as long as one final letter can still end the word with it,
which keeps multi-unit letters available: `a` = `A A`, `o` = `A O`, `oe` = `A O I`, `ng` = `A G`,
`i` after a vowel = `I I`, … When a new unit makes that impossible, a small search over the tail —
usually one or two units, at most three per letter — commits the cheapest right-robust letters that
leave a tail one final letter can end. A structural token commits the whole tail (its last letter
at `fina`/`isol`, or at `medi`/`init` before a joiner). At the end of the word the tail becomes the
final letter. If no letter ends it — a shape outside the tables' domain; none is known — strict
normalization fails with the input and its written units, and the lenient variant returns the input
unchanged; it never mis-encodes.

**Preference.** Among letters of equal cost (a bare letter is one code point, a letter with an FVS
two), the encoder prefers, in this order:

- for a final `A` right after a vowel, `n` — a vowel after a vowel is foreign to the language, a
  final `n` is common;
- after a feminine vowel (the last masculine or feminine vowel written; an MVS does not reset it,
  so a suffix follows its stem), `e oe ue` before `a o u`;
- code-point order, except `g` before `h` (ᠭᠡᠷ, not ᠬᠡᠷ);
- then the lower FVS.

An isolated `I` is written `i+FVS1`, never `j`. The renderings that shape unifies with a unit pair
(`Dd`, medial `H`/`Hx`, initial `Cr`) are never emitted.

The output encodes the glyphs, not the spelling. ᠰᠠᠢᠨ becomes `s a i+FVS3 i n`: `S A I I` is itself
a shape, whose first `I` must be committed as a single medial `I` after a vowel, and only `i+FVS3`
renders that. No function of the glyphs can restore the intended spelling in general — ᠲᠡᠷᠡ *tere*
and ᠳᠡᠷᠡ *dere* render identically — and correct spelling contradicts prefix-stability: ᠪᠠᠯ *bal*
is a prefix shape of ᠪᠡᠯᠭᠡ *belge*, so belge's key starts like bal's.

**How the tables are built.** Whether a letter is right-robust depends on the committed text only
through a small *context*: the previous letter (letter, FVS, position, written units); the previous
token (none, letter, MVS, nirugu, ZWJ) and whether an MVS followed the previous letter; whether an
initial consonant has been followed by a medial one (III.2a cluster `marked`); whether the nearest
vowel back is a masculine one in initial or medial position (III.2f `g`/`h`); the particle-key
prefix of the current MVS segment (III.3); and the promise in force. `examples/gen_normalize_table`
explores every context the encoder can reach, breadth first from the word start (about 2 700), and
asks the shaping engine — by shaping probe texts: the committed text, the candidate letter, and
every continuation the encoder may write over the next two or three letters, plus every particle
key the segment can still complete — which letters may be committed before which next unit under
which promise, and which letters can end the word. States with the same context must get the same
answers; the generator fails otherwise (the context would miss something the rules see). Each
answer is tabulated over the fewest context components that determine it, as a default plus
exception rows. The run takes about 49 million probes, 15 s on eight cores. It writes
`python/mongol_norm/data/MNG.normalize.json` (schema in
[`docs/data-format.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/data-format.md)),
which `gen_rust_tables.py` compiles into `src/generated/mng_normalize.rs`.

**Guarantees.** Canonical and idempotent: the input is the shape. Prefix-stable: by construction.
Round trip: every committed letter is robust against every continuation the encoder can write, and
the final letter is chosen in context; the tests check it on every input of up to two symbols
(letters with and without each FVS, MVS, nirugu, ZWJ), 2 000 random words dense in structural
characters, every corpus word and every prefix of it (`tests/online_encoder.rs`), and the canonical
golden vectors. Debug builds re-shape every output (`debug_assert!`); release builds do not.
`normalize_written_units` and the positioned API still re-shape, as their input may be no shape at
all.

On the 1 990 corpus shape groups the output averages **6.62 code points** (0.64 of them FVS;
`mng-canonical/2`: 8.46, 2.25 FVS; the shortest encodings with no prefix-stability requirement
average 5.83). `normalize` takes about 0.95 µs per word, of which `shape` is 0.58 µs
(`mng-canonical/2`: 2.8 µs).

The exact canonical selection policy is frozen as **`mng-canonical/3`**. It is available as
`Shaper::canonical_version` (Python: `shaper.canonical_version`) and embedded in
`MNG.normalize.json`. Applications that persist normalized search/index keys should store this
version alongside them and rebuild those keys if a future release changes it.

**`mng-canonical/3` invalidates keys stored under `mng-canonical/2`.** The online encoder replaces
the per-unit FVS-pinned table of `/2`: 1 656 of the 1 990 golden representatives change (1 562
shorter, 91 the same length, 3 longer). Rebuild any stored normalized key.

**`mng-canonical/2` (0.2.0) invalidated keys stored under `mng-canonical/1`.** Unifying the nine
duplicate encodings changed canonical text: comparing against the base branch's 1993
`mng-canonical/1` golden representatives found **287** changed texts; three verified pairs merged
into **1990** groups.

## Data and fixtures

The shaping and normalize rules are flat, language-agnostic JSON in `python/mongol_norm/data/`
(`MNG.json`, `TOD.json`, `SIB.json`, `MCH.json` and `MNG.normalize.json`). Nothing reads it at
runtime: `python/scripts/gen_rust_tables.py` compiles it into the static Rust tables in
`src/generated/`, which is what both the crate and the wheel carry. The wheel still ships the JSON
for tooling. The schema and the consuming algorithm are documented in
[`docs/data-format.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/data-format.md), so a
port in another language needs only a JSON parser.

Both test suites read the same fixtures, which live once under the crate's `tests/`:

| Fixture | Contents |
|---|---|
| `tests/data/core-hud.tsv` | 177 rows — mongfontbuilder's curated regression set (225 cases) |
| `tests/data/eac-hud.tsv` | 3512 rows — GB/T 25914-2023 (3513 cases, 5 UTN-xfail) |
| `tests/golden/mng-canonical-v1.jsonl` | 1990 canonical vectors |
| `tests/golden/mng-phase-trace-v1.json` | 15 phase-trace vectors |

Because the corpus and golden tests read that directory, `cargo test` needs a repository checkout —
the published crate does not include the fixtures.
