# mongol-norm

[![PyPI](https://img.shields.io/pypi/v/mongol-norm.svg)](https://pypi.org/project/mongol-norm/)
[![Python versions](https://img.shields.io/pypi/pyversions/mongol-norm.svg)](https://pypi.org/project/mongol-norm/)
[![crates.io](https://img.shields.io/crates/v/mongol-norm.svg)](https://crates.io/crates/mongol-norm)
[![Test](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml/badge.svg)](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml)

mongol-norm is a Traditional Mongolian (Hudum) same-shape comparison and normalization tool.
传统蒙古文（回鹘式）同形比较与规范化工具。*中文见下半部分。*

---

## English

**mongol-norm** is a Traditional Mongolian (Hudum) same-shape comparison and normalization tool.
It is implemented according to the Mongolian shaping rules of UTN #57 (GB/T 25914—2023). It
converts Mongolian text that has the same written form but different Unicode encodings into a
unified encoding, making search, deduplication, and corpus processing easier.

This package is a thin binding over the Rust crate
[`mongol-norm`](https://crates.io/crates/mongol-norm); both give byte-identical results.

### Input requirements

This library is intended for Mongolian text under the UTN #57 (GB/T 25914—2023) rules. Text that
uses other standards or encodings — for example Delehi, Oyun, and Menk related to
GB/T 25914—2010, as well as Menk old shape codes, ZCode, Zvvnmod, and the like — must first be
converted to UTN #57 (GB/T 25914—2023) before this library can be used. For encoding conversion,
you may consider [`mongol-convert`](https://github.com/Satsrag/mongol-convert); that library is
also currently in Beta, so use it with care and verify the conversion results.

### An example

Under the UTN #57 v4 shaping model, the following spellings have the same written form, but
their Unicode encodings differ, so a direct string comparison treats them as different text.
mongol-norm recognizes them as having the same abstract shape and produces the same normalized
result.

![Five encodings of sain all normalizing to the same canonical form](https://github.com/Satsrag/mongol-norm/raw/main/assets/sain-variants.png)

### Install

```bash
pip install mongol-norm
```

CPython ≥ 3.9. The engine is compiled into the extension, so there are no runtime dependencies and
no Rust toolchain to install. Prebuilt `cp39-abi3` wheels:

| Platform | Wheels |
|---|---|
| Linux x86_64 / aarch64 | glibc (manylinux2014) and musl (musllinux_1_2) |
| macOS | x86_64 and Apple silicon (arm64) |
| Windows | x64 |

On any other platform pip falls back to the source distribution, which compiles the extension
locally and needs Rust ≥ 1.83. The install also puts the `mongol-norm` command on `PATH`.

### Usage

`shape` returns the canonical written-unit sequence under mongol-norm's abstract shaping model
(UTN #57 v4 plus `mongfontbuilder`-derived data and rules). `same_shape` reports whether two
encodings produce the same written-unit sequence under that model — not whether they render
identically in every font.

> **Beta.** `normalize` output is a unified encoding by **glyph shape**, **not standard spelling**,
> and may change in a later release. Keep the original text; when storing normalized keys, also
> record `shaper.canonical_version` and rebuild after a version change.

```python
from mongol_norm import MongolianShaper

shaper = MongolianShaper(locale="MNG")

shaper.shape("ᠰᠠᠢᠨ")          # U+1830 (SA) U+1820 (A) U+1822 (I) U+1828 (NA) → ['S', 'A', 'I', 'I', 'A']
shaper.shape("ᠰᠡᠢᠨ")          # U+1830 (SA) U+1821 (E) U+1822 (I) U+1828 (NA) → ['S', 'A', 'I', 'I', 'A']
shaper.shape("ᠰᠨ᠌ᠢᠢᠨ")        # U+1830 (SA) U+1828 (NA) U+180C (FVS2) U+1822 (I) U+1822 (I) U+1828 (NA) → ['S', 'A', 'I', 'I', 'A']
shaper.shape("ᠰᠠᠶ᠋ᠢᠨ")        # U+1830 (SA) U+1820 (A) U+1836 (YA) U+180B (FVS1) U+1822 (I) U+1828 (NA) → ['S', 'A', 'I', 'I', 'A']
shaper.shape("ᠰᠠᠶ᠋ᠶ᠋ᠨ")      # U+1830 (SA) U+1820 (A) U+1836 (YA) U+180B (FVS1) U+1836 (YA) U+180B (FVS1) U+1828 (NA) → ['S', 'A', 'I', 'I', 'A']
shaper.shape_str("ᠰᠠᠢᠨ")                    # → 'S+A+I+I+A'
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")           # → True
shaper.normalize("ᠰᠡᠢᠨ")                    # → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'
shaper.normalize_text("Hello ᠰᠡᠢᠨ world")   # → 'Hello ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌ world'
shaper.canonical_version                    # → 'mng-canonical/5'

# Deduplicate
{shaper.normalize(w) for w in ["ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ", "ᠰᠨ᠌ᠢᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ", "ᠰᠠᠶ᠋ᠶ᠋ᠨ"]}   # → {'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'}
```

- `shape` / `normalize` take **one word** and raise `ValueError` on non-Mongolian characters; use
  `normalize_text` for sentences and mixed-script text.
- A shape the built-in table cannot encode raises `NormalizationFallbackError` (a `ValueError`,
  with `.text` and `.written_units`). Pass `strict=False` to get the input back unchanged instead.
- Locale: `"MNG"` (Hudum), the only one. Todo, Sibe and Manchu are not supported.

Written-unit input, if you already have a shape:

```python
shaper.normalize_written_units(["B", "Aa"])            # → 'ᠪᠠ᠋'
shaper.normalize_positioned_written_units([
    {"unit": "B", "position": "init"},
    {"unit": "Aa", "position": "fina"},
])                                                     # → 'ᠪᠠ᠋'
shaper.parse_written_units("B+Aa")                     # → ['B', 'Aa']
```

Debugging: `shape_detailed(text)` (per-letter breakdown), `trace(text)` (rule-by-rule trace),
`rule_names()`.

### Command line

```bash
mongol-norm shape 'ᠰᠠᠢᠨ'                  # → S+A+I+I+A
mongol-norm same 'ᠰᠠᠢᠨ' 'ᠰᠡᠢᠨ'            # exit 0 if same model shape, 1 if not
mongol-norm normalize 'ᠰᠡᠢᠨ'
mongol-norm normalize-text -i in.txt -o out.txt
mongol-norm normalize --batch -i words.txt -o keys.txt   # one word per line
```

`mongol-norm --help` lists every option.

### Model and limitations

`shape()` and `same_shape()` implement **Mongolian-specific shaping equivalence** under
mongol-norm's abstract shaping model — **UTN #57 v4** (GB/T 25914—2023) plus
[mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder)-derived data and rules,
together with mongol-norm's control-character canonicalization and duplicate-unit unification.
It does not invoke HarfBuzz with a specific font or compare rendered glyph IDs or outlines.

For example, `"ᠰᠠᠢᠨ"` (`SA + A + I + NA`) and `"ᠰᠨ᠌ᠢᠢᠨ"` (`SA + NA+FVS2 + I + I + NA`) both
shape to `['S', 'A', 'I', 'I', 'A']`, so `same_shape()` returns `True`. A real-world font that
follows an older standard (such as GB/T 25914—2010), has incomplete FVS support, or uses
different OpenType rules may still render them differently. Applications requiring a particular
font's final rendering to match should validate directly with HarfBuzz and that font.

### Links

- Source, issues, docs: <https://github.com/Satsrag/mongol-norm>
- How it works: [`docs/internals.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/internals.md)
- License: MIT. Rules and data derived from UTN #57 and
  [mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) (MIT); see
  [NOTICE](https://github.com/Satsrag/mongol-norm/blob/main/NOTICE).

---

## 中文

**mongol-norm** 是一个传统蒙古文（回鹘式）的同形比较与规范化工具，依据 UTN #57（GB/T 25914—2023）的蒙古文整形规则实现。它将书写形态相同、Unicode 编码不同的蒙古文转换为统一编码，方便搜索、去重和语料处理。

本包是 Rust crate [`mongol-norm`](https://crates.io/crates/mongol-norm) 的薄绑定，结果与 crate 逐字节相同。

### 输入要求

本库面向 UTN #57（GB/T 25914—2023）规则下的蒙古文文本。采用其他标准或编码的文本，例如 GB/T 25914—2010 相关的 Delehi、Oyun、Menk，以及 Menk 旧形码、ZCode、Zvvnmod 等，需要先转换为 UTN #57（GB/T 25914—2023），才能使用本库。编码转换可考虑使用 [`mongol-convert`](https://github.com/Satsrag/mongol-convert)，该库目前也处于 Beta 阶段，请谨慎使用，并核对转换结果。

### 一个例子

在 UTN #57 v4 整形模型下，下面几种写法对应相同的书写形态，但 Unicode 编码不同，直接比较字符串会被当作不同文本。mongol-norm 可以判断它们在模型下同形，并生成相同的规范化结果。

![五种 sain 编码全部规范化为同一个形式](https://github.com/Satsrag/mongol-norm/raw/main/assets/sain-variants.png)

### 安装

```bash
pip install mongol-norm
```

CPython ≥ 3.9。引擎已编译进扩展模块，无运行时依赖，也不需要安装 Rust 工具链。预编译 `cp39-abi3` wheel：

| 平台 | wheel |
|---|---|
| Linux x86_64 / aarch64 | glibc（manylinux2014）与 musl（musllinux_1_2） |
| macOS | x86_64 与 Apple silicon（arm64） |
| Windows | x64 |

其他平台 pip 会回退到源码包，在本地编译扩展，需要 Rust ≥ 1.83。安装后同时会有 `mongol-norm` 命令。

### 用法

`shape` 返回 mongol-norm 抽象整形模型（UTN #57 v4 及源自 `mongfontbuilder` 的数据与规则）下的规范书写单元序列。`same_shape` 判断两种编码在该模型下是否得到相同的书写单元序列，并不保证在所有字体下渲染结果完全一致。

> **Beta 提醒：** `normalize` 输出的是**按字形统一的编码**，**不是标准拼写**，后续版本可能变化。建议保留原文；保存规范化键时，同时记录 `shaper.canonical_version`，版本变化后重建。

```python
from mongol_norm import MongolianShaper

shaper = MongolianShaper(locale="MNG")

shaper.shape("ᠰᠠᠢᠨ")          # U+1830 (SA) U+1820 (A) U+1822 (I) U+1828 (NA) → ['S', 'A', 'I', 'I', 'A']
shaper.shape("ᠰᠡᠢᠨ")          # U+1830 (SA) U+1821 (E) U+1822 (I) U+1828 (NA) → ['S', 'A', 'I', 'I', 'A']
shaper.shape("ᠰᠨ᠌ᠢᠢᠨ")        # U+1830 (SA) U+1828 (NA) U+180C (FVS2) U+1822 (I) U+1822 (I) U+1828 (NA) → ['S', 'A', 'I', 'I', 'A']
shaper.shape("ᠰᠠᠶ᠋ᠢᠨ")        # U+1830 (SA) U+1820 (A) U+1836 (YA) U+180B (FVS1) U+1822 (I) U+1828 (NA) → ['S', 'A', 'I', 'I', 'A']
shaper.shape("ᠰᠠᠶ᠋ᠶ᠋ᠨ")      # U+1830 (SA) U+1820 (A) U+1836 (YA) U+180B (FVS1) U+1836 (YA) U+180B (FVS1) U+1828 (NA) → ['S', 'A', 'I', 'I', 'A']
shaper.shape_str("ᠰᠠᠢᠨ")                    # → 'S+A+I+I+A'
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")           # → True
shaper.normalize("ᠰᠡᠢᠨ")                    # → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'
shaper.normalize_text("Hello ᠰᠡᠢᠨ world")   # → 'Hello ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌ world'
shaper.canonical_version                    # → 'mng-canonical/5'

# 去重
{shaper.normalize(w) for w in ["ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ", "ᠰᠨ᠌ᠢᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ", "ᠰᠠᠶ᠋ᠶ᠋ᠨ"]}   # → {'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'}
```

- `shape` / `normalize` 只处理**单个词**，遇到非蒙古文字符抛 `ValueError`；句子或混合文本用 `normalize_text`。
- 内置表无法编码的 shape 抛 `NormalizationFallbackError`（`ValueError` 子类，带 `.text`、`.written_units`）；
  传 `strict=False` 则原样返回。
- locale：只有 `"MNG"`（回鹘式），不支持托忒文、锡伯文、满文。

### 模型与局限

`shape()` 与 `same_shape()` 实现的是 mongol-norm 抽象整形模型下的**蒙古文整形等价**——基于
**UTN #57 v4**（GB/T 25914—2023）与源自
[mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) 的数据与规则，并结合本库的
控制符规范化与同形单元归一规则；不调用 HarfBuzz 加载具体字体，也不比较渲染后的字形 ID 或轮廓。

例如，`"ᠰᠠᠢᠨ"`（`SA + A + I + NA`）与 `"ᠰᠨ᠌ᠢᠢᠨ"`（`SA + NA+FVS2 + I + I + NA`）都整形为
`['S', 'A', 'I', 'I', 'A']`，因此 `same_shape()` 返回 `True`；但在旧标准字体、FVS 支持不完整
或 OpenType 规则不同的字体中，两者仍可能渲染不同。若应用需要确认某款特定字体的最终渲染一致，请直接使用
HarfBuzz 配合该字体进行验证。

书写单元输入、调试接口和命令行见上方英文部分，用法相同。
