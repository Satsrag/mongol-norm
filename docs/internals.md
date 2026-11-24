# How mongol-norm works

Background for contributors and porters. Users only need the [README](../README.md).

> **Beta.** `shape` is considered stable. The `normalize` output (the `mng-canonical/5` policy
> described below) encodes by glyph shape, not the standard phonetic (nominal-character) spelling —
> e.g. ᠮᠣᠩᠭᠣᠯ (`MA+O+ANG+GA+O+LA`) → ᠮᠣᠠᠭ᠌ᠨ᠋ᠨ᠋ᠣᠯ (`MA+O+A+GA+FVS2+NA+FVS1+NA+FVS1+O+LA`) — and may change in a later release.

The normalizer implements a **lightweight Mongolian shaping engine** — equivalent to what HarfBuzz
does with a font file, but using only the rule data from
[UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) and the
[mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) project. No font files needed.

## Why encodings differ

The same visible Traditional Mongolian (Hudum) word can be typed as many Unicode sequences.
A and E look alike in medial and final position; O/U and OE/UE share written forms; Free
Variation Selectors (FVS) and joining controls (nirugu, ZWJ, ZWNJ) can produce the same
glyph in several ways. Public `shape` fingerprints that visible word; `normalize` maps each
shape to one Unicode string. Control-character filtering and the stored-key policy are in
the sections below.

## Shaping pipeline (UTN #57 v4 Mongolian-specific phase)

1. **Chachlag** — Chachlag forms for A/E after MVS (Mongolian Vowel Separator)
2. **Syllabic** — Consonant/vowel context: onset, devsger, marked, masculine/feminine harmony, dotless
3. **Particle** — MVS particle dictionary lookup for specific suffix words
4. **Devsger** — I after a vowel (vowel_devsger) gets double-tooth form: `I → I+I`
5. **Post-bowed** — Vowel forms change after bowed consonants (G, B, K, P, F)

## Control-character post-processing

After raw shaping, public shape (`Shaper::shape`, `FontShaper::shape`, `canonical_units`,
`canonical_positioned_units`, `trace.shape`) runs a three-stage, **expansion-only** pipeline:

```text
Shaping:       raw ─► expand_pre (E1–E5) ─► canonical_controls ─► expand_post (E6–E9)
Normalization: shape ─► contract (C1–C4) ─► encode & verify (unchanged MNG.normalize.json)
```

First, `expand_pre` expands single-unit duplicates (`Dd`, `Cr`, medial `H`, medial `Hx`). Next,
`canonical_controls` runs complete control rounds: merge consecutive identical `Zwj`, `Zwnj`, and
`Mvs`; run the complete ZWJ filter; then scan ZWNJs once from left to right. Repeat until a
complete round makes no change. Each changing round removes at least one control, so finite inputs
terminate. Nirugu count and order are preserved. Finally, `expand_post` expands the four
chain-final duplicate units (`G:fina`, `B2:fina`, `<bowed> + Aa:fina`, standalone `A:isol`) once,
leaving lone `Zwj+G` / `Zwj+B2` intact so no new redundant control is ever introduced (no outer
fixpoint loop is needed).

Normalization alone owns contraction (`src/contract.rs`): right before table lookup, it applies
the exact inverse tail-pair contractions (`C1–C4`) so `MNG.normalize.json` and canonical Unicode
output (`mng-canonical/5`) stay 100% byte-identical.

For each ZWNJ, simulate removing only that unit using the current sequence. MVS and ZWNJ
break joining chains; ZWJ and Nirugu provide joining context. Remove the break only when
all affected letters retain their positions or pass their reviewed per-unit positional
equivalences. An adjacent MVS supplies the same break, so that ZWNJ can be removed.
Four reviewed ZWNJ tables (`INIT_TO_MEDI`, `FINA_TO_MEDI`, `ISOL_TO_FINA`, and
`ISOL_TO_INIT` in `src/redundant_zwnj.rs`) supply `init ↔ medi`, `fina ↔ medi`,
`isol ↔ fina`, and `isol ↔ init`;
only approved paths for that same unit establish further equivalence. Missing evidence
keeps the ZWNJ. A sequence consisting only of `[Zwj]` or `[Zwnj]` keeps that representative.

