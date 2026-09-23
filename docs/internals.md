# How mongol-norm works · 实现原理

[English](#english) | [中文](#中文)

---

<a id="english"></a>
## English

Background for contributors and porters. Users only need the [README](../README.md).

> **Beta.** `shape` is considered stable. The `normalize` output (the `mng-canonical/3` policy
> described below) encodes by glyph shape, not the standard phonetic (nominal-character) spelling —
> e.g. ᠮᠣᠩᠭᠣᠯ (`MA+O+ANG+GA+O+LA`) → ᠮᠣᠩᠨ᠋ᠨᠣᠯ (`MA+O+ANG+NA+FVS1+NA+O+LA`) — and may change in a later release.

The normalizer implements a **lightweight Mongolian shaping engine** — equivalent to what HarfBuzz
does with a font file, but using only the rule data from
[UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) and the
[mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) project. No font files needed.

### Shaping pipeline (UTN #57 v4 Mongolian-specific phase)

1. **Chachlag** — Suffix forms for A/E after MVS (Mongolian Vowel Separator)
2. **Syllabic** — Consonant/vowel context: onset, devsger, marked, masculine/feminine harmony, dotless
3. **Particle** — MVS particle dictionary lookup for specific suffix words
4. **Devsger** — I after a vowel (vowel_devsger) gets double-tooth form: `I → I+I`
5. **Post-bowed** — Vowel forms change after bowed consonants (G, B, K, P, F)

### Duplicate encodings

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

### Normalization strategy

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

### Data and fixtures

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

---

<a id="中文"></a>
## 中文

面向贡献者和移植者的背景说明。普通用户只需要看 [README](../README.md#中文)。

> **Beta 版。** `shape` 视为稳定。`normalize` 的输出（下文描述的 `mng-canonical/3` 策略）是按字形编码的，
> 不是按读音书写的标准名义字符序列——例如 ᠮᠣᠩᠭᠣᠯ（`MA+O+ANG+GA+O+LA`）→ ᠮᠣᠩᠨ᠋ᠨᠣᠯ（`MA+O+ANG+NA+FVS1+NA+O+LA`）——
> 后续版本可能修改。

规范化器实现了一个**轻量级的蒙古文整形引擎**：相当于 HarfBuzz 配合字体文件所做的事，但只使用
[UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) 和
[mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) 项目中的规则数据，不需要字体文件。

### 整形流程（UTN #57 v4 蒙古文专用阶段）

1. **Chachlag** —— MVS（蒙古文元音分隔符）之后 A/E 的后缀形式
2. **Syllabic（音节）** —— 辅音/元音上下文：onset、devsger、marked、阳性/阴性元音和谐、dotless
3. **Particle（粒子）** —— 对特定后缀词查 MVS 粒子词典
4. **Devsger** —— 元音之后的 I（vowel_devsger）取双齿形：`I → I+I`
5. **Post-bowed** —— 圆头辅音（G、B、K、P、F）之后的元音变形

### 重复编码

`shape` 承诺是*可见*词形的指纹——"同 shape ⟹ 同 `normalize`"是这个 crate 存在的全部理由。有九个书写单元
破坏了这个承诺：它们渲染出的墨迹和另一串单元完全相同，于是同一个词的两种拼写得到了不同的 shape。

其中五个通过把单个单元**展开**成一对来统一：

| 重复单元 | 公开 shape | 见证对 |
|---|---|---|
| `Dd:medi` | `O:medi A:medi` | ᠣᠳᠪᠣ / ᠠ᠋ᠣᠣᠠᠪᠣ᠋ |
| `Dd:fina` | `O:medi A:fina` | ᠠᠷᠠᠳ / ᠠᠷᠠᠤᠠ |
| `H:medi`  | `A:medi A:medi` | ᠪᠠᠭᠰᠢ / ᠪᠠᠠᠠᠰᠢ |
| `Hx:medi` | `N:medi N:medi` | ᠠᠷᠭᠠᠯ / ᠠ᠋ᠠᠷᠨ᠋ᠨ᠋ᠠᠯ |
| `Cr:init` | `O:init O:medi` | ᡂ᠊ / ᠤ᠋ᠤ᠊ |

另外四种形式通过**收缩**统一，只在验证过的位置和上下文里选择更短的规范形式。`Aa:fina` 紧跟在圆头书写单位
之后时带一个齿，其他情况下没有齿；反复插入/删除 `A` 并不能保持墨迹不变：

| 单元对 | 公开 shape | 见证对 |
|---|---|---|
| 占满整条链的 `A Aa` | `A:isol`  | ᠡ / ᠡᠠ᠋ |
| `圆头书写单位 A:medi Aa:fina` | `圆头书写单位 Aa:fina` | ᠪᠠ / ᠪᠠᠠ᠋ |
| 结束一条链的 `O Aa` | `B2:fina` | ᠊ᠪ᠋ / ᠊ᠤᠠ᠋ |
| 结束一条链的 `I Aa` | `G:fina`  | ᠊ᠭ / ᠊ᠢᠠ᠋ |

每条规则都带位置，而且是*链*内的位置——链是结构单元之间的那些字母；nirugu 或 ZWJ 邻居会像在渲染中一样
给链补位。所以 `B2` 和 `G` 的见证对前面都写了一个 nirugu：正是它让这个单元成为词尾形。不在验证过的单元对
之内的形式保持不变——词首和词尾的 `H`/`Hx` 墨迹不同，单独一个 `Cr` 的链是 `Cr:isol`（而不是验证过的
`Cr:init`），单独一个 `A` 的链本来就是规范形式。

**术语。** UTN #57 使用**圆头书写单位**（bowed written units）这一术语，对应的查找称为 **Post-bowed**；
见 [UTN #57 第 4 版](https://www.unicode.org/notes/tn57/utn57-mong-4.pdf)。

**上下文。** 回鹘式完整的圆头书写单位集合是 **B、P、F、G、Gx、K、K2**，并非所有辅音。来源：
[回鹘式书写单元连写变体](https://mongfontbuilder.pages.dev/hudum/)、
[必需连字](https://github.com/Kushim-Jiang/mongfontbuilder/blob/7d5fc1cdaf8210f675c16699a8eaeb71aa1e80ca/data/ligatures.ts)
（`BAa`、`PAa`、`FAa`、`GAa`、`GxAa`、`KAa`、`K2Aa`），以及 `src/rules.rs` 中的 post-bowed 类。
圆头书写单位必须在同一条链里紧挨在词中 `A` 之前：`B A Aa → B Aa`，而 `N A Aa`、`A A Aa`、`B A A Aa`
保持不变。连接符的补位不算圆头书写单位，也不能让这条规则跨越结构符。另一条独立的整链规则
`A:init Aa:fina → A:isol` 不变。

**终止性与幂等性。** 先展开一次，再只检查一次最后一对。展开不会产生新的展开目标，收缩也不会让词首/词尾的
`H`/`Hx` 变成词中。每次收缩都在链尾结束：`A`、`B2`、`G` 都不以 `Aa` 结尾；收缩得到的 `Aa` 前面是圆头
书写单位而不是 `A`/`O`/`I`，所以不会再次收缩。幂等性来自这些限制条件，而不是把文本强行迭代到不动点。
20 个单元的字母表（包括全部圆头书写单位、展开目标和结构符）在长度 4 以内做了穷举：**168 421 个输入**。
语料的书写单元重新编码后也能整形回自身。

展开不会带来非圆头书写单位的收缩：ᠲᠡᠳ᠌ᠡ᠋（`T A Dd Aa`）变成 `T A O A Aa`，**而不是** `T A B2`，因为 `O`
不是圆头书写单位。同样，`B H Aa` 展开后是 `B A A Aa`。这两种相互作用都有回归测试。

所有面向用户的接口看到的都是统一后的序列：`same_shape`、`normalize` 和书写单元编码器。
`normalize_written_units` 仍然*接受*重复单元作为输入，并先统一它们，所以调用方已有的数据照常可用。
ᠠᠷᠠᠳ 和 ᠠᠷᠠᠤᠠ 是同一个可见的词，现在也是同一个 shape、同一个规范文本。

UTN #57 和 GB/T 25914-2023 把这九个都当作不同的书写单元——它们的 EAC 测试向量把 ᠠᠷᠭᠠᠯ 写作
`A A R Hx A L`——引擎内部也仍然会产生它们。标准自己的序列可以通过 `Shaper::shape_raw`（Python：
`MongolianShaper._shape_raw`）得到，一致性测试比较的就是它。它**不属于公开契约**：它是 `#[doc(hidden)]`，
将来为了统一更多重复编码可能会改变，且不算大版本变更。

`shape_detailed` 和 `trace` 的 `written_by_token` 报告的是每个 token 自己的单元，所以它们也是未统一的原始
序列——统一是整词改写，单个 token 无法承载。`trace` 的 `shape` 字段是公开的、统一后的序列。

### 规范化策略

在规范化表覆盖的范围内，`normalize` 是 **shape 的纯函数**：任意两种整形结果相同的编码都得到同一个
Unicode 输出，而且输出能往返——`shape(normalize(x)) == shape(x)`。它还是**前缀稳定**的：一个词的
shape 前缀的编码，除最后一个字母外，是整个词的编码的前缀。这些目标冲突时的优先级是
**往返 > 前缀稳定 > 最短**。

对每个词，`normalize` 先对输入整形（重复编码已统一，见上文），再用**在线编码器**（`src/encoder.rs`）
从左到右编码书写单元。结构字符——MVS、nirugu、ZWJ——在 shape 中原样作为 PascalCase 的
`Mvs` / `Nirugu` / `Zwj` 出现，并原样复制到输出；它们也是判断相邻字母取词首/词中/词尾形式的依据。

**前缀稳定就是在线编码。** 除最后一个字母外，每个字母都会被*提交*：追加到输出，之后不再修改。最后一个
字母保持*待定*，直到词结束。这样工作的编码器无论选哪些字母，按构造就是前缀稳定的；所以设计问题只剩下：
提交哪个字母，什么时候提交。

**提交的字母必须右稳健。** 提交一个字母时，它左边的一切都已确定——那是已提交的文本。未知的是词的其余部分。
只有当一个字母对编码器之后可能写出的*每一种*后续，都仍渲染出它自己的书写单元、并且不改变任何已提交字母的
书写单元时，才能提交它。待定的最后一个字母不需要稳健：它是在实际的、完整的上下文里选出来的。

**承诺。** 下一个字母也由编码器来写，所以提交一个裸字母时可以同时约束下一个字母。词中的裸 `n` 只有在元音前
才渲染成 `N`（其他情况下是一个普通的齿，即 `A`），所以可以带着"下一个字母是元音"的承诺提交它，而不必写成
`n+FVS1`；之后下一个字母就只从这一类里选。五个类别覆盖了所有向右看的规则：*元音*、*除 `ee` 以外的元音*
（`t` 在 `ee` 前取 devsger 形）、*阳性元音*、*阴性或中性元音*、*辅音或 `ee`*。

**懒提交。** 只要还有一个词尾字母能用待定的尾部结束这个词，尾部就继续等待，这样多单元字母仍然可用：
`a` = `A A`、`o` = `A O`、`oe` = `A O I`、`ng` = `A G`、元音后的 `i` = `I I`，…… 当新来的单元让这变得
不可能时，就对尾部做一次小搜索——通常一两个单元，每个字母最多三个——提交最便宜的右稳健字母，使剩下的尾部
能由一个词尾字母结束。遇到结构符时提交整个尾部（其最后一个字母在 MVS 前取 `fina`/`isol`，在连接符前取
`medi`/`init`）。词结束时，尾部成为词尾字母。如果没有字母能结束它——即超出表覆盖范围的 shape，目前没有
已知的例子——严格模式下规范化报错，并给出输入和它的书写单元；宽松模式原样返回输入。它绝不会编错。

**偏好。** 在代价相同的字母之间（裸字母 1 个码位，带 FVS 的字母 2 个），编码器按以下顺序选择：

- 紧跟在元音后的词尾 `A` 用 `n`——元音后接元音在这门语言里很少见，词尾 `n` 很常见；
- 在阴性元音之后（指最近写下的阳性或阴性元音；MVS 不会重置它，所以后缀跟随词干），`e oe ue` 优先于
  `a o u`；
- 按码位顺序，但 `g` 排在 `h` 之前（ᠭᠡᠷ，而不是 ᠬᠡᠷ）；
- 最后 FVS 编号小的优先。

独立形的 `I` 写作 `i+FVS1`，从不写作 `j`。shape 会统一成单元对的那些渲染（`Dd`、词中 `H`/`Hx`、词首 `Cr`）
从不输出。

输出编码的是字形，不是拼写。ᠰᠠᠢᠨ 编成 `s a i+FVS3 i n`：`S A I I` 本身就是一个 shape，它的第一个 `I`
必须作为元音后的单个词中 `I` 提交，而只有 `i+FVS3` 能渲染出这个形式。一般来说，任何基于字形的函数都无法
还原本来的拼写——ᠲᠡᠷᠡ *tere* 和 ᠳᠡᠷᠡ *dere* 渲染得完全一样——而且正确拼写和前缀稳定互相矛盾：ᠪᠠᠯ *bal*
是 ᠪᠡᠯᠭᠡ *belge* 的前缀 shape，所以 belge 的 key 开头和 bal 一样。

**表是怎么生成的。** 一个字母是否右稳健，只通过一个小的*上下文*依赖已提交的文本：前一个字母（字母、FVS、
位置、书写单元）；前一个 token（无、字母、MVS、nirugu、ZWJ），以及前一个字母之后是否出现过 MVS；是否有
一个词首辅音后接了词中辅音（III.2a cluster `marked`）；向前最近的元音是否是位于词首或词中的阳性元音
（III.2f `g`/`h`）；当前 MVS 段的粒子 key 前缀（III.3）；以及当前生效的承诺。
`examples/gen_normalize_table` 从词首开始，广度优先地探索编码器能到达的每一个上下文（约 2 700 个），并通过
对探测文本整形来询问整形引擎——探测文本由已提交的文本、候选字母，以及编码器在接下来两三个字母里可能写出的
每一种后续组成，外加该段仍可能补全的每一个粒子 key——哪些字母可以在哪个下一单元之前、带哪种承诺提交，
哪些字母可以结束这个词。上下文相同的状态必须得到相同的答案，否则生成器报错（说明上下文漏掉了规则能看到的
东西）。每个答案都按能决定它的最少上下文分量制成表，形式是一个默认值加若干例外行。整个运行约 4 900 万次
探测，8 核约 15 秒。它写出 `python/mongol_norm/data/MNG.normalize.json`（格式见
[`docs/data-format.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/data-format.md#中文)），再由
`gen_rust_tables.py` 编译成 `src/generated/mng_normalize.rs`。

**保证。** 规范且幂等：输入就是 shape。前缀稳定：由构造保证。往返：每个提交的字母对编码器可能写出的每种
后续都稳健，词尾字母在上下文中选出；测试在以下输入上检查它：所有不超过两个符号的输入（字母带或不带各个
FVS，以及 MVS、nirugu、ZWJ）、2 000 个结构字符密集的随机词、每个语料词及其每个前缀
（`tests/online_encoder.rs`），以及规范 golden 向量。debug 构建会对每个输出重新整形校验（`debug_assert!`），
release 构建不校验。`normalize_written_units` 和 positioned API 仍然重新整形校验，因为它们的输入可能
根本不是任何字形。

在 1 990 个语料 shape 组上，输出平均 **6.62 个码位**（其中 FVS 0.64 个；`mng-canonical/2`：8.46 个，
FVS 2.25 个；不要求前缀稳定时，最短编码平均 5.83 个）。`normalize` 每个词约 0.95 µs，其中 `shape` 占
0.58 µs（`mng-canonical/2`：2.8 µs）。

具体的规范选择策略固定为 **`mng-canonical/3`**。它可以通过 `Shaper::canonical_version`（Python：
`shaper.canonical_version`）取得，也写在 `MNG.normalize.json` 里。持久化规范化搜索/索引 key 的应用应当把
这个版本和 key 一起保存，并在以后的版本修改它时重建这些 key。

**`mng-canonical/3` 使 `mng-canonical/2` 下保存的 key 失效。** 在线编码器取代了 `/2` 的逐单元 FVS 钉死表：
1 990 个 golden 代表中有 1 656 个发生变化（1 562 个变短，91 个长度不变，3 个变长）。请重建所有已保存的
规范化 key。

**`mng-canonical/2`（0.2.0）使 `mng-canonical/1` 下保存的 key 失效。** 统一九个重复编码改变了规范文本：与
基线分支的 1993 个 `mng-canonical/1` golden 代表相比，有 **287** 个文本变化；三对经过验证的组合并后共
**1990** 组。

### 数据与测试数据

整形和规范化规则是扁平的、与编程语言无关的 JSON，位于 `python/mongol_norm/data/`（`MNG.json`、`TOD.json`、
`SIB.json`、`MCH.json` 和 `MNG.normalize.json`）。运行时不读取它们：`python/scripts/gen_rust_tables.py`
把它们编译成 `src/generated/` 中的静态 Rust 表，crate 和 wheel 带的都是这些表。wheel 仍然附带 JSON 供工具
使用。格式和使用它的算法写在
[`docs/data-format.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/data-format.md#中文) 里，所以
用其他语言移植只需要一个 JSON 解析器。

两套测试读取同一份测试数据（fixtures），只在 crate 的 `tests/` 下存放一次：

| 测试数据 | 内容 |
|---|---|
| `tests/data/core-hud.tsv` | 177 行——mongfontbuilder 精选的回归集（225 个用例） |
| `tests/data/eac-hud.tsv` | 3512 行——GB/T 25914-2023（3513 个用例，5 个 UTN-xfail） |
| `tests/golden/mng-canonical-v1.jsonl` | 1990 个规范向量 |
| `tests/golden/mng-phase-trace-v1.json` | 15 个阶段追踪向量 |

由于语料和 golden 测试读取这个目录，`cargo test` 需要完整的仓库检出——发布的 crate 不包含这些测试数据。
