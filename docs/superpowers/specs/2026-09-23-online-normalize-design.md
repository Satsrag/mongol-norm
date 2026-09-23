# Online (prefix-committed) normalize encoder — research and design

Date: 2026-09-23 · Status: **implemented** (branch `feat/online-normalize`, reviewed in the working
tree, then opened as a pull request) — decisions in §9 resolved as recommended, strict
prefix-stability kept, no `standardize` (§10). What the implementation changed relative to this
design: §11 and the plan's execution notes (`docs/superpowers/plans/2026-09-23-online-normalize.md`).

> **中文摘要**
>
> **问题：** 现在的 `normalize`（`mng-canonical/2`）为了前缀稳定，对每个 (位置, 书写单元) 都选一个
> "对左右任意邻居都上下文无关" 的 FVS 钉死编码，并且优先单单元切分；每个词还要整形校验 2~3 次。结果：
> 语料 1990 个 shape 组平均 **8.46** 码位（每组 2.25 个 FVS），比理论最短（5.83）长 45%；速度
> 2.7~2.9 µs/词，是 `shape` 的 5 倍。
>
> **关键洞察：** 前缀稳定 ⇔ 编码可以**在线**进行、最多只悬置最后一个字母：除最后一个字母外，所有
> 字母一旦输出（"提交"）就永不改写。于是
> 1. 只需要对**右侧**（未来）稳健——左侧上下文在提交时已完全已知，现在的电池同时要求左侧无关是过度约束；
> 2. 最后一个字母从不提交，可以用最短的裸形 / 多单元字母（`a`=A+A、`o`=A+O、`ng`=A+G、元音后的 `i`=I+I…）；
> 3. 编码器自己决定下一个字母，所以可以对它**做承诺**（例如裸 `n` 表示 N 时承诺下一个字母是元音），
>    让 n/t/d/g/h 这类只依赖下一个字母类别的裸形也能提交。
>
> **算法：** 按 shape 从左到右读单元；尾部能成为一个末字母就先悬置（懒提交）；不能时，用一个极小的规划
> （通常 1~2 个单元）提交最便宜的"右侧稳健"字母；最后输出最便宜的末字母。前缀稳定**由构造保证**，
> 不是靠测试。
>
> **结果（原型，未改仓库）：**
> - 长度：8.46 → **6.63** 码位（**−21.6%**），FVS 2.25 → 0.65/组；前缀稳定意义下的下界约 6.42，差 3%；
>   理论最短（不要求前缀稳定）5.87。1563 组变短、424 组相同、3 组多 1 个码位（跨 MVS 的严格前缀稳定代价）。
> - 正确性：语料全部 shape 及其全部前缀、**所有长度 ≤3 的输入（567 万个，62816 个不同 shape）**、
>   4000 个高密度 MVS/nirugu/ZWJ 随机长词：往返失败 0、前缀嵌套违例 0、死路 0。
>   现算法在长度 ≤3 的集合里有 2 个可达 shape 编码不了（报错），另外 `[Nirugu J Mvs Aa]` 也编不了；新算法全部能编。
> - 速度：把判定编译成表后（抽象状态 6 个分量，6.5 万 shape 上 0 冲突），未优化的原型运行时
>   shape 0.62 µs + 编码 0.58 µs = **1.20 µs/词，比现在（2.79 µs）快 2.3 倍**；不再需要整形校验。
>   现在一半时间在 `shape` 本身，可另行优化。
> - 表：360 个（候选字母, 位置）里只有 36 个的判定依赖上下文，末字母表只有 14 组依赖上下文，
>   压缩后约几千格；观察到的抽象状态只有 331 个。
>
> **需要你决定：** 见第 9 节（是否采用、承诺机制、重复编码字母、运行时校验、生成器用 Rust 还是 Python）。
>
> **追加研究（第 10 节）：输出能否保证是正确拼写（ger 而非 her，sain 而非 sein）？** 仅凭字形不能：
> tere/dere、ol/ul、hota/huda 等同形词字形完全相同；语料 shape 里只有 10.5% 的无 FVS 写法唯一，规则只能
> 猜对约一半。正确拼写还与前缀稳定根本冲突（bal/belge）。要保证正确需要词表。
> **决定：不做 `standardize`（需要词库，不再轻量）；`normalize` 只做 key。**

