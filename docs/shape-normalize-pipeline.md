# Shape-to-Normalize Pipeline

This document describes the internal pipeline that powers `shape()`, `same_shape()`, `normalize()`, and `normalize_text()` in `shaper.py`. It is intended for contributors, integrators, and anyone trying to understand how Mongolian encoding ambiguity is resolved.

For installation and quick-start usage, see the [README](../README.md).

---

## Overview

Traditional Mongolian Unicode has a many-to-one problem: multiple codepoint sequences can produce the same visual word. The pipeline resolves this by:

1. **Shaping** the input into an abstract glyph sequence (written units).
2. **Reverse-mapping** those glyphs back to a single canonical Unicode sequence.

```
  Input Unicode        shape()          Written units       normalize()        Canonical Unicode
 ┌──────────────┐    ┌──────────┐    ┌───────────────┐    ┌─────────────┐    ┌──────────────────┐
 │ ᠰᠡᠢᠨ          │ →  │ 5 steps  │ →  │ S A I I A     │ →  │ harmony +   │ →  │ ᠰᠠᠢᠨ              │
 │ (S+E+I+NA)   │    │ + resolve│    │               │    │ reverse map │    │ (S+A+I+NA)       │
 └──────────────┘    └──────────┘    └───────────────┘    └─────────────┘    └──────────────────┘
```

---

## The Four Public Methods

| Method | Input | Output | Purpose |
|--------|-------|--------|---------|
| `shape(text)` | One Mongolian word | `list[str]` of written-unit names | Forward shaping: Unicode → glyph sequence |
| `same_shape(a, b)` | Two Mongolian words | `bool` | Visual identity test: do `a` and `b` render identically? |
| `normalize(text)` | One Mongolian word | `str` canonical Unicode | Many-to-one normalization via shape + vowel harmony |
| `normalize_text(text)` | Sentence / paragraph / mixed script | `str` normalized text | Per-word normalization preserving spaces and non-Mongolian text |

---

## shape(): Forward Shaping

`shape()` converts a Unicode string into a list of abstract glyph names called **written units**. Two strings that produce the same written-unit list look identical when rendered.

### Pipeline Steps

```
  text
   │
   ▼
 tokenize()          ← Split into Token objects; group letter + trailing FVS
   │
   ▼
 assign_positions()  ← Mark each letter as isol / init / medi / fina
   │
   ▼
 Step 1: Chachlag    ← MVS suffix forms for a/e
   │
   ▼
 Step 2: Syllabic    ← Consonant/vowel context (onset, devsger, marked, harmony, dotless)
   │
   ▼
 Step 3: Particle    ← MVS particle dictionary override
   │
   ▼
 Step 4: Devsger     ← i after vowel → double-tooth (vowel_devsger)
   │
   ▼
 Step 5: Post-bowed  ← Vowels after bowed consonants (G, K, B, P, F)
   │
   ▼
 resolve_written()   ← condition + position + FVS → written units from variant data
   │
   ▼
 flatten             ← Collect all written units into a single list
```

### Tokenization

Each Mongolian letter becomes one `Token`. A trailing FVS (U+180B..U+180D, U+180F) is attached to its preceding letter. MVS (U+180E) and NNBSP (U+202F) become MVS tokens (NNBSP is normalized to MVS at this earliest stage). Non-Mongolian characters are skipped.

### Position Assignment

| Letters in word | Positions assigned |
|---|---|
| 1 | `isol` |
| 2 | `init`, `fina` |
| 3+ | `init`, `medi` (repeated), `fina` |

Position determines which glyph variant a letter uses. This is analogous to Arabic initial/medial/final forms.

### The 5 Shaping Steps

Each step assigns a **condition** string to tokens that need a non-default glyph variant. Steps run in order; once a token has a condition, later steps skip it.

#### Step 1 -- Chachlag

| Trigger | Condition assigned | Effect |
|---|---|---|
| `a` or `e` immediately after MVS (no explicit FVS) | `chachlag` | Suffix connection form for a/e after a stem boundary |

