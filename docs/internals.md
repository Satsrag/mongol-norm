# How mongol-norm works

Background for contributors and porters. Users only need the [README](../README.md).

> **Beta.** `shape` is considered stable. The `normalize` output (the `mng-canonical/3` policy
> described below) encodes by glyph shape, not the standard phonetic (nominal-character) spelling —
> e.g. ᠮᠣᠩᠭᠣᠯ (`MA+O+ANG+GA+O+LA`) → ᠮᠣᠠᠭ᠌ᠨ᠋ᠨ᠋ᠣᠯ (`MA+O+A+GA+FVS2+NA+FVS1+NA+FVS1+O+LA`) — and may change in a later release.

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

## Redundant ZWJ

UTN #57 v4 treats ZWJ (U+200D) purely as a cursive joining control: it forces its neighbours into
their joined (`init` / `medi` / `fina`) forms. (v1/v2 also used a mid-word ZWJ as a *word root
delimiter* that changed the following vowel; v3 withdrew that, and so did the reference font.)
Where the neighbours already join, or where the joined form is the same ink as the unjoined one,
the ZWJ is invisible — yet kept as a `Zwj` token it broke "same shape ⟹ same `normalize`" (ᠯᠢᠡ᠋
is `L G`, ᠯᠢ + ZWJ + ᠡ᠋ was `L I Zwj Aa`, because the token also stopped duplicate unification from
crossing it). Issue #33 has the full study.

`shape` therefore drops every ZWJ that changes no glyph. The filter (`src/redundant_zwj.rs`) runs
on the written-unit sequence after the rules resolved every letter and **before** duplicate
unification — so a joiner that separated `I Aa` no longer blocks the `G` contraction — and it is
the same function for `shape`, `trace` and `normalize_written_units`, so `[L, I, Zwj, Aa]` and the
text `ᠯᠢ‍ᠡ᠋` are one contract. `shape_raw`, `shape_detailed` and the trace tokens keep every
ZWJ of the input. Everything below is about written units: a *letter unit* is any written unit
other than the structural `Mvs`, `Nirugu` and `Zwj` (`WrittenUnit::is_structural`), and `U` names
one such unit.

| ZWJ context | public shape |
|---|---|
| `Zwj Zwj` | one copy (then judged like a single one) |
| next to a `Nirugu`, either side | dropped — the nirugu already joins |
| between two letter units (segment interior) | dropped |
| segment-initial (word start or directly after an `Mvs`) before `U` + more letters (`U:medi` → `U:init`) | dropped for `B C Ch Cr D Dd F G Gx I K K2 O P R Rh S Sh T W Y Z Zr` |
| segment-initial before a lone `U` (`U:fina` → `U:isol`) | dropped for `Aa Cr Dd I U Zr` |
| segment-final (word end or directly before an `Mvs`) after letters + `U` (`U:medi` → `U:fina`) | dropped for `Cr O Zr` |
| segment-final after a lone `U` (`U:init` → `U:isol`) | dropped for `B C Ch Cr D F G Gx H Hx K K2 L M N P R Rh S Sh T W Y Z Zr` |
| a lone `Zwj` | kept (it is the whole shape) |

The four edge lists are the maintainer's reviewed equivalence classes for Hudum, selected on #33
from an ink comparison of every unit on the reference font (mongfontbuilder `hudum.otf`): each
candidate pair was encoded with `normalize_written_units`, rendered with HarfBuzz and compared
outline against outline. Most listed pairs are identical ink (`B:init`/`B:medi`, `F`, `G`, `K`,
`P`, …, or differ by a hairline stub: `D`, `R`, `W`, `Y`); a few are explicitly accepted
near-equivalents (`S`, `Sh`, `C`, `Ch`, `Z` word-initially; the whole word-final-alone list, where
a consonant's isolated form *is* its initial unit). `A`, `N`, `L`, `M`, `H` and `Hx` differ
visibly between `init` and `medi` (crown, tooth count) and keep their word-initial ZWJ; every
ordinary unit grows a tail at the end of a word and keeps its word-final ZWJ after another
letter. "Lone" means the unit is the only letter of its segment — nothing, or an `Mvs`, on the far
side; segments are split at MVS exactly as the position assignment splits them. The lists are
Hudum data, so they apply to MNG only; the structural rows hold for every locale.

An MVS is a segment edge on both sides, and both sides behave like the word edge. The font gives
the letter before an MVS its `fina` (or `isol`) form, so `ᠠᠣ‍᠎ᠠ` vs `ᠠᠣ᠋᠎ᠠ` is the very
`O:medi`/`O:fina` pair of the word-final row and the same lists decide it (`ᠪ‍᠎ᠠ` = `ᠪ᠎ᠠ`,
`ᠠᠯ‍᠎ᠠ` keeps its `Zwj`). The letter after an MVS takes its `init` (or `isol`) form, and on the
reference font every after-MVS pair has exactly the letter glyphs of its word-initial twin. The
only extra difference is the MVS itself: followed by a joiner it is drawn as the font's dashed
"MVS" placeholder box (an MVS no rule handles), otherwise as a plain gap. The box is not text
ink, so the word-initial lists decide: `ᠠ᠎‍ᠶᠢᠨ` = `ᠠ᠎ᠶ᠌ᠢᠨ` (`Y:medi` ≡ `Y:init`), while
`ᠲᠠᠯ᠎‍ᠠ` keeps its `Zwj` (`A:fina` is not the suffix `a`).

The pass is a fixed point of left-to-right single removals: `Zwj B Zwj` first loses the leading
joiner (`B:medi` ≡ `B:init`), then — now a lone `B:init` — the trailing one (`B:init` ≡ `B:isol`).
Applying it to its own output changes nothing, which is what `normalize`'s verification
(`shape(candidate) == target`) relies on. The positioned written-unit API inserts its implicit
ZWJs by position and then folds them the same way, so `[B:medi]` encodes as bare `b`, while
`[N:medi]` keeps both joiners.

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

1. **shape** the input into its written-unit sequence. MVS and nirugu appear verbatim as PascalCase `Mvs` / `Nirugu` tokens, and a ZWJ as `Zwj` where it changes the ink (see "Redundant ZWJ" above; nirugu renders a visible stem; all three are the evidence for a neighbour's init/medi/fina form). **Split** the shape at these tokens into *chains*; the tokens themselves are copied through unchanged.
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

The exact canonical selection policy is frozen as **`mng-canonical/3`**. It is available as
`Shaper::canonical_version` (Python: `shaper.canonical_version`) and embedded in
`MNG.normalize.json`. Applications that persist normalized search/index keys should store this
version alongside them and rebuild those keys if a future release changes it.

**`mng-canonical/3` invalidates keys stored under `mng-canonical/2` only where the text contains a
ZWJ.** Dropping redundant ZWJs (above) changes no canonical text without a ZWJ: of the 1990
`mng-canonical/2` golden representatives, two merge into groups that already existed
(`A A R A Zwj O A` into `A A R A O A`, `Zwj O A` into `O A`) and every other vector is unchanged,
leaving **1988** groups. Keys stored for texts that contain a ZWJ must be rebuilt; all other keys
are stable.

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
| `tests/golden/mng-canonical-v1.jsonl` | 1988 canonical vectors |
| `tests/golden/mng-phase-trace-v1.json` | 15 phase-trace vectors |

Because the corpus and golden tests read that directory, `cargo test` needs a repository checkout —
the published crate does not include the fixtures.
