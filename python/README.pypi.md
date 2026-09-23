# mongol-norm

[![PyPI](https://img.shields.io/pypi/v/mongol-norm.svg)](https://pypi.org/project/mongol-norm/)
[![Python versions](https://img.shields.io/pypi/pyversions/mongol-norm.svg)](https://pypi.org/project/mongol-norm/)
[![crates.io](https://img.shields.io/crates/v/mongol-norm.svg)](https://crates.io/crates/mongol-norm)
[![Test](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml/badge.svg)](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml)

Shape-aware normalizer for Traditional Mongolian (Hudum) script.
传统蒙古文（回鹘式）形态感知规范化器。*中文见下半部分。*

---

## English

> **Beta.** `shape` / `same_shape` should be stable. The `normalize` output currently encodes by
> **glyph shape**, not the standard **phonetic (nominal-character) spelling**, and may change in a
> later release. For example ᠮᠣᠩᠭᠣᠯ (`MA+O+ANG+GA+O+LA`) normalizes to ᠮᠣᠩᠨ᠋ᠨᠣᠯ
> (`MA+O+ANG+NA+FVS1+NA+O+LA`). If you store normalized keys, store
> `shaper.canonical_version` with them and rebuild when it changes.

### Why

The same visible Mongolian word can be typed as many different Unicode sequences (A/E look alike,
O/U share forms, FVS and joiners give the same glyph several ways), so search, deduplication and
indexing break. `mongol-norm` runs the [UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html)
shaping rules — no font needed — to compare words by what they look like and map every encoding
of a word to one key.

This package is a thin binding over the Rust crate
[`mongol-norm`](https://crates.io/crates/mongol-norm); both give byte-identical results.

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

```python
from mongol_norm import MongolianShaper

shaper = MongolianShaper(locale="MNG")

shaper.shape("ᠰᠠᠢᠨ")                        # → ['S', 'A', 'I', 'I', 'A']
shaper.shape_str("ᠰᠠᠢᠨ")                    # → 'S+A+I+I+A'
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")           # → True
shaper.normalize("ᠰᠡᠢᠨ")                    # → 'ᠰᠠᠢ᠍ᠢᠨ'
shaper.normalize_text("Hello ᠰᠡᠢᠨ world")   # → 'Hello ᠰᠠᠢ᠍ᠢᠨ world'
shaper.canonical_version                    # → 'mng-canonical/3'

# Deduplicate
{shaper.normalize(w) for w in ["ᠰᠡᠢᠨ", "ᠰᠠᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ"]}   # → {'ᠰᠠᠢ᠍ᠢᠨ'}
```

- `shape` / `normalize` take **one word** and raise `ValueError` on non-Mongolian characters; use
  `normalize_text` for sentences and mixed-script text.
- A shape the built-in table cannot encode raises `NormalizationFallbackError` (a `ValueError`,
  with `.text` and `.written_units`). Pass `strict=False` to get the input back unchanged instead.
- Locales: `"MNG"` (Hudum), `"TOD"`, `"SIB"`, `"MCH"`. Only `MNG` normalizes; the others shape only.

Written-unit input, if you already have a shape:

```python
shaper.normalize_written_units(["B", "Aa"])            # → 'ᠪᠠ'
shaper.normalize_positioned_written_units([
    {"unit": "B", "position": "init"},
    {"unit": "Aa", "position": "fina"},
])                                                     # → 'ᠪᠠ'
shaper.parse_written_units("B+Aa")                     # → ['B', 'Aa']
```

Debugging: `shape_detailed(text)` (per-letter breakdown), `trace(text)` (rule-by-rule trace),
`rule_names()`.

### Command line

```bash
mongol-norm shape 'ᠰᠠᠢᠨ'                  # → S+A+I+I+A
mongol-norm same 'ᠰᠠᠢᠨ' 'ᠰᠡᠢᠨ'            # exit 0 if identical, 1 if not
mongol-norm normalize 'ᠰᠡᠢᠨ'
mongol-norm normalize-text -i in.txt -o out.txt
mongol-norm normalize --batch -i words.txt -o keys.txt   # one word per line
```

`mongol-norm --help` lists every option.

### Links

- Source, issues, docs: <https://github.com/Satsrag/mongol-norm>
- How it works: [`docs/internals.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/internals.md)
- License: MIT. Rules and data derived from UTN #57 and
  [mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) (MIT); see
  [NOTICE](https://github.com/Satsrag/mongol-norm/blob/main/NOTICE).

---

## 中文

> **Beta 版。** `shape` / `same_shape` 应该是稳定的。`normalize` 的输出目前是**按字形**编码的，**不是**按读音
> 书写的标准名义字符序列，后续版本有可能修改。例如 ᠮᠣᠩᠭᠣᠯ（`MA+O+ANG+GA+O+LA`）规范化后是 ᠮᠣᠩᠨ᠋ᠨᠣᠯ
> （`MA+O+ANG+NA+FVS1+NA+O+LA`）。如果要持久化规范化后的
> key，请同时保存 `shaper.canonical_version`，版本变化时重建。

### 为什么

同一个看起来一样的蒙古文词可以用很多种 Unicode 序列输入（A/E 同形、O/U 共享字形、FVS 和连接符能用不同
方式得到同一字形），导致搜索、去重、索引失效。`mongol-norm` 按
[UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) 整形规则（不需要字体）按外形比较词，并把同一个
词的所有编码映射成同一个 key。

本包是 Rust crate [`mongol-norm`](https://crates.io/crates/mongol-norm) 的薄绑定，结果与 crate 逐字节相同。

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

```python
from mongol_norm import MongolianShaper

shaper = MongolianShaper(locale="MNG")

shaper.shape("ᠰᠠᠢᠨ")                        # → ['S', 'A', 'I', 'I', 'A']
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")           # → True
shaper.normalize("ᠰᠡᠢᠨ")                    # → 'ᠰᠠᠢ᠍ᠢᠨ'
shaper.normalize_text("Hello ᠰᠡᠢᠨ world")   # → 'Hello ᠰᠠᠢ᠍ᠢᠨ world'

# 去重
{shaper.normalize(w) for w in ["ᠰᠡᠢᠨ", "ᠰᠠᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ"]}   # → {'ᠰᠠᠢ᠍ᠢᠨ'}
```

- `shape` / `normalize` 只处理**单个词**，遇到非蒙古文字符抛 `ValueError`；句子或混合文本用 `normalize_text`。
- 内置表无法编码的 shape 抛 `NormalizationFallbackError`（`ValueError` 子类，带 `.text`、`.written_units`）；
  传 `strict=False` 则原样返回。
- locale：`"MNG"`（回鹘式）、`"TOD"`、`"SIB"`、`"MCH"`，只有 `MNG` 支持规范化。

书写单元输入、调试接口和命令行见上方英文部分，用法相同。