Mongolian uses MVS (Mongolian Vowel Separator) to mark the boundary between a stem and its suffix. The a/e immediately after MVS takes a special joining form.

#### Step 2 -- Syllabic

The largest step. It examines each letter's neighbors to determine syllabic role.

| Letter(s) | Context | Condition | Meaning |
|---|---|---|---|
| `o`, `u`, `oe`, `ue` | After initial consonant | `marked` | Tall-stem vowel form (vs. short-stem default) |
| `oe`, `ue` | After consonant cluster from init | `marked` | Same, for deeper clusters |
| `d` | Before final vowel (no FVS) | `marked` | Different stroke form |
| `n`, `j`, `w` | Before MVS + isolated `a`/`e` | `chachlag_onset` | Pre-suffix consonant form |
| `h`, `g` | Before MVS + isolated `a` | `chachlag_onset` | Pre-suffix consonant form |
| `g` | Before MVS + isolated `e` | `chachlag_onset` | Pre-suffix consonant form (GA variant) |
| `n`, `t`, `d` | Before vowel | `onset` | Syllable-initial tooth direction |
| `n`, `t`, `d` | After vowel | `devsger` | Syllable-connecting tooth direction |
| `h`, `g` | Next is masculine vowel (`o`/`u`) | `masculine_onset` | QA form |
| `h`, `g` | Next is feminine/neuter vowel | `feminine` | GA form |
| `h`, `g` | Previous is masculine vowel | `masculine_devsger` | QA form (post-vowel) |
| `h`, `g` | Previous is feminine vowel | `feminine` | GA form (post-vowel) |
| `h`, `g` | No adjacent vowel (remote scan) | `masculine_devsger` or `feminine` | Scans entire word for unambiguous vowel |
| `h`, `g` | No vowel found anywhere | `feminine` | Default fallback |
| `t` | Before `ee` or consonant | `devsger` | Connecting form |
| `sh` | Before `i` (init or medi) | `dotless` | Dot omitted to avoid collision |
| `g` | After `s` or `d` | `dotless` | Dot omitted |

#### Step 3 -- Particle

If the entire word's alias sequence matches a known particle in the MVS particle dictionary, specific tokens receive the `particle` condition, **overriding** any syllabic condition from step 2.

Affected letters: `a`, `e`, `i`, `u`, `ue`, `d`, `y`.

#### Step 4 -- Devsger

| Trigger | Condition | Effect |
|---|---|---|
| `i` in `medi` position, after a vowel, no existing condition or FVS | `vowel_devsger` | Renders as double-tooth: written `('I', 'I')` instead of `('I',)` |

This is checked only when the preceding vowel's written does not already end with `I` (preventing triple-tooth).

#### Step 5 -- Post-bowed

| Trigger | Condition | Effect |
|---|---|---|
| `o`, `u`, `oe`, `ue`, `a`, `e` after a bowed consonant | `post_bowed` | Modified vowel connection form |

**Bowed consonants** are those whose final stroke curves rightward:

| Bowed written units |
|---|
| `G`, `Gx`, `K`, `K2`, `B`, `P`, `F` |

### Written Resolution

After all 5 steps, each token's written units are resolved using this priority:

| Priority | Source | Example |
|---|---|---|
| 1 | Explicit FVS on the token | User wrote `YA+FVS1` → use FVS1 variant |
| 2 | Condition from 5-step pipeline | `vowel_devsger` → find which FVS has that condition |
| 3 | Default variant (bare letter) | No FVS, no condition → use the variant marked `default: true` |

The variant data comes from `mongfontbuilder`'s `variants.json`, which encodes the complete letter x position x FVS → glyph mapping from UTN #57 v4.

---

## same_shape(): Visual Identity Comparison

```python
def same_shape(self, text1, text2):
    return self.shape(text1) == self.shape(text2)
```

Two strings are visually identical if and only if they produce the same written-unit sequence. This is the foundation that makes normalization possible: if `shape(a) == shape(b)`, then `a` and `b` must normalize to the same canonical form.