## 1. Goals and definitions

`normalize` maps a word to one Unicode string per shape. It must be:

- **round-trip** — `shape(normalize(x)) == shape(x)`;
- **canonical** — a function of `shape(x)` only (so same shape ⟹ same output); idempotence follows;
- **prefix-stable** — for reachable shapes `s ⊑ t` (a prefix, as written-unit sequences):
  `letters(E(s))[..-1] ⊑ letters(E(t))`, where `letters` splits a string into letter+FVS chunks and
  structural characters (MVS, nirugu, ZWJ) are chunks of their own. Only the last chunk of `E(s)`
  may change when the word grows — exactly the boundary letter whose position flips `fina → medi`;
- **short** — few code points (every Mongolian letter, FVS and MVS is 3 bytes in UTF-8, so code
  points are proportional to bytes);
- **fast** — O(n) per word with a small constant, no search, ideally no re-shaping.

Priority when they conflict stays **round-trip > prefix-stable > short**.

## 2. The current algorithm and why it is long and slow

`mng-canonical/2` (see `docs/internals.md`, "Normalization strategy"):

1. per `(position, written unit)` one `(letter, FVS)` chosen offline by a *context-independence
   battery*: it must render the unit next to **every** probe neighbour, left and right;
2. partition each chain **single unit first**, multi-unit table entries only as a last resort;
3. encode chains right to left, re-shape every chain in full context, re-shape the result.

Measured on the corpus (3389 unique words, 1990 shape groups):

| | current |
|---|---|
| code points per shape group | **8.46** (2.25 of them FVS) |
| shortest spelling found in the corpus | 6.09 |
| shortest possible spelling (search, any letters) | 5.83 |
| `normalize` / `shape` per word | 2.7–2.9 µs / 0.58–0.62 µs |

Causes:

- **Left context is treated as unknown.** Encoding left to right, the letters before a slot are
  already decided. Only the letters *after* it are unknown. Requiring independence from the left
  pins forms that the known left context already determines (medial `I` after a consonant, final
  `A` after a non-bowed letter, …).
- **Single-unit-first partition.** An isolated `a` (`A+A`) becomes `a+FVS1 a+FVS2` — 4 code points
  for a 1-code-point word; `ng` (`A+G`) becomes `a+FVS1 g+FVS2`.
- **The last letter is pinned too**, although prefix-stability never constrains it.
- **Speed:** 1 shape of the input + 1 re-shape per chain + 1 final re-shape.

Examples (aliases; code points in parentheses):

| word | current (`mng-canonical/2`) | proposed |
|---|---|---|
| ᠠ `a` | `a fvs1 a fvs2` (4) | `a` (1) |
| ᠰᠠᠢᠨ `s a i n` | `s a i fvs3 i fvs3 a fvs2` (8) | `s a i fvs3 i a` (6) |
| ᠮᠣᠩᠭᠣᠯ `m o ng g o l` | `m o a g fvs2 n fvs1 n fvs1 o l` (11) | `m o ng n fvs1 n o l` (8) |
| ᠤᠯᠠᠭᠠᠨ `u l a g a n` | `a fvs1 o l a n fvs1 n fvs1 a a fvs2` (12) | `o l a n fvs1 n a a` (8) |
| ᠪᠠᠶᠠᠨ᠌ᠦ᠌ᠨᠳᠦᠷ `b a y a n fvs2 ue fvs2 n d ue r` | `b a y fvs3 a a a o i fvs3 a t fvs2 o r` (15) | `b a y a a a o i fvs3 a t o r` (13) |
| ᠲᠠᠯ᠎ᠠ᠎ᠶᠢᠨ `t a l mvs a mvs y i n` | `t a l mvs a mvs i fvs1 i fvs3 a fvs2` (12) | `t a l mvs a mvs j i a` (9) |
| ᠲᠩᠷᠢ `t ng r i` | `t a g fvs2 r i` (6) | `t ng r i` (4) |
| ᠮᠣᠷᠢᠨ `m o r i n` | `m o r i fvs3 a fvs2` (7) | `m o r i a` (5) |

