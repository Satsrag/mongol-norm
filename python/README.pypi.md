# mongol-norm

[![PyPI](https://img.shields.io/pypi/v/mongol-norm.svg)](https://pypi.org/project/mongol-norm/)
[![Python versions](https://img.shields.io/pypi/pyversions/mongol-norm.svg)](https://pypi.org/project/mongol-norm/)
[![crates.io](https://img.shields.io/crates/v/mongol-norm.svg)](https://crates.io/crates/mongol-norm)
[![Test](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml/badge.svg)](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml)

Shape-aware normalizer for Traditional Mongolian (Hudum) script.
传统蒙古文（回鹘式）形态感知规范化器。

*English first, 中文见下半部分。*

---

## English

`mongol-norm` implements the full [UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html)
Mongolian shaping pipeline and an FVS-pinned canonical normalizer. The same visible Mongolian word
can be written as many different Unicode sequences, which breaks search, deduplication and
indexing. `shape()` renders a word into its written-unit sequence — what a font would draw,
computed without a font — and `normalize()` rewrites any encoding of a word into the one canonical
encoding for that shape.

This package is a thin [PyO3](https://pyo3.rs) binding over the Rust crate
[`mongol-norm`](https://crates.io/crates/mongol-norm), which is developed in the
[same repository](https://github.com/Satsrag/mongol-norm). There is one implementation: one set of
data tables, one corpus and golden fixture set, one version number — `pip install mongol-norm` and
the crate of the same version return byte-identical results.

### Install

```bash
pip install mongol-norm
```

The engine and its shaping/normalize tables are compiled into the `mongol_norm._native` extension,
so the package has **no runtime dependencies** and needs no Rust toolchain.

Prebuilt `cp39-abi3` wheels — one per platform, serving every CPython ≥ 3.9 — cover:

| Platform | Wheels |
|---|---|
| Linux x86_64 / aarch64 | glibc (manylinux2014) and musl (musllinux_1_2) |
| macOS | x86_64 and Apple silicon (arm64) |
| Windows | x64 |

On any other platform pip falls back to the source distribution, which compiles the extension
locally and needs a Rust toolchain ≥ 1.83 (pip fetches the `maturin` build backend by itself).

### Quick start

```python
from mongol_norm import MongolianShaper

shaper = MongolianShaper(locale="MNG")   # Hudum Traditional Mongolian

# Shape: the written-unit sequence a font would draw
shaper.shape("ᠰᠠᠢᠨ")
# → ['S', 'A', 'I', 'I', 'A']

# Compare: do two encodings render identically?
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")
# → True
shaper.same_shape("ᠰᠠᠢᠨ", "ᠨᠠᠢ᠍ᠮᠠ")
# → False

# Normalize: many encodings of one word → one canonical, FVS-pinned form
shaper.normalize("ᠰᠡᠢᠨ")     # → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'
shaper.normalize("ᠰᠠᠶ᠋ᠢᠨ")   # → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'
shaper.normalize("ᠰᠠᠶ᠋ᠶ᠋ᠨ")   # → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'
```

Deduplicating a list of encodings is then just a `set`:

```python
words = ["ᠰᠡᠢᠨ", "ᠰᠠᠢᠨ", "ᠰᠨ᠌ᠢᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ"]
unique = {shaper.normalize(w) for w in words}
print(f"{len(words)} inputs → {len(unique)} unique form(s): {unique}")
# 4 inputs → 1 unique form(s): {'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'}
```

Locales are `"MNG"` (Hudum), `"TOD"` (Todo), `"SIB"` (Sibe) and `"MCH"` (Manchu); an unknown locale
raises `ValueError`. Only `MNG` has a normalize table — the other three shape only.

### API

`MongolianShaper(locale="MNG")` is the whole public surface, alongside the
`NormalizationFallbackError` exception.

#### Shaping

```python
shaper.shape("ᠰᠠᠢᠨ")       # → ['S', 'A', 'I', 'I', 'A']
shaper.shape_str("ᠰᠠᠢᠨ")   # → 'S+A+I+I+A'   ("+".join(shape(text)))
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")   # → True
```

`shape()` and `normalize()` take a **single word** and raise `ValueError` on any character outside
the Mongolian word alphabet (letters, FVS, MVS, NNBSP, nirugu, ZWJ). Use `normalize_text()` for
mixed-script input.

**The public shape unifies nine duplicate encodings** — written units that render as exactly the
same ink as a sequence of other units. Five are unified by expanding the unit into the pair: `Dd`
(in both the positions it has), medial `H`, medial `Hx` and initial `Cr` come out as `O A`, `A A`,
`N N` and `O O`. The other four cannot be expanded — their expansion ends in `Aa`, which is itself
a duplicate, so it would never terminate — and are unified the other way, by contracting the pair
into the unit: a chain-final `A Aa` becomes `Aa` (or `A` when it is the whole chain), `O Aa`
becomes `B2`, and `I Aa` becomes `G`. That is what makes `shape()` a fingerprint of the *visible*
word:

```python
shaper.shape("ᠠᠷᠠᠳ")                  # → ['A', 'A', 'R', 'A', 'O', 'A']
shaper.same_shape("ᠠᠷᠠᠳ", "ᠠᠷᠠᠤᠠ")     # → True  (one word, two spellings)
shaper.shape("ᠪᠠᠠ᠋")                  # → ['B', 'Aa']
shaper.same_shape("ᠪᠠ", "ᠪᠠᠠ᠋")        # → True
```

The full rule table, the witness pairs and the termination argument are in the project README's
"Duplicate encodings" section.

UTN #57 and GB/T 25914-2023 keep all nine as distinct written units — their EAC vectors spell ᠠᠷᠭᠠᠯ
as `A A R Hx A L` — and the engine still produces them. The standard's own sequence is
`shaper._shape_raw(text)`, which exists for the conformance suites and is **not part of the public
contract** (hence the leading underscore); it may change to fold further duplicates without a major
bump. `shape_detailed()` and `trace()['written_by_token']` report each token's own units and are
therefore raw as well — unification is a whole-word rewrite no single token can carry;
`trace()['shape']` is the public, unified sequence.

`shape_detailed(text)` returns one dict per token — code point, locale alias, joining position, FVS
selector, the shaping condition that selected the variant, and the written units it renders to:

```python
shaper.shape_detailed("ᠪᠠ")
# → [{'cp': 'U+182A', 'alias': 'b', 'position': 'init', 'fvs': '',
#     'condition': '', 'written': ['B']},
#    {'cp': 'U+1820', 'alias': 'a', 'position': 'fina', 'fvs': '',
#     'condition': 'post_bowed', 'written': ['Aa']}]
```

`trace(text)` returns the rule-by-rule pipeline trace used by the project's golden fixtures —
`{"positions", "transitions", "final_conditions", "written_by_token", "shape"}`, where each
transition is `{"rule": name, "changes": [{"token", "before", "after"}, ...]}` and only rules that
changed at least one condition are listed:

```python
shaper.trace("ᠪᠠ")["transitions"]
# → [{'rule': 'III.5.post_bowed',
#     'changes': [{'token': 1, 'before': None, 'after': 'post_bowed'}]}]

shaper.rule_names()
# → ['III.1.chachlag', 'III.2a.o_u_oe_ue.marked', ..., 'III.5.post_bowed']
```

#### Normalization

```python
shaper.normalize("ᠰᠡᠢᠨ")                        # one word
shaper.normalize_text("Hello ᠰᠡᠢᠨ world")       # free-form text
# → 'Hello ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌ world'
```

`normalize_text()` normalizes each Mongolian word independently and preserves spaces, punctuation
and non-Mongolian text verbatim — use it for sentences, paragraphs and mixed-script input.

Both take `strict=True` by default: a written-unit shape the bundled normalize table cannot encode
raises `NormalizationFallbackError`, whose `.text` and `.written_units` attributes carry the input
and the uncovered shape. Pass `strict=False` explicitly only when returning the input unchanged is
acceptable — the round trip is preserved either way, and the API never mis-encodes.

```python
from mongol_norm import NormalizationFallbackError   # a ValueError subclass

try:
    canonical = shaper.normalize(word)
except NormalizationFallbackError as exc:
    print(exc.text, exc.written_units)

shaper.normalize(word, strict=False)   # keep the input if it is uncovered
```

`canonical_version` is the frozen name of the exact selection policy, currently
`'mng-canonical/2'` (it raises `RuntimeError` for a locale with no normalize table). Applications
that persist normalized search or index keys should store this version alongside them and rebuild
those keys if a future release changes it.

```python
shaper.canonical_version   # → 'mng-canonical/2'
```

**`mng-canonical/2` (0.2.0) invalidates keys stored under `mng-canonical/1`.** Unifying the nine
duplicate encodings changed the canonical text of every word containing one — 288 of the 1993
corpus shape groups, four of which merged with another group. Rebuild any stored normalized key.

#### Written-unit input

When you already have a shape rather than nominal Unicode, encode it directly.

```python
shaper.normalize_written_units(["B", "Aa"])
# → 'ᠪᠠ᠋'

# The output of shape() is accepted as-is, structural controls included
shaper.normalize_written_units(["S", "A", "I", "I", "N", "Mvs", "Aa"])
```

`normalize_written_units()` accepts an ordered `Sequence[str]` of shape units, not nominal Unicode.
Letter positions are inferred from unit order and structural controls; explicit position records
are not accepted by this API. All written-unit names use PascalCase; structural controls are `Mvs`,
`Nirugu` and `Zwj`, exactly as returned by `shape()`. Old lowercase or all-uppercase control aliases
are not accepted. The API never infers or inserts a structural control: ZWJ is present in the output
only when `Zwj` is present in the requested sequence. An empty sequence returns an empty string. A
malformed outer input or a non-string item raises `TypeError`; unknown units and sequences that
cannot reshape exactly raise `ValueError` rather than being guessed or partially encoded.

```python
# Authoritative HUD written-unit positions, without inferring controls
shaper.normalize_positioned_written_units([
    {"unit": "B", "position": "init"},
    {"unit": "Aa", "position": "fina"},
])
# → 'ᠪᠠ᠋'

# HUD position is not Unicode topology: isolated FA borrows F:init
shaper.normalize_positioned_written_units([{"unit": "F", "position": "init"}])
# → 'ᠹ' (bare U+1839, no ZWJ); F:isol is unsupported

# O:init reuses the O+A canonical prefix, then adds the required trailing ZWJ
shaper.normalize_positioned_written_units([{"unit": "O", "position": "init"}])
# → U+1824 U+180B U+200D
```

`normalize_positioned_written_units()` accepts an ordered sequence of exact built-in
`{"unit": str, "position": str}` dict records. Here `position` is the position of the written unit
in the authoritative HUD inventory, not a Unicode letter's joining topology. It reuses
`normalize_written_units()` rather than a second encoding table. A complete multi-record chain
starts with `init` and ends with `fina`. An incomplete left or right edge gets an implicit `Zwj`;
for example `B:medi, O:medi, G:fina` is normalized as `Zwj, B, O, G`. A single `init` record is
normally normalized without ZWJ, so `F:init` becomes bare U+1839. The sole exception is `O:init`: it
reuses the U+1824 U+180B prefix selected by canonical `O:init, A:fina`, then adds U+200D. A single
`medi` gets ZWJ on both sides and a single `fina` gets ZWJ on the left. `F:isol` is absent from the
source inventory and fails closed. `Mvs` and `Nirugu` use `control`; explicit `Zwj` input is
rejected. A wrong outer/record/field type raises `TypeError`; wrong keys, unit, position, chain
positions, exact encoding, or more than 1024 records raises `ValueError`. This API has no CLI
subcommand yet.

`parse_written_units(text)` parses the CLI spelling of a written-unit sequence into unit names:

```python
shaper.parse_written_units("B+Aa")   # → ['B', 'Aa']
shaper.parse_written_units("BAa")    # → ['B', 'Aa']   (compact, if unambiguous)
```

It raises `ValueError` when the text is malformed or names a unit the normalize table does not know.

### Command line

Installing the package puts `mongol-norm` on `PATH`. It is the crate's own CLI — the same code
`cargo install mongol-norm` gives you as a standalone binary — run in-process by the console script.
`mongol-norm --version` prints the package version and `mongol-norm --help` lists every flag.

```bash
# Inline text
mongol-norm shape 'ᠰᠠᠢᠨ'                        # → S+A+I+I+A
mongol-norm normalize 'ᠰᠡᠢᠨ'                    # canonical form
mongol-norm normalize --allow-fallback 'ᠰᠡᠢᠨ'   # keep the input if it is uncovered
mongol-norm normalize-text 'Hello ᠰᠡᠢᠨ'         # mixed script
mongol-norm normalize-written-units 'B+Aa'      # → ᠪᠠ᠋
mongol-norm normalize-written-units 'BZwj'      # compact PascalCase units
mongol-norm same 'ᠰᠠᠢᠨ' 'ᠰᠡᠢᠨ'                  # exit 0 if identical, 1 if different

# Pipe / stdin (use `-` as the text)
echo 'B+Aa' | mongol-norm normalize-written-units -
cat doc.txt | mongol-norm normalize-text -

# File in / out
mongol-norm normalize-text -i in.txt -o out.txt

# Batch: one word per line in, one canonical per line out
mongol-norm normalize --batch -i words.txt -o canonical.txt
echo 'ᠰᠡᠢᠨ' | mongol-norm normalize --batch -
```

`--locale MNG|TOD|SIB|MCH` selects the script (default `MNG`; only `MNG` normalizes). It is a
*global* option and goes before the sub-command — `mongol-norm --locale TOD shape 'ᠰᠠᠢᠨ'`, not
after it. `--` ends the options so a following `-` is text, and errors exit with code 2 (`same`
prints `true`/`false` and exits 0/1).

`shape` and `normalize` take a single word and reject every character outside the Mongolian word
alphabet — including the newline `echo` appends, so pipe them with `printf`, with `--batch` (one
word per line), or use `normalize-text` for free-form text.

`normalize-written-units` accepts compact PascalCase or explicit `+` boundaries. Compact input must
have one unique segmentation; ambiguous input fails closed and must be rewritten with `+`. After
parsing, the same exact-shape validation as `normalize_written_units()` applies, so a syntactically
valid unit stream can still be rejected when it has no canonical MNG encoding.

### Upgrading from 0.0.x

The public API — `MongolianShaper`, `NormalizationFallbackError` and the `mongol-norm` command — is
unchanged, and `from mongol_norm.shaper import …` as well as `python -m mongol_norm.shaper …` still
work through a compatibility shim. Differences:

- Python ≥ 3.9 is required.
- An unknown locale raises `ValueError` instead of `FileNotFoundError`.
- `mongol_norm.rules` and the private shaper internals no longer exist; `mongol_norm.shaper` only
  re-exports.
- The CLI is the Rust crate's, which adds `-V/--version`; its remaining intentional differences from
  the old argparse CLI (help-text layout, error prefixes and exit codes) are listed at the top of
  [`src/cli.rs`](https://github.com/Satsrag/mongol-norm/blob/main/src/cli.rs).
- New in 0.1: `shaper.trace()`, `shaper.rule_names()`, `shaper.parse_written_units()`.

### Requirements

- CPython ≥ 3.9 (CI-tested on 3.9 / 3.10 / 3.11 / 3.12 / 3.13 / 3.14)
- No runtime dependencies — the engine and its tables are compiled into the extension module
- Prebuilt wheels on the platforms listed above; elsewhere the sdist builds with Rust ≥ 1.83

The package also ships `mongol_norm/data/*.json` — the flat, language-agnostic shaping and normalize
rules — for tooling. The runtime never reads them; the
[data format](https://github.com/Satsrag/mongol-norm/blob/main/docs/data-format.md) is documented so
the normalizer can be ported to any language with just a JSON parser.

### Links

- Source, issues and the full documentation: <https://github.com/Satsrag/mongol-norm>
- The Rust engine: [crates.io/crates/mongol-norm](https://crates.io/crates/mongol-norm) · [docs.rs/mongol-norm](https://docs.rs/mongol-norm)
- Data format for other-language ports: [`docs/data-format.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/data-format.md)

### License

MIT License — see [LICENSE](https://github.com/Satsrag/mongol-norm/blob/main/LICENSE). The shaping
rules and bundled data are derived from
[mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) (MIT) and UTN #57; their required
notices are retained in [NOTICE](https://github.com/Satsrag/mongol-norm/blob/main/NOTICE).

---

## 中文

`mongol-norm` 完整实现 [UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) 蒙古文整形流程，
以及 FVS 钉死的 canonical 规范化器。同一个可见的蒙古文词可以写成多种不同的 Unicode 序列，这会破坏搜索、
去重和索引。`shape()` 把词渲染成书写单元序列——即字体会画出的形状，但无需字体——`normalize()` 则把同一个
词的任意编码改写成该形态唯一的 canonical 编码。

本包是 Rust crate [`mongol-norm`](https://crates.io/crates/mongol-norm) 之上的一层薄
[PyO3](https://pyo3.rs) 绑定，二者在
[同一个仓库](https://github.com/Satsrag/mongol-norm)开发。**只有一份实现**：一套数据表、一套语料与
golden 固件、一个版本号——`pip install mongol-norm` 与同版本 crate 的结果逐字节相同。

### 安装

```bash
pip install mongol-norm
```

引擎及其 shaping / normalize 数据表已编译进 `mongol_norm._native` 扩展，因此本包**零运行时依赖**，也不需要
安装 Rust 工具链。

预编译的 `cp39-abi3` wheel（每个平台一个，服务全部 CPython ≥ 3.9）覆盖：

| 平台 | wheel |
|---|---|
| Linux x86_64 / aarch64 | glibc（manylinux2014）与 musl（musllinux_1_2） |
| macOS | x86_64 与 Apple silicon（arm64） |
| Windows | x64 |

其他平台 pip 会回退到源码包（sdist），在本地编译扩展，需要 Rust ≥ 1.83 工具链（`maturin` 构建后端由 pip
自动获取）。

### 快速开始

```python
from mongol_norm import MongolianShaper

shaper = MongolianShaper(locale="MNG")   # Hudum 传统蒙文

# 字形化：字体会画出的书写单元序列
shaper.shape("ᠰᠠᠢᠨ")
# → ['S', 'A', 'I', 'I', 'A']

# 比较：两个编码渲染结果是否相同？
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")
# → True
shaper.same_shape("ᠰᠠᠢᠨ", "ᠨᠠᠢ᠍ᠮᠠ")
# → False

# 规范化：同一个词的多种编码 → 唯一的 FVS 钉死 canonical 形式
shaper.normalize("ᠰᠡᠢᠨ")     # → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'
shaper.normalize("ᠰᠠᠶ᠋ᠢᠨ")   # → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'
shaper.normalize("ᠰᠠᠶ᠋ᠶ᠋ᠨ")   # → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'
```

于是给一组编码去重就只是一个 `set`：

```python
words = ["ᠰᠡᠢᠨ", "ᠰᠠᠢᠨ", "ᠰᠨ᠌ᠢᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ"]
unique = {shaper.normalize(w) for w in words}
print(f"{len(words)} 个输入 → {len(unique)} 个唯一形态：{unique}")
# 4 个输入 → 1 个唯一形态：{'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'}
```

locale 可取 `"MNG"`（回鹘式）、`"TOD"`（托忒文）、`"SIB"`（锡伯文）、`"MCH"`（满文）；未知 locale 抛
`ValueError`。只有 `MNG` 有规范化表，其余三种只做整形。

### API

公开接口就是 `MongolianShaper(locale="MNG")` 与异常 `NormalizationFallbackError`。

#### 整形

```python
shaper.shape("ᠰᠠᠢᠨ")       # → ['S', 'A', 'I', 'I', 'A']
shaper.shape_str("ᠰᠠᠢᠨ")   # → 'S+A+I+I+A'   （"+".join(shape(text))）
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")   # → True
```

`shape()` 与 `normalize()` 处理**单个词**，遇到蒙古文词字母表（字母、FVS、MVS、NNBSP、nirugu、ZWJ）之外
的字符抛 `ValueError`。混合文字请用 `normalize_text()`。

**公开 shape 统一了九个重复编码**——即与另一串单元渲染出完全相同墨迹的书写单元。其中五个靠展开统一：
`Dd`（它仅有的两个位置）、词中 `H`、词中 `Hx`、词首 `Cr` 分别输出为 `O A`、`A A`、`N N`、`O O`。另四个不能
展开——它们的展开式以 `Aa` 结尾，而 `Aa` 本身就是重复编码，永不收敛——改用反方向统一，把单元对收缩成单个单元：
chain 末尾的 `A Aa` 变成 `Aa`（独占整条 chain 时变成 `A`），`O Aa` 变成 `B2`，`I Aa` 变成 `G`。这正是
`shape()` 能作为**可见**词指纹的原因：

```python
shaper.shape("ᠠᠷᠠᠳ")                  # → ['A', 'A', 'R', 'A', 'O', 'A']
shaper.same_shape("ᠠᠷᠠᠳ", "ᠠᠷᠠᠤᠠ")     # → True（同一个词的两种拼法）
shaper.shape("ᠪᠠᠠ᠋")                  # → ['B', 'Aa']
shaper.same_shape("ᠪᠠ", "ᠪᠠᠠ᠋")        # → True
```

完整规则表、见证词对与收敛性论证见项目 README 的“重复编码”一节。

UTN #57 与 GB/T 25914-2023 把这九个保留为不同的书写单元——其 EAC 向量把 ᠠᠷᠭᠠᠯ 拼作 `A A R Hx A L`——
引擎也仍然产出它们。国标自己的序列是 `shaper._shape_raw(text)`，它为一致性套件而存在，**不属于公开契约**
（故带前导下划线），将来可能在不升 major 的情况下统一更多重复编码。`shape_detailed()` 与
`trace()['written_by_token']` 报告的是每个 token 自身的单元，因此同样是原始序列——统一是整词级改写，单个
token 承载不了；`trace()['shape']` 则是公开的统一序列。

`shape_detailed(text)` 逐 token 返回一个 dict——码位、locale alias、连接位置、FVS 选择符、选中该变体的
shaping condition，以及它渲染出的书写单元：

```python
shaper.shape_detailed("ᠪᠠ")
# → [{'cp': 'U+182A', 'alias': 'b', 'position': 'init', 'fvs': '',
#     'condition': '', 'written': ['B']},
#    {'cp': 'U+1820', 'alias': 'a', 'position': 'fina', 'fvs': '',
#     'condition': 'post_bowed', 'written': ['Aa']}]
```

`trace(text)` 返回项目 golden 固件所用的逐规则管线轨迹——
`{"positions", "transitions", "final_conditions", "written_by_token", "shape"}`，其中每条 transition 是
`{"rule": 名称, "changes": [{"token", "before", "after"}, ...]}`，且只列出至少改变了一个 condition 的规则：

```python
shaper.trace("ᠪᠠ")["transitions"]
# → [{'rule': 'III.5.post_bowed',
#     'changes': [{'token': 1, 'before': None, 'after': 'post_bowed'}]}]

shaper.rule_names()
# → ['III.1.chachlag', 'III.2a.o_u_oe_ue.marked', ..., 'III.5.post_bowed']
```

#### 规范化

```python
shaper.normalize("ᠰᠡᠢᠨ")                        # 单个词
shaper.normalize_text("Hello ᠰᠡᠢᠨ world")       # 自由文本
# → 'Hello ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌ world'
```

`normalize_text()` 独立规范化每个蒙古文词，原样保留空格、标点和非蒙古文文本——句子、段落和混合文字都用它。

两者默认 `strict=True`：内置规范化表无法编码的 written-unit shape 会抛 `NormalizationFallbackError`，其
`.text` 与 `.written_units` 属性带回原输入和未覆盖的 shape。只有在能接受原样返回时才显式传
`strict=False`——两种模式都保住往返，API 绝不错编。

```python
from mongol_norm import NormalizationFallbackError   # ValueError 的子类

try:
    canonical = shaper.normalize(word)
except NormalizationFallbackError as exc:
    print(exc.text, exc.written_units)

shaper.normalize(word, strict=False)   # 未覆盖时原样返回
```

`canonical_version` 是冻结的精确选择策略名，当前为 `'mng-canonical/2'`（没有规范化表的 locale 会抛
`RuntimeError`）。持久化规范化搜索键 / 索引键的应用应同时保存该版本；未来版本若发生变化，应重建这些键。

```python
shaper.canonical_version   # → 'mng-canonical/2'
```

**`mng-canonical/2`（0.2.0）会使 `mng-canonical/1` 下存储的键失效。** 统一九个重复编码之后，凡含有其中
之一的词，canonical 文本都变了——1993 个语料 shape 组里有 288 个，其中 4 个与别的组合并。已存储的规范化键
必须重建。

#### 书写单元输入

如果手上已经是 shape 而不是 nominal Unicode，可以直接编码。

```python
shaper.normalize_written_units(["B", "Aa"])
# → 'ᠪᠠ᠋'

# shape() 的输出可直接使用，包含结构 control
shaper.normalize_written_units(["S", "A", "I", "I", "N", "Mvs", "Aa"])
```

`normalize_written_units()` 接受由 shape unit 组成的有序 `Sequence[str]`，而不是 nominal Unicode。字母
位置由单元顺序与结构 control 推导；此 API 不接受显式 position record。所有 written-unit 名称统一使用
PascalCase；结构 control 为 `Mvs`、`Nirugu`、`Zwj`，与 `shape()` 输出完全一致。旧的小写或全大写 control
别名不再接受。API 绝不自行推断或插入结构 control：只有显式包含 `Zwj` 时，输出才会包含 ZWJ。空序列返回
空字符串。非法外层输入或非字符串单元抛 `TypeError`；未知 unit 或无法精确重新 shape 的序列抛
`ValueError`，不会猜测或返回部分编码结果。

```python
# 按权威 HUD written-unit position 编码，不推断或插入 control
shaper.normalize_positioned_written_units([
    {"unit": "B", "position": "init"},
    {"unit": "Aa", "position": "fina"},
])
# → 'ᠪᠠ᠋'

# HUD position 不是 Unicode topology：FA 的 isolated variant 借用 F:init
shaper.normalize_positioned_written_units([{"unit": "F", "position": "init"}])
# → 'ᠹ'（裸 U+1839，不含 ZWJ）；F:isol 不受支持

# O:init 复用 O+A canonical 前缀，再添加所需的尾部 ZWJ
shaper.normalize_positioned_written_units([{"unit": "O", "position": "init"}])
# → U+1824 U+180B U+200D
```

`normalize_positioned_written_units()` 接受由严格内建 `{"unit": str, "position": str}` dict record 组成
的有序序列。这里的 `position` 表示权威 HUD inventory 中的 written-unit position，不是 Unicode 字母在当前
序列中的 joining topology。它直接复用 `normalize_written_units()`，不再维护第二套编码表。完整复合链必须
以 `init` 开头、以 `fina` 结束；左端或右端不完整时自动补 `Zwj`，例如 `B:medi, O:medi, G:fina` 会按
`Zwj, B, O, G` 规范化。单个 `init` 通常不补 ZWJ，因此 `F:init` 输出裸 U+1839；唯一特例 `O:init` 复用
canonical `O:init, A:fina` 选出的 U+1824 U+180B 前缀，再添加 U+200D。单个 `medi` 前后补 ZWJ，单个 `fina`
只在左侧补 ZWJ。inventory 中不存在的 `F:isol` 会 fail closed。`Mvs` 与 `Nirugu` 使用 `control`，显式
`Zwj` 输入被拒绝。外层 / record / 字段类型错误抛 `TypeError`；keys、unit、position、复合链位置、exact
encoding 错误以及超过 1024 条 record 均抛 `ValueError`。本 API 暂不提供 CLI 子命令。

`parse_written_units(text)` 把命令行写法的书写单元序列解析成单元名：

```python
shaper.parse_written_units("B+Aa")   # → ['B', 'Aa']
shaper.parse_written_units("BAa")    # → ['B', 'Aa']   （紧凑写法，需无歧义）
```

文本非法、或所命名的 unit 不在规范化表内时抛 `ValueError`。

### 命令行

安装本包后，`mongol-norm` 命令即在 `PATH` 上。它就是 crate 自带的 CLI——`cargo install mongol-norm` 会把
同一份代码装成独立二进制——由控制台脚本在进程内运行。`mongol-norm --version` 输出包版本，
`mongol-norm --help` 列出全部参数。

```bash
# 直接传文本
mongol-norm shape 'ᠰᠠᠢᠨ'                        # → S+A+I+I+A
mongol-norm normalize 'ᠰᠡᠢᠨ'                    # 输出 canonical
mongol-norm normalize --allow-fallback 'ᠰᠡᠢᠨ'   # 未覆盖时原样返回
mongol-norm normalize-text 'Hello ᠰᠡᠢᠨ'         # 混合文字
mongol-norm normalize-written-units 'B+Aa'      # → ᠪᠠ᠋
mongol-norm normalize-written-units 'BZwj'      # 紧凑 PascalCase 单元串
mongol-norm same 'ᠰᠠᠢᠨ' 'ᠰᠡᠢᠨ'                  # 同形退出码 0，不同为 1

# 管道 / 标准输入（文本位置写 `-`）
echo 'B+Aa' | mongol-norm normalize-written-units -
cat doc.txt | mongol-norm normalize-text -

# 文件输入 / 输出
mongol-norm normalize-text -i in.txt -o out.txt

# 批量：一行一词输入，一行一个 canonical 输出
mongol-norm normalize --batch -i words.txt -o canonical.txt
echo 'ᠰᠡᠢᠨ' | mongol-norm normalize --batch -
```

`--locale MNG|TOD|SIB|MCH` 选文种（默认 `MNG`，只有 `MNG` 支持规范化）。它是**全局**选项，要写在子命令
之前——`mongol-norm --locale TOD shape 'ᠰᠠᠢᠨ'`，不能写在子命令后面。`--` 结束选项（其后的 `-` 当文本），
出错退出码 2（`same` 打印 `true`/`false`，退出码 0/1）。

`shape` 和 `normalize` 处理单个词，拒绝蒙古文词字母表之外的任何字符——包括 `echo` 追加的换行，所以管道
请用 `printf`，或者用 `--batch`（一行一词），自由文本请用 `normalize-text`。

`normalize-written-units` 接受紧凑 PascalCase 或显式 `+` 边界。紧凑输入必须只有一种合法切分；存在歧义时
fail closed，须改用 `+`。解析后继续执行与 `normalize_written_units()` 相同的 exact-shape 校验，因此语法
合法的 unit stream 若没有 canonical MNG 编码仍会被拒绝。

### 从 0.0.x 升级

公开 API——`MongolianShaper`、`NormalizationFallbackError` 和 `mongol-norm` 命令——保持不变；
`from mongol_norm.shaper import …` 与 `python -m mongol_norm.shaper …` 通过兼容 shim 仍可用。差异：

- 需要 Python ≥ 3.9。
- 未知 locale 抛 `ValueError`（原为 `FileNotFoundError`）。
- `mongol_norm.rules` 及 shaper 私有内部实现已移除；`mongol_norm.shaper` 只做 re-export。
- 命令行改为 Rust crate 自带的 CLI，新增 `-V/--version`；其余相对旧 argparse CLI 的刻意差异（帮助文本
  排版、错误前缀与退出码）见
  [`src/cli.rs`](https://github.com/Satsrag/mongol-norm/blob/main/src/cli.rs) 顶部。
- 0.1 新增：`shaper.trace()`、`shaper.rule_names()`、`shaper.parse_written_units()`。

### 环境要求

- CPython ≥ 3.9（CI 实测矩阵：3.9 / 3.10 / 3.11 / 3.12 / 3.13 / 3.14）
- 无运行时依赖——引擎及其数据表已编译进扩展模块
- 上表所列平台提供预编译 wheel；其他平台用 sdist 构建，需要 Rust ≥ 1.83

本包同时随附 `mongol_norm/data/*.json`——扁平、语言无关的 shaping 与 normalize 规则——供工具脚本使用；
运行时从不读取它们。[数据格式](https://github.com/Satsrag/mongol-norm/blob/main/docs/data-format.md)
有完整文档，其他语言只需一个 JSON 解析器即可移植规范化器。

### 链接

- 源码、issue 与完整文档：<https://github.com/Satsrag/mongol-norm>
- Rust 引擎：[crates.io/crates/mongol-norm](https://crates.io/crates/mongol-norm) · [docs.rs/mongol-norm](https://docs.rs/mongol-norm)
- 其他语言移植所需的数据格式：[`docs/data-format.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/data-format.md)

### 许可证

MIT License —— 见 [LICENSE](https://github.com/Satsrag/mongol-norm/blob/main/LICENSE)。整形规则与内置
数据派生自 [mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder)（MIT）和 UTN #57，其许可证
要求的署名保留在 [NOTICE](https://github.com/Satsrag/mongol-norm/blob/main/NOTICE) 中。