---

## normalize(): Canonical Encoding

`normalize()` takes a single Mongolian word and returns its canonical bare-Unicode encoding. The core principle is **minimal encoding**: output only letter codepoints, no FVS, since the shaping engine will automatically select the correct default variant from context.

### Algorithm

```
  tokens = tokenize + assign_positions
   │
   ▼
  Run 5-step pipeline (same as shape())
   │
   ▼
  Resolve all written units
   │
   ▼
  Detect vowel harmony          ← masculine / feminine / default
   │
   ▼
  Build segments                ← (type, written_tuple, original_alias)
   │
   ▼
  Merge adjacent identical      ← e.g. ('I',) + ('I',) → ('I','I')
  single-unit segments             (two YA+FVS1 tokens → one 'i' devsger)
   │
   ▼
  Re-derive positions           ← merging may change letter count
   │
   ▼
  For each segment:
    get candidates              ← all (cp, fvs) that produce this written at this position
    pick_by_harmony             ← select canonical letter using harmony + original preservation
    output bare codepoint       ← chr(cp), no FVS
```

### Vowel Harmony Detection

Mongolian vowel harmony constrains which vowels can co-occur in a native word:

| Harmony class | Unambiguous vowels | Ambiguous pairs affected |
|---|---|---|
| Masculine | `o`, `u` | `a`/`e` → `a`; `h`/`g` → `h` (QA) |
| Feminine | `oe`, `ue`, `ee` | `a`/`e` → `e`; `h`/`g` → `g` (GA) |
| Neuter | `i` (appears in either) | No resolution by itself |

Detection priority:

| Priority | Method | Result |
|---|---|---|
| 1 | Word contains `o` or `u` (and no `oe`/`ue`/`ee`) | Masculine |
| 1 | Word contains `oe`, `ue`, or `ee` (and no `o`/`u`) | Feminine |
| 1 | Word contains both | First unambiguous vowel wins |
| 2 | `h`/`g` condition from step 2 (`masculine_onset`/`masculine_devsger` or `feminine`) | Use that class |
| 3 | No signal | Default to masculine |

### Harmony Pairs

These letter pairs produce identical glyphs in certain positions but represent different phonemes:

| Pair | Masculine choice | Feminine choice | Where they collide |
|---|---|---|---|
| `a` / `e` | `a` | `e` | Medial and final positions |
| `h` / `g` (QA / GA) | `h` | `g` | Multiple positions |

### Candidate Selection (`_pick_by_harmony`)

Given a set of candidate letters that all produce the same written unit at a position:

| Priority | Rule | Rationale |
|---|---|---|
| 1 | Original letter is a candidate AND not part of a harmony pair | Minimal-change preservation (e.g., keep `n` even if `a` also produces same glyph) |
| 1a | ...but only at word boundaries (`init`/`fina`/`isol`) when harmony pair exists | At boundaries, the original letter's identity is structural (e.g., NA@fina = consonant N) |
| 2 | Harmony pair present: pick masculine or feminine member by detected harmony | Core ambiguity resolution |
| 3 | Original letter is a candidate | Preserve original |
| 4 | Pick any default candidate | Fallback |

### Segment Merging

Before candidate selection, adjacent segments with identical single written units are merged:

| Before merge | After merge | Canonical letter |
|---|---|---|
| `('I',)` + `('I',)` | `('I', 'I')` | `i` (vowel_devsger form) |

This handles the case where two `YA+FVS1` tokens each produce a single tooth `I`, but together they represent the double-tooth devsger of letter `i`.

### Bare Encoding Output

The final output contains only letter codepoints and MVS markers -- no FVS. This works because the shaping engine's default variant selection already produces the correct glyph for bare letters in context.

---

## normalize_text(): Full-Text Normalization

`normalize()` operates on a single word -- it drops spaces and non-Mongolian characters during tokenization. `normalize_text()` wraps it for real-world text:

### Segmentation

The input is split into alternating runs:

| Segment type | Characters included | Processing |
|---|---|---|
| Mongolian | Letters (U+1820..U+18AF), FVS (U+180B..U+180D, U+180F), MVS (U+180E), NNBSP (U+202F) | `normalize()` each run independently |
| Non-Mongolian | Everything else (spaces, punctuation, Latin, digits, etc.) | Pass through verbatim |

### Properties

| Property | Guarantee |
|---|---|
| Word independence | Each Mongolian word is normalized independently; word A cannot affect word B |
| Space preservation | All whitespace characters between words are preserved exactly |
| Mixed-script safety | Latin, CJK, digits, punctuation pass through unchanged |
| Idempotence | `normalize_text(normalize_text(x)) == normalize_text(x)` |

---

## Worked Example: Five Encodings of "sain"

The word "sain" (meaning "good") is the classic demonstration of Mongolian encoding ambiguity. All five encodings below render identically but use different Unicode sequences:

| # | Encoding | Codepoints | How the I-tooth is produced |
|---|---|---|---|
| 1 | S + A + I + NA | `1830 1820 1822 1828` | I = letter I; NA@fina produces glyph A |
| 2 | S + E + I + NA | `1830 1821 1822 1828` | Same as #1 but E instead of A (identical medi glyph) |
| 3 | S + NA+FVS2 + I + I + NA | `1830 1828 180C 1822 1822 1828` | NA+FVS2@init produces glyph A; two explicit I letters |
| 4 | S + A + YA+FVS1 + I + NA | `1830 1820 1836 180B 1822 1828` | YA+FVS1@medi produces glyph I (single tooth) |
| 5 | S + A + YA+FVS1 + YA+FVS1 + NA | `1830 1820 1836 180B 1836 180B 1828` | Two YA+FVS1 tokens, each producing one I tooth |

### Shaping (all five)

All five produce the same written-unit sequence:

```
['S', 'A', 'I', 'I', 'A']
```

### Normalization

1. Vowel harmony: no `oe`/`ue`/`ee` present, default → **masculine**
2. `a`/`e` pair → resolves to `a`
3. Merged segments produce canonical positions
4. Output: `ᠰᠠᠢᠨ` = `S + A + I + NA` (bare Unicode, no FVS)

```python
shaper = MongolianShaper(locale="MNG")

# All five normalize to the same string
assert shaper.normalize("ᠰᠠᠢᠨ")       == "ᠰᠠᠢᠨ"   # encoding 1
assert shaper.normalize("ᠰᠡᠢᠨ")       == "ᠰᠠᠢᠨ"   # encoding 2
assert shaper.normalize("ᠰᠨ᠌ᠢᠢᠨ")     == "ᠰᠠᠢᠨ"   # encoding 3
assert shaper.normalize("ᠰᠠᠶ᠋ᠢᠨ")     == "ᠰᠠᠢᠨ"   # encoding 4
assert shaper.normalize("ᠰᠠᠶ᠋ᠶ᠋ᠨ")    == "ᠰᠠᠢᠨ"   # encoding 5

# Visual identity confirmed
assert shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ") == True
```

---

## NNBSP Handling

Narrow No-Break Space (U+202F) appears in some Mongolian text as an alternative to MVS (U+180E). The pipeline normalizes NNBSP → MVS at the earliest possible point (tokenization), so all downstream processing sees only MVS.

| Input character | Tokenized as | Output by normalize() |
|---|---|---|
| U+180E (MVS) | MVS token | U+180E |
| U+202F (NNBSP) | MVS token | U+180E |

---

## Key Invariants

| Invariant | Description |
|---|---|
| **Convergence** | If `same_shape(a, b)` then `normalize(a) == normalize(b)` |
| **Idempotence** | `normalize(normalize(x)) == normalize(x)` |
| **Visual preservation** | `same_shape(x, normalize(x))` is always `True` |
| **Bare encoding** | `normalize()` output never contains FVS codepoints |
| **NNBSP elimination** | `normalize()` output never contains U+202F |