Like today's output, the proposed one encodes glyph shape, not phonetic spelling (`s a i … a`, not
`s a i n`).

## 3. Theory

### 3.1 Prefix-stability is online encoding with one pending letter

Call the letters of `E(s)` other than the last one *committed*. Prefix-stability says the committed
letters of `s` are a prefix of `E(t)` for every extension `t`. Equivalently, an encoder that reads
the shape left to right, **appends** committed letters and never revises them, and keeps at most
one pending letter (emitted only when the word ends) is prefix-stable *by construction* — whatever
choices it makes. Conversely, every prefix-stable encoder behaves this way on reachable prefixes.

So the design question is only: *which* letter to commit, *when*, such that the round trip holds
for every continuation.

### 3.2 Right-robustness is all a committed letter needs

When a letter is committed, everything to its left is known exactly (it is committed text). What
is unknown is the continuation. A committed letter `L` is **right-robust** if, for every
continuation the encoder itself may later produce, `L` still renders its own written units and every
earlier committed letter still renders its own. The final letter needs no robustness: it is checked
in its actual, complete context.

The shaping rules that look right from a letter are few and short-range: `n/t/d` onset/devsger and
`t` devsger (next letter), `sh` dotless (next letter and its position), `g/h` harmony (next adjacent
vowel), `d` marked (next vowel *and whether it is final* — two letters), `chachlag_onset` and `g`
dotless across an MVS (MVS, the next letter, whether it is isolated — three tokens). Two reach
further and get special treatment: the particle dictionary (whole MVS segment, ≤ 8 letters —
checked by completing every particle key that is still possible) and the masculine-marker walk of
`g/h` layer (b) (unbounded — bare `g/h` right after `i` is never committed). One more interaction
runs the other way: III.4 resolves (freezes) the previous vowel before III.5 can give it
`post_bowed`; that is caught because every probe re-checks all committed letters.

### 3.3 Promises