Earlier kept breaks are retried in the next round. For example,
`N+Zwnj+B+Zwnj+I` becomes `N+Zwnj+B+I` in the first ZWNJ scan, then `N+B+I` in the next.
Likewise `Mvs+Zwnj+Mvs` becomes `Mvs+Mvs`, and only the next round merges the MVS run.
The positioned-unit API validates the original records before inserting implicit ZWJs
and applying the shared policy. Raw shaping, detailed records, and trace tokens retain
their original semantics.

## Redundant ZWJ

UTN #57 v4 treats ZWJ (U+200D) purely as a cursive joining control: it forces its neighbours into
their joined (`init` / `medi` / `fina`) forms. (v1/v2 also used a mid-word ZWJ as a *word root
delimiter* that changed the following vowel; v3 withdrew that, and so did the reference font.)
Where the neighbours already join, or where the joined form is the same ink as the unjoined one,
the ZWJ is invisible — yet kept as a `Zwj` token it broke "same shape ⟹ same `normalize`" (ᠯᠢᠡ᠋
is `L I Aa`, ᠯᠢ + ZWJ + ᠡ᠋ was `L I Zwj Aa`, and ᠪ‍ᠠ᠋ was `B Zwj Aa` instead of `B A Aa`, because
the token also stopped post-control duplicate expansion from seeing the adjacent letters). Issue #33
has the full study.

`shape` therefore drops every ZWJ that changes no glyph. The filter (`src/redundant_zwj.rs`) runs
on the written-unit sequence after the rules resolved every letter and `expand_pre` (E1–E5) ran,
and **before** `expand_post` (E6–E9) — so dropping a redundant joiner or break automatically lets
`expand_post` expand exposed pairs (`ᠪ‍ᠠ᠋` / `ᠪ‌ᠠ᠌` → `B Aa` → `B A Aa`) — and it is
the same function for `shape`, `trace`, `canonical_units`, `FontShaper::shape`, and
`normalize_written_units`, so `[L, I, Zwj, Aa]` and the
text `ᠯᠢ‍ᠡ᠋` are one contract. `shape_raw`, `shape_detailed` and the trace tokens keep every
ZWJ of the input. Everything below is about written units: a *letter unit* is any written unit
other than the structural `Mvs`, `Nirugu`, `Zwj` and `Zwnj` (`WrittenUnit::is_structural`), and `U` names
one such unit.

| ZWJ context | public shape |
|---|---|
| `Zwj Zwj` | one copy (then judged like a single one) |
| next to a `Nirugu`, either side | dropped — the nirugu already joins |
| between two letter units (segment interior) | dropped |
| segment-initial (word start or directly after an `Mvs`) before `U` + more letters (`U:medi` → `U:init`) | dropped for `B C Ch D F G Gx I K K2 O P R Rh S Sh T W Y Z Zr` |
| segment-initial before a lone `U` (`U:fina` → `U:isol`) | dropped for `Aa I U Zr` |
| segment-final (word end or directly before an `Mvs`) after letters + `U` (`U:medi` → `U:fina`) | dropped for `O Zr` |
| segment-final after a lone `U` (`U:init` → `U:isol`) | dropped for `B C Ch D F G Gx H Hx K K2 L M N P R Rh S Sh T W Y Z Zr` |
| no letter unit on either side (`Zwj Mvs…`, `…Mvs Zwj`, `Mvs Zwj Mvs`) | dropped — it has nothing to join |
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
side; segments are split at MVS exactly as the position assignment splits them.

An MVS is a segment edge on both sides, and both sides behave like the word edge. The font gives
the letter before an MVS its `fina` (or `isol`) form, so `ᠠᠣ‍᠎ᠠ` vs `ᠠᠣ᠋᠎ᠠ` is the very
`O:medi`/`O:fina` pair of the word-final row and the same lists decide it (`ᠪ‍᠎ᠠ` = `ᠪ᠎ᠠ`,
`ᠠᠯ‍᠎ᠠ` keeps its `Zwj`). The letter after an MVS takes its `init` (or `isol`) form, and on the
reference font every after-MVS pair has exactly the letter glyphs of its word-initial twin. The
only extra difference is the MVS itself: followed by a joiner it is drawn as the font's dashed
"MVS" placeholder box (an MVS no rule handles), otherwise as a plain gap. The box is not text
ink, so the word-initial lists decide: `ᠠ᠎‍ᠶᠢᠨ` = `ᠠ᠎ᠶ᠌ᠢᠨ` (`Y:medi` ≡ `Y:init`), while
`ᠲᠠᠯ᠎‍ᠠ` keeps its `Zwj` (`A:fina` is not the suffix `a`). A ZWJ whose only neighbours are MVS
or the word boundary joins nothing at all: on the reference font every such pair renders the
same glyphs at the same positions, MVS included, so it is dropped like the other structural
cases.