---

## Additional Worked Examples

The "sain" example above demonstrates a/e ambiguity and YA+FVS1 merging. The examples below show other aspects of the pipeline.

### Example 2: Feminine Word — "üge" (word/speech)

```
Input:  ᠥᠭᠡ  (OE + GA + E)
```

| Step | Token | Alias | Position | Condition assigned | Why |
|---|---|---|---|---|---|
| Tokenize | `1825` | `oe` | — | — | OE letter |
| Tokenize | `182D` | `g` | — | — | GA letter |
| Tokenize | `1821` | `e` | — | — | E letter |
| Positions | `oe` | `oe` | `init` | — | First letter |
| Positions | `g` | `g` | `medi` | — | Middle |
| Positions | `e` | `e` | `fina` | — | Last letter |
| Step 2 | `g` | `g` | `medi` | `feminine` | Prev vowel OE is feminine → GA form |

**Shaping result**: `['OE', 'G', 'Aa']` (note: `Aa` is the final form of `e`)

**Normalization**:
- Vowel harmony: OE present → **feminine**
- `g` stays `g` (feminine member of h/g pair)
- Output: `ᠥᠭᠡ` (unchanged — already canonical)

### Example 3: Devsger — "ail" (village)

```
Input:  ᠠᠢᠯ  (A + I + L)
```

| Step | Token | Alias | Position | Condition | Why |
|---|---|---|---|---|---|
| Positions | `a` | `a` | `init` | — | First |
| Positions | `i` | `i` | `medi` | — | Middle |
| Positions | `l` | `l` | `fina` | — | Last |
| Step 4 | `i` | `i` | `medi` | `vowel_devsger` | `i` after vowel `a` → double-tooth |

**Shaping result**: `['A', 'I', 'I', 'L']` — note `i` expands to two `I` written units

**Normalization**: Output `ᠠᠢᠯ` — bare `i` in context produces double-tooth by default.

### Example 4: MVS Chachlag — stem + suffix

```
Input:  ᠶᠠᠪᠤ᠋ᠬᠤ  →  with MVS between stem and suffix
        (conceptual: yabu + MVS + a)
```

When MVS appears between a stem and a suffix starting with `a`/`e`:

| Step | What happens |
|---|---|
| Tokenize | MVS becomes MVS token (whether input is U+180E or U+202F) |
| Step 1 (Chachlag) | The `a`/`e` after MVS gets condition `chachlag` → suffix connection glyph |
| Step 2 (Syllabic) | Consonant before MVS may get `chachlag_onset` if followed by isolated `a`/`e` |
| Step 3 (Particle) | If the full word matches a particle pattern, override with `particle` condition |

### Example 5: Post-bowed — vowel after bowed consonant

When a vowel follows a bowed consonant (G, Gx, K, K2, B, P, F), the vowel takes a modified connection form:

```
Input:  ...B + U...  (B followed by U in medial position)
```

| Step | Token | Condition | Why |
|---|---|---|---|
| Step 5 | `u` | `post_bowed` | Previous letter's written ends with `B` (a bowed unit) |

The `post_bowed` condition selects a glyph variant where the vowel's leading stroke connects smoothly to the bow of the preceding consonant.

### Summary: Before/After Normalization

| Input (various encodings) | Canonical output | Harmony | Key resolution |
|---|---|---|---|
| `ᠰᠡᠢᠨ` (S+E+I+NA) | `ᠰᠠᠢᠨ` (S+A+I+NA) | masculine (default) | E→A (a/e pair, masculine) |
| `ᠰᠠᠶ᠋ᠶ᠋ᠨ` (S+A+YA×2+NA) | `ᠰᠠᠢᠨ` (S+A+I+NA) | masculine | YA+FVS1 merged → I |
| `ᠰᠨ᠌ᠢᠢᠨ` (S+NA+FVS2+I+I+NA) | `ᠰᠠᠢᠨ` (S+A+I+NA) | masculine | NA+FVS2→A glyph, reverse-mapped to A |
| `ᠥᠭᠡ` (OE+GA+E) | `ᠥᠭᠡ` (unchanged) | feminine (OE present) | Already canonical |
| `ᠠᠢᠯ` (A+I+L) | `ᠠᠢᠯ` (unchanged) | masculine (default) | Already canonical |

