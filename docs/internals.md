# How mongol-norm works

Background for contributors and porters. Users only need the [README](../README.md).

> **Beta.** `shape` is considered stable. The `normalize` output (the `mng-canonical/2` policy
> described below) is FVS-pinned and does not look like the spellings people usually type; it may
> change in a later release.

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

Position is part of every rule, and it is the *chain* position `normalize` already uses — slots
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

Within the normalization table's supported written-unit domain, `normalize` is a **pure function of
shape**: any two encodings that shape identically produce the same Unicode output, and the output
round-trips — `shape(normalize(x)) == shape(x)`. It is also **prefix-stable**. When these goals
conflict the priority is **round-trip > prefix-stable > shortest**.

Per word:

1. **shape** the input into its written-unit sequence. Structural characters — MVS, nirugu, ZWJ — appear verbatim as PascalCase `Mvs` / `Nirugu` / `Zwj` tokens (nirugu renders a visible stem; all three are the evidence for a neighbour's init/medi/fina form). **Split** the shape at these tokens into *chains*; the tokens themselves are copied through unchanged.
2. **encode each chain** (right-to-left, so appending a suffix can't disturb what precedes it):
   1. **partition + table lookup** — the primary path. At each position take the single unit if the table has it (preferred — clean output), else the longest available multi-unit entry, and look up `(position, written-unit) → (letter, FVS)` in an FVS-pinned table. Each value renders its unit **regardless of neighbours**, so the result is a deterministic, O(N), prefix-stable function of the shape.
   2. **velar-feminine refinement** — a `G`/`Gx` velar's forward-coupled vowel (`a`/`o`/`u`) is swapped to its feminine partner (`e`/`oe`/`ue`) for clean output.
   3. **verify** — reshape the candidate in full context; accept only if it equals the target chain shape.
   4. **no search fallback** — the table is total over the corpus (with FVS-first selection there are no gap chains left). If an out-of-corpus shape ever misses the table, normalization fails closed with the input and the uncovered written-unit sequence. Callers may explicitly ask for the lenient variant to return the input unchanged (round-trip preserved, never a mis-encoding). A letter next to a joiner simply looks its unit up at the shifted (joined) position.
3. **post-MVS suffix rule** — a chain directly after MVS takes its **standalone** canonical (drop the MVS, normalize, re-attach), so the spelling never depends on MVS. One exception: chachlag `Aa` after MVS is written the bare letter `a`. (The isolate-`I` → `i+FVS1` spelling is pinned in the table itself — no post-processing pass exists.)

**Prefix-stability** means: if word *A* = word *B* + a suffix and their shapes share a prefix, the
shared region encodes identically except the single boundary unit whose position changes (final in
*B* → medial in *A*). The per-unit table delivers this for free — each unit's encoding depends only
on its own position, never on its neighbours.

**How the table is built** (the *selection method*): offline, a **context-independence battery**
fills each `(position, written-unit)` slot with the `(letter, FVS)` that renders *exactly* that unit
in *every* probed neighbour context (the probes include a bowed consonant, so post-bowed effects
can't hide). Candidate order is **letter-major, FVS-first within the letter** — an FVS exists
precisely to pin a form against context, so the pinned variant of the right letter always beats its
context-sensitive bare form. The result is exported as JSON; the battery lives in
`python/scripts/gen_normalize_table.py`.

> Note: supported output is **FVS-pinned**, not bare — each unit carries the selector that fixes its
> form independent of context. This is what makes "same shape ⟹ same Unicode" and prefix-stability
> hold inside the table's domain.

The exact canonical selection policy is frozen as **`mng-canonical/2`**. It is available as
`Shaper::canonical_version` (Python: `shaper.canonical_version`) and embedded in
`MNG.normalize.json`. Applications that persist normalized search/index keys should store this
version alongside them and rebuild those keys if a future release changes it.

**`mng-canonical/2` (0.2.0) invalidates keys stored under `mng-canonical/1`.** Unifying the nine
duplicate encodings changes canonical text: comparing current output against the base branch's
1993 `mng-canonical/1` golden representatives finds **287** changed texts; three verified pairs
merge into **1990** groups. Rebuild any stored normalized key. The context correction also
invalidates pre-fix PR #26 output; this is not a new release or a claim that old test suites were rerun.

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