The encoder chooses the next letter too, so a committed letter may constrain it. Five classes cover
every rule above: *vowel*, *vowel but not `ee`* (`t + ee` is `t`'s devsger), *masculine vowel*,
*feminine or neutral vowel*, *consonant or `ee`*. Committing bare `n` for `N` with the promise
"next letter is a vowel" saves the FVS of `n+FVS1`; the next letter is then chosen from that class
only. A promise is made only when the next unit has a letter of that class at both medial and final
position.

### 3.4 Lazy commitment

A pending tail stays pending while it can still be one final letter; only when a new unit makes that
impossible does the encoder commit. This keeps multi-unit letters available (`a` = `A+A`, `o` =
`A+O`, `oe` = `A+O+I`, `ng` = `A+G`, `hh` = `A+Hr`, `i` after a vowel = `I+I`, …). Prefixes that are
not shapes of anything (e.g. a tail ending in a medial-only unit such as `Y`) impose no constraint;
the encoder simply keeps waiting.

### 3.5 How close to optimal

| encoder | code points / shape group |
|---|---|
| current `mng-canonical/2` | 8.46 |
| proposed, single-unit commits, no promises | 7.21 |
| proposed, single-unit commits + promises | 6.88 |
| proposed, lazy multi-unit commits, no promises | 6.91 |
| **proposed (lazy + promises)** | **6.63** |
| lower bound for *any* prefix-stable encoder (see below) | ≈ 6.42 |
| shortest possible, not prefix-stable (no duplicate letters / any letters) | 5.87 / 5.83 |

The lower bound is the cheapest round-tripping encoding of each shape whose every reachable prefix
is "a letter-prefix of it + one final letter"; it ignores robustness against the prefix's *other*
extensions, so no prefix-stable encoder can beat it (140 of the 1990 searches hit their budget and
count at the proposed value, so the true bound is a little lower). Prefix-stability itself therefore
costs about 10% over the unconstrained optimum, and the proposed encoder is within ~3% of the bound.
Example of the inherent cost: `[S A I I]` is a shape, and its first `I` must be committed as a
single medial `I` after a vowel, which only `i+FVS3` renders — so `sain` cannot be `s a i n`.

## 4. The algorithm (normative sketch)

Input: a shape (written units with structural tokens `Mvs`/`Nirugu`/`Zwj`), processed left to right.

**Candidates.** Every `(letter, FVS)` together with the written units it renders at a position:
FVS-named variants (cost 2, render their variant in every context), bare letters (cost 1, render
their default or a condition variant), and "unnamed FVS" forms (cost 2: an FVS that names no
variant still blocks the FVS-sensitive rules, so the letter keeps its default — final `j+FVS2` = `J`
is immune to `chachlag_onset`). Duplicate-encoding renderings (`Dd`, medial `H`/`Hx`, initial `Cr`)
are excluded (option, §9). Each `(position, units)` has its options sorted by
`(cost, preference, FVS)`.

**State.** Committed text, the pending units, the promise on the next letter.

**On a letter unit.** Append it to the pending tail. If the tail has a final letter (cheapest option
at `fina`/`isol` that renders it after the committed text), keep waiting. Otherwise *plan*: choose
spans from the front of the tail (longest first) and for each the cheapest right-robust option
(trying promises for bare letters) such that the rest of the tail again has a final letter; take
the cheapest total. If no plan exists, keep waiting (the prefix is not a shape).

**On a structural token.** Commit the whole tail (the last letter at `fina`/`isol`, or `medi`/`init`
before a joiner) against continuations that start with that token; then append the token.

**At the end.** Emit the final letter of the tail; if there is none, the shape is uncovered —
`NormalizationFallback` (strict) / input unchanged (lenient), exactly as today.

**Right-robustness battery** (what "every continuation" means, offline): committed text + candidate
+ `X1 X2 X3` where `X1` ranges over every option (any letter, any FVS) at `medi`/`fina` whose first
unit is the next pending unit and which satisfies the promise (or `X1` is the structural token),
`X2 ∈ {end, MVS, nirugu, ZWJ, every bare letter, a few FVS forms}`, `X3 ∈ {end, l, a, MVS, nirugu}`,
plus every still-possible particle completion; the candidate must render its own units at its
position and every committed letter its own. Bare `g/h` right after `i` is never committed.

**Preference among equal cost.** Code point order in the prototype (`a e i o u oe ue ee n …`); it
does not affect length. One visible quirk: `ger` comes out `h e r` because bare `h` and `g` both
render `G` before `e`; a preference list (e.g. `g` before `h`, the letter whose default glyph is the
unit first) would fix readability without changing any length.

## 5. Results

Prototype: `scratchpad/lab` (see Appendix), using the real crate as the shaping oracle.

**Length** — see §3.5. Per shape group: mean saving 1.83 code points, median 2, max 9; 1563 groups
shorter, 424 equal, 3 longer by one code point — all three across an MVS (`[Nirugu J Mvs]`,
`[Nirugu Nirugu J Mvs]`, a post-MVS `y`), where strict prefix-stability over structural tokens pins a
letter the current encoder leaves bare. (The current encoder cannot encode the extension
`[Nirugu J Mvs Aa]` at all.)

**Correctness** — prefix-stability holds by construction; the checks below cover the round trip,
the nesting of committed parts, and totality (no dead ends):

| test set | shapes | round-trip failures | nesting violations | unencodable |
|---|---|---|---|---|
| corpus shape groups, and every prefix of each | 1990 (12 350 prefixes) | 0 | 0 | 0 |
| corpus prefix pairs (incl. MVS/nirugu/ZWJ shapes) | 3859 pairs | — | 0 | — |
| every input of ≤ 3 symbols (35 letters × {bare, FVS1–4}, MVS, nirugu, ZWJ): 5 671 614 inputs | 62 816 | 0 | 0 | 0 |
| random 4–12-symbol words, 20% structural tokens | 4000 | 0 | 0 | 0 |

For comparison the current encoder fails (strict error) on 2 of the 62 816 short shapes
(`A+W+Mvs+A`, `A+W+Mvs+Aa`, from 45 inputs) and on `[Nirugu J Mvs Aa]`.

**Speed** — a table-driven prototype runtime (next section) reproduces the oracle encoder byte for
byte on the corpus: `shape` 0.62 µs + encode 0.58 µs = **1.20 µs per word vs 2.79 µs today (2.3×)**,
single-threaded, release build. The encoder is an unoptimized prototype (hash maps with 64-bit keys,
a recursive planner); `shape` is now half of the cost and is the next target (its tokens allocate,
its tables are hash maps).

## 6. Compiling it into tables

The oracle is too slow for runtime (≈ 20 ms per word). Its two predicates — "is this candidate
right-robust here" and "which final letter here" — depend on the committed text only through a small
**abstract state**:

- the previous letter: `(letter, FVS, position, written units)`;
- the kind of the previous token (letter / MVS / nirugu / ZWJ / none) and whether an MVS separates
  the previous letter;
- consonant-cluster status (an initial consonant followed by ≥ 1 medial consonant, for
  `III.2a` cluster `marked`);
- the forward masculine marker (nearest vowel back is a masculine one in initial/medial position);
- the particle-segment prefix (a node of the trie of particle keys, or dead once it cannot match or
  carries an FVS);

plus the promise. With the predicates memoized by this state, 64 805 shapes (corpus + all ≤ 3-symbol
inputs) produced **zero conflicts** (no state key ever needed two different answers), and the
table-driven runtime matched the oracle exactly. Only **331** distinct states occurred.

Sizes: of 360 `(candidate, position)` pairs only **36** have a state-dependent robustness answer
(bare `n t d i h g sh y a oe ue o u e` in a few positions, and `oe/ue+FVS4`); the other 324 are
constant. Each of the 36 depends on at most three state components plus the next unit and the
promise (≈ 4.3k cells before bit-packing the next-unit dimension). Final letters: 14 state-dependent
`(position, units)` groups (≈ 400 cells); everything else is a static `(position, units) → letter`
table like today's. Runtime structures: the candidate list, per-`(position, units)` option lists,
the 36 + 14 exception tables, a 200-node particle trie, and a hand-written state tracker.

Complete tables are generated by a breadth-first search over abstract states from the start state
(committing every candidate that can be committed), evaluating the predicates with `shape_detailed`
as the oracle and the battery of §4. Next-letter probes can be grouped by what the rules see
(alias class, FVS present) to keep this to minutes. The generator writes `MNG.normalize.json`
(schema `mongol-normalize-table/2`), `gen_rust_tables.py` compiles it, `--check` keeps it fresh —
the existing pipeline. Because the probing needs ~10⁸–10⁹ `shape_detailed` calls, the generator
should be Rust (a `publish = false` workspace tool), not Python (§9).

## 7. Guarantees and how they are established

- **Canonical, idempotent** — by construction (the input is the shape).
- **Prefix-stable** — by construction (append-only commits, one pending letter). Tested anyway,
  over all shapes including structural tokens.
- **Round-trip** — each committed letter is right-robust against every continuation the encoder can
  produce and never disturbs an earlier one; the final letter is checked in full context. The
  soundness argument rests on the reach of the rules (§3.2) and on the abstract state capturing
  every left dependency; both are validated by the conflict check and the exhaustive tests. The
  runtime can additionally re-shape its output (`debug_assert!`, or always — §9).
- **Totality** — no dead end in any test; if one ever occurs the contract is today's
  (`NormalizationFallback` / lenient echo).

## 8. What changes

- `canonical_version` → `mng-canonical/3`; every stored key changes (beta; documented).
- API unchanged. `normalize_written_units` / the positioned API keep their re-shape check, because
  their input can be an unreachable sequence.
- Regenerated: `MNG.normalize.json` (new schema), `src/generated/mng_normalize.rs`,
  `tests/golden/mng-canonical-v1.jsonl` (→ `-v3`); tests that hard-code canonical strings
  (`canonical_sain`, the README examples) change.
- `docs/internals.md` "Normalization strategy" and `docs/data-format.md` "Normalize table" are
  rewritten; ports in other languages need the state tracker and the small planner in addition to
  the tables.
- Prefix-stability test widened from pure-letter chains to all shapes, plus an exhaustive
  short-shape nesting test.
- Optional, independent: an incremental API (`push(unit) → committed text`) falls out of the design
  (IME / streaming search), and `shape` itself can be made faster.

## 9. Decisions needed

**Resolved 2026-09-23:** all six as recommended below (adopt as `mng-canonical/3`; promises on;
duplicate-encoding letters off; no re-shape check in `normalize`, `debug_assert!` only; a Rust
table generator; a readability tie-break list). The key keeps strict prefix-stability.

1. **Adopt the online encoder as `mng-canonical/3`?** Recommended.
2. **Promises** (next-letter class constraints): on — 4% shorter, about 15 of 9679 corpus decisions
   produce unusual spellings (`t n` for `T A`, `y+FVS1` for a medial `I`). Off — simpler tables.
   Recommended: on.
3. **Duplicate-encoding letters** (`h` for `A A`, `d` for `O A`, `g` for `N N`): off — 0.6% longer,
   far more natural (`y a b o a a t a a a o` instead of `y a b d n t a a a o`). Recommended: off.
4. **Runtime re-shape check in `normalize`:** off (fastest; correctness from the generator's
   exhaustive search and the tests; `debug_assert!` in debug builds) or on (+0.6 µs/word, fails
   closed). Recommended: off, keep it in `normalize_written_units`.
5. **Table generator:** Rust workspace tool (recommended; compute-heavy) or extend
   `gen_normalize_table.py` (keeps the "generators are Python" convention, much slower).
6. **Tie-break preference** for equal-cost letters: code point order, or a readability list
   (`g` before `h`, default-glyph letter first). Recommended: the readability list.

## 10. Follow-up: can the output be the *correct* spelling? (research, 2026-09-23)

Question from review: can `normalize` guarantee the spelling a user expects — `ger`, not `her`;
`sain`, not `sein` and not `s a i+FVS3 i a`?

**Not from the glyphs.** The script maps different correct words to the same glyphs, so any
function of the shape is wrong for one of them. Verified with the shaper — each pair renders
identically:

| pair | shape |
|---|---|
| ᠲᠡᠷᠡ `tere` "that" / ᠳᠡᠷᠡ `dere` "pillow" | `T A R A` |
| ᠣᠯ `ol` "find" / ᠤᠯ `ul` "sole" | `A O L` |
| ᠬᠣᠲᠠ `hota` "city" / ᠬᠤᠳᠠ `huda` "in-law" | `H O D A` |
| ᠣᠷᠣᠨ `oron` / ᠤᠷᠤᠨ `urun` | `A O R O A` |
| ᠭᠡᠷ `ger` / ᠬᠡᠷ `her` | `G A R` |
| ᠰᠠᠢᠨ `sain` / ᠰᠡᠢᠨ `sein` | `S A I I A` |

Corpus measurements (FVS-free spellings enumerated by search; the FVS-free corpus inputs — mostly
GB/T test vectors of real words — used as references):

- Of 1989 shapes, only **209 (10.5%)** have exactly one FVS-free spelling; 394 have none (need an
  FVS); 485 have 17 or more. `S A I I A` has six: `s a i n`, `s e i n`, `s a i a`, `s a i e`,
  `s e i a`, `s e i e`.
- Filtering by vowel harmony (no masculine `a o u` with feminine `e oe ue ee`; `i` neutral) and
  syllable structure leaves a unique candidate in 95 of 884 reference shapes. Rules plus default
  preferences (masculine, `o` before `u`, `g` before `h`, `t` before `d`) pick a reference spelling
  in **about 50%** of shapes.
- What stays ambiguous is mostly lexical — `g/h` (243 shapes), `o/u` (179), `d/t` (129), `oe/ue`
  (121), `s/sh` before `i` (65), `i/y` (60) — plus `a/e` (156) where the word has no harmony clue
  (initial vowel, first-syllable `oe/ue`, a velar form). No rule decides these; a lexicon does.

**Correct spelling contradicts prefix-stability.** ᠪᠠᠯ `bal` "honey" shapes to `B A L`, ᠪᠡᠯᠭᠡ
`belge` "sign" to `B A L G Aa`. A prefix-stable encoder must write belge's first vowel like bal's,
`b a l g e`, which is misspelled; spelling both correctly breaks prefix-stability. The correct
letter depends on glyphs that come later (harmony, the word's identity), prefix-stability forbids
looking at them. Among the corpus reference spellings, 38 of 985 prefix pairs are inconsistent in
this way (`tabu`/`tebüne`, `bui`/`börhüg`, `tede`/`tatag`, …).

**The FVS in `s a i+FVS3 i a` is a different matter.** It is forced by strict prefix-stability over
*every* shape, including `S A I I`, which no FVS-free spelling produces. Requiring stability only
between prefixes that can be typed without FVS would let the key drop most FVS, but it would still
not be the correct spelling (belge stays `b a l g e`).

**What can be guaranteed, and how.**

- A lexicon (correct spellings, with frequencies) makes the output correct for every word it
  contains. Homographs (`tere`/`dere`) still need a choice: the most frequent reading — or, for
  display, the input itself when it is already a lexicon word, so a correctly typed `dere` is never
  "corrected" to `tere`. Words outside the lexicon can only be best effort (rules, or a letter
  n-gram model trained on the lexicon).
- So there should be two outputs:
  - `normalize` — the **key** (canonical, prefix-stable, short, fast; §§3–7). For search, dedup,
    indexing. Not for display.
  - `standardize` (new) — the **display spelling**: keep the input when it is a standard spelling;
    otherwise look the key up in the lexicon and take the most frequent spelling; otherwise fall
    back to rules. Not prefix-stable. The lexicon is indexed by the key, so the two fit together.
- The crate can ship the mechanism (a `Lexicon` built from a word list, `standardize(text,
  &lexicon)`) and keep the data outside, or bundle a small list of common words.

**Decision (2026-09-23): no `standardize`.** It needs a lexicon, and a lexicon would make the
crate heavy. `normalize` stays a key; guaranteed-correct spelling is out of scope. Still open:
whether the key should be relaxed to "prefix-stable for FVS-free prefixes" to read better.

## 11. Implementation (2026-09-23)

Implemented as designed (runtime `src/encoder.rs` + `src/normalize.rs`, generator
`examples/gen_normalize_table/`, schema `mongol-normalize-table/2`), with these refinements:

- **Preference** (§4, §9.6): cost, then `n` first for a final `A` right after a vowel (the word's
  last letter and the last letter before a structural token), then — after a feminine vowel —
  `e oe ue` before `a o u`, then code point order with `g` before `h`, then the FVS. `sain` is
  therefore `s a i+FVS3 i n`, not the prototype's `… i a`; `ger` is `g e r`.
- **Harmony is a runtime preference, not a table key**: finals are validity masks over a group's
  options and the runtime picks the first valid one in its order; this kept the tables small.
- **Tables**: 406 candidates (100 context-dependent), 2 705 reachable contexts, 996 robustness rows
  and 893 final rows after default + exception compression; 95 KB of JSON; ~12 s to generate.
- **Results** match §5: 6.62 code points per shape group (0.64 FVS), 0.92–1.00 µs per word including
  `shape` (0.58 µs) vs 2.72–2.91 µs, and 0 failures on every input of ≤ 3 symbols and 20 000 random
  words. Details and the reviewed behaviour changes: the plan's execution notes.

## Appendix: prototype

Session scratchpad (temporary; move it into the repository before it is needed for implementation)
`/private/tmp/claude-501/-Volumes-SN570-Users-satsrag-orca-mongol-norm/32cf3db5-0cfc-4628-8a79-9985f347b702/scratchpad/lab/`
— Rust, depends on the crate by path; `lab/src/online.rs` is the oracle encoder (the reference for
this design), `lab/src/lean.rs` the table-driven runtime prototype:

- `optimal` — A* shortest encoding per shape group (unconstrained optimum);
- `online_eval [promise] [nodup] [single]` — the oracle online encoder on the corpus: lengths,
  round trip, prefix pairs, every prefix;
- `pslb` — prefix-stable lower bound;
- `exhaust <max_len> <fuzz>` — all inputs up to `max_len` symbols plus random words;
- `compiled_eval <extra>` — memoized abstract-state predicates (conflict check), the table-driven
  runtime and the timing;
- `cur_total`, `cur_pairs`, `cur_prefix` — the same checks against the current encoder;
- `natural`, `standardize`, `ref_prefix`, `homographs` — §10: FVS-free spellings per shape, the
  rule-based pick, prefix conflicts between reference spellings, glyph-identical word pairs.