The pass is a fixed point of left-to-right single removals: `Zwj B Zwj` first loses the leading
joiner (`B:medi` ≡ `B:init`), then — now a lone `B:init` — the trailing one (`B:init` ≡ `B:isol`).
Applying it to its own output changes nothing, which is what `normalize`'s verification
(`shape(candidate) == target`) relies on. The positioned written-unit API inserts its implicit
ZWJs by position and then folds them the same way, so `[B:medi]` encodes as bare `b`, while
`[N:medi]` keeps both joiners. A `Zwnj` (below) is a segment edge for every rule in the table,
exactly like an `Mvs`.

## ZWNJ: an in-word joining break

A ZWNJ (U+200C) is a word character (issue #34). It breaks the cursive chain — the position
assignment splits segments at it as it does at an MVS, so the letter before it is written `fina`
(or `isol`) and the letter after it `init` (or `isol`) — and that is *all* it does to the
rendering: following the reference font (mongfontbuilder `hudum.otf`,
[Kushim-Jiang/mongolian#58](https://github.com/Kushim-Jiang/mongolian/issues/58)), phase III
substitutes the ZWNJ to an ignored glyph before any rule runs, and every rule lookup skips
ignored glyphs. The engine mirrors that: the context walks of vowel harmony, the gender markers,
chachlag, dotless g and the particle lookup look through a ZWNJ the way they look through a
nirugu (`Token::is_transparent`), while an MVS keeps blocking them. ᠨᠦ‌ᠬ᠊ (`n ue` + ZWNJ +
`h nirugu`) is `N Ue Zwnj G Nirugu`: the `ue` is final and the `h` initial, yet the `h` is the
feminine `G` the vowel across the break asks for — the font's `u182C.G.init`.

Where the font keys a lookup on joining positions, the ZWNJ changes the answer through the
positions it changed, not through the glyph:

- the forward gender markers (`iii.py` preprocessing A/B, D/E) move only onto `medi`/`fina`
  glyphs, so they cross a break only when a joiner follows it; the backward chain (G–K) runs
  through letters of every position and skips the break, but needs the marker G hands only to
  `init`/`medi`/`fina` glyphs;
- an `init`/`isol` g/h after a vowel (possible only through a break) has no `masculine_devsger`
  variant and falls through to the "initial g/h before a consonant → feminine" rule;
- a particle pattern names every letter's position, so a particle follows the joining chain: it
  ends where a break ends the chain (`MVS i` + ZWNJ + `n` is the particle `I`), starts where one
  starts it (`a r nirugu` + ZWNJ + `ue ue`), and runs on where two ZWJs join across the break
  (`u` + ZWJ ZWNJ ZWJ + `u` is the particle `u u`);
- the vowel-devsger and `sh` dotless classes are `init`/`medi` vowels and `medi`/`fina` `i`,
  which a letter across a break is not;
- post_bowed reads a final or isolated consonant (one before a break) in the form it has before
  its FVS applies, since the font applies an FVS last: `a b` FVS1 + ZWNJ ZWJ + `e` takes
  post_bowed after the `B` that becomes `B2`, while an isolated g/h, which takes no gender, is the
  unbowed `Hx`/`H` whatever its FVS selects. GB's resets follow the font too: FVS1/FVS3 on a g/h
  at any position, FVS2/FVS4 before o/u only on a joined one;
- the lookups that need an MVS right beside a letter (chachlag, the chachlag onset, the dotless
  g, the MASC marker's `fina + MVS + a`) look through a run of ZWJ, nirugu and ZWNJ that holds a
  ZWNJ, and leave the rest to the letter's position: `a n` MVS ZWJ ZWNJ `a` has the chachlag
  onset, `a n` MVS ZWNJ ZWJ `a` does not (its `a` is final).

Each of these is written so that a word without a ZWNJ takes exactly the path it took before.

An FVS after a ZWNJ stays with the ZWNJ, as one after a joiner stays with the joiner: it selects
no variant, but the font's FVS-reading lookups skip the ZWNJ and read it as an FVS after the
letter before the break. So GB.A resets a `marked` o/u/ö/ü there (`t o` + ZWNJ + FVS3 is
`T U Zwnj`, like `t o` + FVS3), the initial d before that vowel loses its `marked` form (`d e` +
ZWNJ + FVS3 is `T A Zwnj`), after the isolated a/e that follows an MVS it postpones the chachlag
onset of the consonant before the MVS (`a j` NNBSP `a` + ZWNJ + FVS4 keeps `J`), and a particle
ends unmatched (`MVS ue` + ZWNJ + FVS3 is `Mvs A Ue Zwnj`). The `Zwnj` is a segment edge for the
ZWJ rules. The shared control pass removes it only under the reviewed positional rules;
a remaining break still splits letter chains for both post-control expansion and normalization
contraction (`L I Zwnj Aa` stays `L I Zwnj Aa`, while `l i e᠋` is `L I Aa` and encodes to `ᠯᠭ᠌`).
`shape_raw`, `shape_detailed` and the trace tokens keep every ZWNJ.

## Duplicate encodings

`shape` promises to be a fingerprint of the *visible* word — "same shape ⟹ same `normalize`" is
this crate's whole reason to exist. Nine written units break that promise: they render as exactly
the same ink as a sequence of other units, so two spellings of one word shaped differently.

All nine are unified in `shape` strictly by **expansion** (1 → 2, never merging), split into two
passes around `canonical_controls` (`src/expand.rs`):

### `expand_pre` (E1–E5, before `canonical_controls`)

| Rule | Duplicate | Expands to | Condition | Witness pair |
|---|---|---|---|---|
| E1 | `Dd:medi` | `O:medi A:medi` | Always (`Dd` only occurs at `medi` / `fina`) | ᠣᠳᠪᠣ / ᠠ᠋ᠣᠣᠠᠪᠣ᠋ |
| E2 | `Dd:fina` | `O:medi A:fina` | Always | ᠠᠷᠠᠳ / ᠠᠷᠠᠤᠠ |
| E3 | `H:medi`  | `A:medi A:medi` | `medi` only; keep `H:init` and `H:fina` | ᠪᠠᠭᠰᠢ / ᠪᠠᠠᠠᠰᠢ |
| E4 | `Hx:medi` | `N:medi N:medi` | `medi` only; keep `Hx:init` and `Hx:fina` | ᠠᠷᠭᠠᠯ / ᠠ᠋ᠠᠷᠨ᠋ᠨ᠋ᠠᠯ |
| E5 | `Cr`      | `O O`           | All positions (`isol` / `init` / `medi` / `fina`) | ᡂ᠊ / ᠤ᠋ᠤ᠊ |

### `expand_post` (E6–E9, after `canonical_controls`; chain-final only, `pad_right == 0`)

| Rule | Duplicate | Expands to | Condition | Witness pair |
|---|---|---|---|---|
| E6 | `G:fina` | `I Aa` | Preceded by a same-chain letter or `Nirugu` (not lone `Zwj+G`) | ᠯᠭ᠌ / ᠯᠢᠡ᠋ |
| E7 | `B2:fina` | `O Aa` | Preceded by a same-chain letter or `Nirugu` (not lone `Zwj+B2`) | ᠊ᠪ᠋ / ᠊ᠤᠠ᠋ |
| E8 | `<bowed> + Aa:fina` | `<bowed> + A + Aa` | Preceded immediately in the same chain by `<bowed>` | ᠪᠠ / ᠪᠠᠠ᠋ |
| E9 | `A:isol` | `A Aa` | Standalone `A` with no adjacent joiner (`pad_left == 0 && pad_right == 0`) | ᠡ / ᠡᠠ᠋ |

### `contract` (C1–C4, normalization layer only, `src/contract.rs`)

Before table lookup in `normalize`, the four chain-final expansions are contracted back to their
short canonical table forms (`pad_right == 0` only):

| Rule | Tail pair | Contracts to | Condition |
|---|---|---|---|
| C1 | `A + Aa` | `A:isol` | Entire chain is `A+Aa`, no adjacent joiner (`pad_left == 0`) |
| C2 | `<bowed> + A + Aa` | `<bowed> + Aa:fina` | Preceded immediately in the same chain by `<bowed>` |
| C3 | `O + Aa` | `B2:fina` | Merged unit lands at `fina` (`pad_left + len >= 3`) |
| C4 | `I + Aa` | `G:fina` | Merged unit lands at `fina` (`pad_left + len >= 3`) |

Position is part of every rule, and it is the *chain* position `normalize` already uses — slots
between structural units, with a nirugu or ZWJ neighbour padding the chain the way it pads the
rendering. That is why the `B2` and `G` witnesses use a leading letter or nirugu (making the unit
final), while lone `Zwj+G` and `Zwj+B2` stay unexpanded in `expand_post`: expanding them would turn
`Zwj+G` into `Zwj+I+Aa`, where `I` is in `INITIAL_BEFORE_LETTER` and would create a new redundant
`Zwj`. Forms outside a verified rule are left alone — initial and final `H`/`Hx`, `G:isol`,
`B2:isol`, and `A` with an adjacent joiner stay unchanged.

**Terminology.** UTN #57 uses **bowed written units** (圆头书写单位) and names the corresponding lookup **Post-bowed**; see [UTN #57 revision 4](https://www.unicode.org/notes/tn57/utn57-mong-4.pdf).

**Context.** The complete Hudum bowed written unit set is **B, P, F, G, Gx, K, K2**, not every consonant.
Sources: the [Hudum written-unit ligated variants](https://mongfontbuilder.pages.dev/hudum/),
[required ligatures](https://github.com/Kushim-Jiang/mongfontbuilder/blob/7d5fc1cdaf8210f675c16699a8eaeb71aa1e80ca/data/ligatures.ts)
(`BAa`, `PAa`, `FAa`, `GAa`, `GxAa`, `KAa`, `K2Aa`), and `src/rules.rs`'s post-bowed classes.
`Aa:fina` carries a tooth immediately after a bowed written unit and no tooth elsewhere. Thus
`B Aa → B A Aa` (E8) and `B A Aa → B Aa` (C2), while `N A Aa`, `A A Aa`, and `B A A Aa` stay
unchanged in both directions. Joiner padding is not a bowed written unit and never lets E8/C2 cross
structural tokens.

**Control invariance, idempotence, and exact bijection.**
1. **Control invariance:** for any control-canonicalized sequence `s`,
   `canonical_controls(expand_post(s)) == expand_post(s)` — all 8 ZWJ/ZWNJ tables stay untouched
   and no outer fixpoint loop is needed.
2. **Idempotence:** `expand_pre` emits only `O`, `A`, `N` and never turns `init`/`fina` into
   `medi`; `expand_post` emits only `I Aa`, `O Aa`, or `A Aa` at the end of a chain, none of which
   matches E1–E9. Hence `canonical_units(canonical_units(u)) == canonical_units(u)`.
3. **Exact bijection:** on every control-canonicalized sequence `s`,
   `contract(expand_post(s)) == contract(s)` and `expand_post(contract(s)) == expand_post(s)`.

All three properties are verified exhaustively in `src/expand.rs` and `src/contract.rs` over the
21-unit alphabet (all bowed units, expansion targets, and structural controls including `Zwnj`)
through length four (**204,205 inputs**), plus length five over the 11-unit tail-interaction
alphabet (**161,051 inputs**). Corpus written-unit re-encoding also reshapes to itself.

Expansion never manufactures a bowed context: ᠲᠡᠳ᠌ᠡ᠋ (`T A Dd Aa`) expands to `T A O A Aa`
(since `O` is not bowed, neither E8 nor C2 fires), and `B H Aa` expands to `B A A Aa`. Both
interactions are regression-tested.

Everything user-facing in shaping (`shape`, `shape_str`, `same_shape`, `canonical_units`,
`canonical_positioned_units`, `trace.shape`, `FontShaper::shape`) sees the expanded sequence; the
shaping modules (`shaper.rs`, `expand.rs`, `controls.rs`, `font.rs`) have zero dependency on
`contract`. `normalize_written_units` still accepts both expanded and raw/pre-v0.5.0 contracted
sequences as input, applies `canonical_units` then `contract`, and produces the exact same
canonical Unicode text. ᠠᠷᠠᠳ and ᠠᠷᠠᠤᠠ are one visible word, and now one shape and one canonical
text; ᠪᠠ, ᠪᠠᠠ᠋, ᠪ‍ᠠ᠋, and ᠪ‌ᠠ᠌ all shape to `B A Aa` and normalize to `ᠪᠠ᠋`.

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

1. **Read the target shape from the whole input and contract for encoding.** Resolve raw written units, apply the three-stage pure-expansion shaping pipeline (`expand_pre` → `canonical_controls` → `expand_post`), then contract the four chain-tail pairs (`contract`, C1–C4) at the normalization entry so table lookup sees the compact form (`norm_shape`). Surviving controls appear as `Mvs` / `Nirugu` / `Zwnj` / `Zwj`. Context rules may look through ZWNJ while reading the raw target.
2. **Split the contracted target at the remaining `Zwnj` units.** Encode each segment independently: encoding receives no context from across ZWNJ. This splits already-resolved canonical units, not the original Unicode text. A single `[Zwnj]` retains its empty segments and encodes as U+200C.
3. **Encode each segment's chains.** Split at its remaining structural tokens (`Mvs`, `Nirugu`, `Zwj`), which are copied through unchanged. Encode chains right-to-left, checking them against the context within this segment:
   1. **partition + table lookup** — the primary path. At each position take the single unit if the table has it (preferred — clean output), else the longest available multi-unit entry, and look up `(position, written-unit) → (letter, FVS)` in an FVS-pinned table. Each value renders its unit **regardless of neighbours**, so the result is a deterministic, O(N), prefix-stable function of the shape.
   2. **velar-feminine refinement** — a `G`/`Gx` velar's forward-coupled vowel (`a`/`o`/`u`) is swapped to its feminine partner (`e`/`oe`/`ue`) for clean output.
   3. **verify** — reshape the candidate via `norm_shape` in its segment context; accept only if it equals the contracted target chain shape.
   4. **no search fallback** — the table is total over the corpus (with FVS-first selection there are no gap chains left). If an out-of-corpus shape ever misses the table, normalization fails closed with the input and the uncovered public `shape` written-unit sequence. Callers may explicitly ask for the lenient variant to return the input unchanged (round-trip preserved, never a mis-encoding). A letter next to a joiner simply looks its unit up at the shifted (joined) position.
4. **Verify and rejoin the ZWNJ segments.** Each encoded segment must independently reshape (`norm_shape`) to exactly its contracted target segment. Reinsert one U+200C per `Zwnj`, then verify the complete result against the whole contracted target (which, by the `expand_post ↔ contract` bijection, is equivalent to `shape(canonical) == shape(input)`). If either check fails, strict normalization reports a fallback, lenient normalization returns the complete original word, and the unit API reports no canonical encoding.

**MVS suffix encoding** works from the suffix's already-parsed written units: encode them
standalone, reattach MVS, and verify in context. For example, `mvs y i n` shapes to
`Mvs I I A`; the suffix is encoded from `I I A` and still displays as `I I A` without MVS.
The isolate-`I` → `i+FVS1` spelling is pinned in the table itself. ZWNJ segmentation uses the
same principle of independently encoding the resolved shape.

**MVS chachlag encoding** is a separate rule: `Aa` directly after MVS is encoded as bare `a`
when that preserves the complete target, and as the table's isol-`Aa` spelling `a+FVS2`
when bare `a` would rewrite a J/W (including one across ZWNJ) into I/U. Ordinary chachlag
keeps the narrow MVS gap; J/W keeps the explicit FVS2 spelling. The existing MVS suffix and
chachlag rules remain inside each ZWNJ segment.

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

The exact canonical selection policy is frozen as **`mng-canonical/5`**. It is available as
`Shaper::canonical_version` (Python: `shaper.canonical_version`) and embedded in
`MNG.normalize.json`. Applications that persist normalized search/index keys should store this
version alongside them and rebuild those keys if a future release changes it.

**`mng-canonical/5` canonicalizes control units.** The complete-round policy above removes
redundant ZWNJs and merges identical ZWNJ/MVS runs in addition to the existing ZWJ rules.
A joining break is retained only when required by the reviewed unit-position rules, or
when `[Zwnj]` is the entire shape. For example, `[Zwnj,B,Zwnj,Zwnj]` folds to `[B]`, while
`[A,N,Zwnj,A,A]` retains its break. MVS-run merging follows the accepted N-dot equivalence;
Nirugu count and order never change. Of the 1988 `/4` golden representatives, three change by merging consecutive MVS units (merging `S A R Mvs Mvs Mvs Aa` into `S A R Mvs Aa`) and five `Cr` representatives expand to `O O` (merging `[Cr]` into `[O, O]`), leaving 1986 groups.
All other representative outputs and all unchanged-shape encodings remain byte-identical;
raw phase-trace fixtures are unchanged.

**Migration:** rebuild persisted shape keys, normalized search keys, and deduplication
indexes under `/5`. In particular, recompute words containing ZWNJ or consecutive MVS;
removing a break can expose another removable ZWJ or a post-control tail expansion (`E6–E9`). Retain
the original source text for regeneration, record `canonical_version()` with the new keys,
and do not compare keys from different policy versions as if they were interchangeable.
This change does not itself increment the package version or publish a release.

`mng-canonical/4` introduced whole-word target parsing followed by independent ZWNJ-segment
encoding. `/5` keeps that encoding and read-back contract, but applies it to the remaining
breaks after control canonicalization. Each surviving `Zwnj` encodes as U+200C. Every
segment must independently reshape to its target and the rejoined word must match the
complete canonical shape. The original nominal letters and FVS spelling need not survive.
The plain written-unit APIs accept `Zwnj`; the positioned API rejects explicit `Zwnj` (like
`Zwj`) and synthesizes breaks across non-joining sub-chain and `Nirugu` boundaries instead.

中文概括：塑造阶段采用三段式纯展开（先展开前置单单元同形 `E1–E5`，再按完整轮次归并和删除冗余控制符，最后执行链尾同形展开 `E6–E9`）；规范化阶段仅在查表编码前做 4 条链尾收缩（`C1–C4`），再按剩余 `Zwnj` 分段独立编码，规范 Unicode 输出保持 `/5` 策略 100% 字节不变。
MVS suffix 与 chachlag 仍分别编码；chachlag 在保形时用裸 `a`，必要时使用已有的 `a+FVS2`。

A representable `J`/`W` + MVS + `Aa` target no longer needs a second MVS or a retained ZWNJ
to encode. Isol `O` remains a genuine table gap: strict normalization reports a fallback,
lenient normalization preserves the complete original word, and unit input reports no
canonical encoding. There is no search fallback or partial result.

**`mng-canonical/3` (0.3.0 on crates.io, 0.3.1 on PyPI) invalidates keys stored under `mng-canonical/2` (0.2.x) for some
texts.** Two kinds of change reach `normalize`:

- **Dropping redundant ZWJs** (above). Of the 1990 `mng-canonical/2` golden representatives, two
  merge into groups that already existed (`A A R A Zwj O A` into `A A R A O A`, `Zwj O A` into
  `O A`). Every other vector is unchanged, which leaves **1988** groups.
- **The shaping fixes made while aligning the engine with the reference font** (#40–#52, #55). They
  change the shape, and so the key, only of rare input:
  - an FVS on a medial o, u, ö or ü (it now resets `marked`, as it does at the word end);
  - an FVS in an unusual place: an invalid one, a second one, one after an MVS, a ZWJ or a
    nirugu, or one on a vowel after an MVS;
  - a ZWJ or nirugu at a word or MVS edge, and two MVS in a row;
  - a particle followed by a joiner;
  - two spots in plain text: g after a word-initial s or d before a masculine vowel (`s g u s`),
    and a final vowel after ŋ.

  None of the 1988 golden vectors changes.

Rebuild keys stored for texts that contain a ZWJ, a nirugu, an FVS or two MVS in a row, a
word-initial s or d before g, or ŋ before a final vowel. All other keys are stable.

**`mng-canonical/2` (0.2.0) invalidates keys stored under `mng-canonical/1`.** Unifying the nine
duplicate encodings changes canonical text: comparing current output against the base branch's
1993 `mng-canonical/1` golden representatives finds **287** changed texts; three verified pairs
merge into **1990** groups. Rebuild any stored normalized key. The context correction also
invalidates pre-fix PR #26 output; this is not a new release or a claim that old test suites were rerun.

## The font backend (feature `font`)

`FontShaper` is a second implementation of `shape`, with the rules taken from the reference font
instead of ported by hand. [harfrust](https://github.com/harfbuzz/harfrust) (the Rust port of
HarfBuzz) runs the Hudum font that mongfontbuilder composes for its own tests, with the OpenType
rules the composer generates, and the crate reads the written units off the glyphs the font chose:
mongfontbuilder names every glyph after what it draws (`u182A_u1823.BO.init` = the ligature of b
and o, written units `B O`, initial form), so the glyph order *is* the glyph → unit mapping. The
font shows exactly the written units of the rule data — the generator refuses a font that brings
another — so `WrittenUnit` needs nothing of its own for it.

What ships: `fonts/hudum.otf`, the font stripped of its outlines, metrics, names and GPOS (GSUB,
GDEF, cmap and the sfnt skeleton remain: 30 KB, 976 glyphs); `fonts/hudum.glyphs.txt`, its glyph
order; and `src/generated/font_hudum.rs`, the mapping as a static table. Nothing of mongfontbuilder
itself. `fonts/mongfontbuilder.json` pins the upstream commit the font was built at and the
checksums; `python/scripts/gen_font_shaper.py` rebuilds all of it from a clone of that commit (see
[`development.md`](development.md#updating-the-bundled-shaping-font)).

Shaping a word: validate the characters as the rule engine does, push them into a HarfBuzz buffer
one cluster per character, shape under the font's `MNG` language system, map each glyph through the
table, then run the same post-passes as the rule engine — the control-character rounds and duplicate
unification — so `FontShaper::shape` and `Shaper::shape` are comparable unit for unit. A
character of the block the font has no glyph for (a letter of another writing system, an Ali Gali
sign, an unassigned code point) is `Error::UnsupportedChar`, where the rule engine shapes it to
nothing. The reading of the controls follows the rule engine: an FVS is never a
unit (the font draws an invalid one as a visible placeholder; the standard's unit streams do not
list it), an idle MVS (drawn as a space plus a silent glyph) is one `Mvs`, NNBSP is an `Mvs`, a
nirugu is verbatim, and so is a ZWNJ (`zwnj`, or `zwnj.ignored` once phase III has made it a mark
the lookups skip). The one HarfBuzz artefact to undo: a ZWJ between two letters the font ligates
comes out after the ligature; it is recognised by its character index against the ligature's
letters and dropped from the public shape, as the redundant-ZWJ pass drops any joiner between two
joined letter units.

The two backends disagree exactly where the font and the ported rules do. On every fixture word
(the Hudum corpora and goldens) the only disagreement is `e n fvs3 mvs e` (EAC `XIM11-46`): the
font draws the form the FVS3 selects, the EAC suite and the engine read the FVS3 as invalid —
mongfontbuilder marks the row as an expected failure (`tests/font_vs_engine.rs` pins it). On
random strings the backends part on about 0.7% of inputs, in edge rules the corpora do not reach
(`FontShaper` is what the font draws there); `examples/compare.rs` lists them (see
[`development.md`](development.md#comparing-the-two-shapers)).

Cost: harfrust and its crates (18), Rust 1.85, ~4.4 µs per word against ~0.8 µs for the rule
engine (release, Apple silicon), and several hundred KB more wasm. The default build has none of
it.

## Data and fixtures

The shaping and normalize rules are flat, language-agnostic JSON in `python/mongol_norm/data/`
(`MNG.json` and `MNG.normalize.json` — Hudum is the only locale). Nothing reads it at
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
| `tests/golden/mng-canonical-v1.jsonl` | 1986 canonical vectors |
| `tests/golden/mng-phase-trace-v1.json` | 15 phase-trace vectors |
| `tests/data/mongfontbuilder/*.tsv` | the font backend's suites: mongfontbuilder's `core-hudum` and `eac-hudum` at the pinned commit |

Because the corpus and golden tests read that directory, `cargo test` needs a repository checkout —
the published crate does not include the fixtures.