---

## Terminology Glossary

| Term | Meaning |
|---|---|
| **Written unit** | An abstract glyph name (e.g., `A`, `I`, `S`, `G`, `OE`). The atomic visual building block. Two strings with the same written-unit sequence look identical when rendered. |
| **FVS** (Free Variation Selector) | Unicode codepoints U+180B–U+180D, U+180F that follow a letter to select a non-default glyph variant. Root cause of encoding ambiguity. |
| **MVS** (Mongolian Vowel Separator) | U+180E. Marks the boundary between a word stem and its grammatical suffix. Triggers chachlag forms and particle matching. |
| **NNBSP** (Narrow No-Break Space) | U+202F. Some text uses this instead of MVS. Normalized to MVS at tokenization. |
| **Chachlag** | The suffix connection form that `a`/`e` takes when appearing immediately after MVS. |
| **Devsger** | A "connecting" consonant form between syllables (e.g., `n` between two vowels). For `i` after a vowel, it means the double-tooth form `('I', 'I')`. |
| **Onset** | A consonant that begins a syllable (before a vowel). |
| **Post-bowed** | A vowel form used after bowed consonants (G, K, B, P, F) to connect smoothly to the preceding bow stroke. |
| **Vowel harmony** | Phonological rule: native Mongolian words contain either masculine vowels (o, u) or feminine vowels (oe, ue, ee), never both. Neuter `i` appears in either class. |
| **Masculine** | Harmony class containing `o`, `u`. Ambiguous `a`/`e` resolves to `a`; ambiguous `h`/`g` resolves to `h` (QA). |
| **Feminine** | Harmony class containing `oe`, `ue`, `ee`. Ambiguous `a`/`e` resolves to `e`; ambiguous `h`/`g` resolves to `g` (GA). |
| **QA / GA** | The two contextual identities of Unicode codepoints U+182C (`h`) and U+182D (`g`). QA is the masculine/back-vowel variant; GA is the feminine/front-vowel variant. They share glyph forms in several positions. |
| **Bare encoding** | Unicode text containing only letter codepoints and MVS — no FVS. The canonical output format of `normalize()`. |
| **Reverse map** | A lookup from `(position, written_tuple)` → list of `(codepoint, fvs)` candidates. Used by `normalize()` to find which letters can produce a given glyph. |
| **Particle** | A Mongolian grammatical suffix (小品词) preceded by MVS, with a fixed glyph form defined in the particle dictionary. |
| **Position** | One of `isol` (isolated), `init` (initial), `medi` (medial), `fina` (final) — determines which glyph variant a letter uses. |
| **Condition** | A string label (e.g., `chachlag`, `onset`, `devsger`, `marked`, `feminine`) assigned by the 5-step pipeline, selecting a non-default glyph variant. |

---

## Data Flow Diagram

```
                          mongfontbuilder (PyPI)
                                │
                   ┌────────────┼────────────────┐
                   │            │                 │
              variants.json  aliases.json    locales.json    particles.json
              (letter×pos×   (cp→alias)      (vowel/cons    (MVS particle
               FVS→glyph)                    categories)     dictionary)
                   │            │                 │               │
                   └────────────┼────────────────┘               │
                                │                                │
                         MongolianShaper.__init__()               │
                                │                                │
              ┌─────────────────┼──────────────────┐             │
              │                 │                  │             │
         variant_lookup    cp_to_alias        vowels/         particle_dict
         default_variant   alias_to_cp        consonants          │
              │                 │             masc/fem/neut       │
              │                 │                  │             │
              └─────────────────┼──────────────────┘             │
                                │                                │
                          shape() / normalize()                  │
                          steps 1-5 use all of the above ────────┘
```
