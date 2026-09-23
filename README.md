# mongol-norm

[![crates.io](https://img.shields.io/crates/v/mongol-norm.svg)](https://crates.io/crates/mongol-norm)
[![docs.rs](https://img.shields.io/docsrs/mongol-norm)](https://docs.rs/mongol-norm)
[![PyPI](https://img.shields.io/pypi/v/mongol-norm.svg)](https://pypi.org/project/mongol-norm/)
[![Test](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml/badge.svg)](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml)

[English](#english) | [中文](#中文)

---

<a id="english"></a>
## English

> [!WARNING]
> **Beta.** `shape` / `same_shape` should be stable. The `normalize` output currently encodes by
> **glyph shape**, not the standard **phonetic (nominal-character) spelling**, and may change in a
> later release. For example ᠮᠣᠩᠭᠣᠯ (`MA+O+ANG+GA+O+LA`) normalizes to ᠮᠣᠩᠨ᠋ᠨᠣᠯ
> (`MA+O+ANG+NA+FVS1+NA+O+LA`). If you store normalized keys, store
> `canonical_version()` with them and rebuild when it changes.

**mongol-norm** tells whether two Traditional Mongolian (Hudum) strings look the same, and maps
them to one key. Rust crate with zero dependencies, plus a Python package and a CLI.

### Why

The same visible Mongolian word can be typed as many different Unicode sequences — A/E look alike
in medial and final position, O/U and OE/UE share forms, FVS selectors and joiners (nirugu, ZWJ)
can produce the same glyph in several ways. So search misses matches, deduplication fails and
index keys differ for words that look identical.

![Five encodings of "sain" all normalizing to the same canonical form](https://raw.githubusercontent.com/Satsrag/mongol-norm/main/assets/sain-variants.png)

mongol-norm runs the [UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) shaping rules
(no font needed) to get the *written units* a font would draw:

- `shape(word)` — the written-unit sequence, a fingerprint of the visible word
- `same_shape(a, b)` — do two encodings render identically?
- `normalize(word)` / `normalize_text(text)` — one canonical Unicode string per shape, and
  prefix-stable: the key of a word's beginning is the beginning of the word's key, apart from its
  last letter

Use it for search and indexing, deduplication, corpus counts, spell-check lookup, and cleaning OCR
or input-method output.

### Install

**Rust** (MSRV 1.82, zero dependencies, also builds for `wasm32-unknown-unknown`):

```bash
cargo add mongol-norm
```

or in `Cargo.toml`:

```toml
[dependencies]
mongol-norm = "0.2.3"
```

**Python** (CPython ≥ 3.9, no runtime dependencies, no Rust toolchain needed):

```bash
pip install mongol-norm
```

Prebuilt wheels cover Linux x86_64 / aarch64 (glibc and musl), macOS x86_64 / Apple silicon and
Windows x64. On other platforms pip builds from the source distribution, which needs Rust ≥ 1.83.

**Command line** — either of these puts `mongol-norm` on `PATH`:

```bash
cargo install mongol-norm
pip install mongol-norm
```

### Rust

```rust
use mongol_norm::{Error, Locale, Shaper};

fn main() -> Result<(), Error> {
    let shaper = Shaper::new(Locale::Mng);

    // Shape: the written units a font would draw
    assert_eq!(shaper.shape_str("ᠰᠠᠢᠨ")?, "S+A+I+I+A");

    // Compare: two encodings, one visible word
    assert!(shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")?);

    // Normalize: every encoding of a shape → the same string
    assert_eq!(shaper.normalize("ᠰᠡᠢᠨ")?, shaper.normalize("ᠰᠠᠶ᠋ᠢᠨ")?);

    // Free-form text: Mongolian words normalized, everything else kept
    let text = shaper.normalize_text("Hello ᠰᠡᠢᠨ world")?;
    assert!(text.starts_with("Hello ") && text.ends_with(" world"));
    Ok(())
}
```

Full API on [docs.rs](https://docs.rs/mongol-norm).

### Python

```python
from mongol_norm import MongolianShaper

shaper = MongolianShaper(locale="MNG")

shaper.shape("ᠰᠠᠢᠨ")                        # → ['S', 'A', 'I', 'I', 'A']
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")           # → True
shaper.normalize("ᠰᠡᠢᠨ")                    # → 'ᠰᠠᠢ᠍ᠢᠨ'
shaper.normalize_text("Hello ᠰᠡᠢᠨ world")   # → 'Hello ᠰᠠᠢ᠍ᠢᠨ world'

# Deduplicate
{shaper.normalize(w) for w in ["ᠰᠡᠢᠨ", "ᠰᠠᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ"]}   # → {'ᠰᠠᠢ᠍ᠢᠨ'}
```

More in the [Python README](https://github.com/Satsrag/mongol-norm/blob/main/python/README.pypi.md).

### Command line

```bash
mongol-norm shape 'ᠰᠠᠢᠨ'                  # → S+A+I+I+A
mongol-norm same 'ᠰᠠᠢᠨ' 'ᠰᠡᠢᠨ'            # exit 0 if identical, 1 if not
mongol-norm normalize 'ᠰᠡᠢᠨ'              # one word
mongol-norm normalize-text -i in.txt -o out.txt
mongol-norm normalize --batch -i words.txt -o keys.txt   # one word per line
```

`mongol-norm --help` lists every option.

### Notes

- Normalization covers **MNG (Hudum)** only. Todo, Sibe and Manchu (`TOD`/`SIB`/`MCH`) shape but
  do not normalize yet.
- A word whose shape the built-in table cannot encode fails with an error rather than being
  guessed; `normalize_allow_fallback` (Python: `strict=False`) returns it unchanged instead.
- Shaping passes 100% of mongfontbuilder's `core-hud` and GB/T 25914-2023 `eac-hud` suites;
  normalization round-trips every corpus word (`shape(normalize(x)) == shape(x)`).
- This project was written with [Claude Code](https://claude.ai/code); tests and core code were
  reviewed by hand. Please report problems as [issues](https://github.com/Satsrag/mongol-norm/issues).

### More

- [`docs/internals.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/internals.md) — shaping pipeline, duplicate encodings, normalization algorithm
- [`docs/data-format.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/data-format.md) — JSON rule tables, for ports to other languages
- [`docs/development.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/development.md) — repository layout, running the tests
- [`docs/releasing.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/releasing.md) — cutting a release

### License

MIT — see [LICENSE](https://github.com/Satsrag/mongol-norm/blob/main/LICENSE). Shaping rules and
data are derived from [UTN #57](https://www.unicode.org/notes/tn57/tn57-4.html) and
[mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) (MIT) by Kushim Jiang; test
suites from mongfontbuilder and GB/T 25914-2023. Notices in
[NOTICE](https://github.com/Satsrag/mongol-norm/blob/main/NOTICE).

---

<a id="中文"></a>
## 中文

> [!WARNING]
> **Beta 版。** `shape` / `same_shape` 应该是稳定的。`normalize` 的输出目前是**按字形**编码的，**不是**按读音
> 书写的标准名义字符序列，后续版本有可能修改。例如 ᠮᠣᠩᠭᠣᠯ（`MA+O+ANG+GA+O+LA`）规范化后是 ᠮᠣᠩᠨ᠋ᠨᠣᠯ
> （`MA+O+ANG+NA+FVS1+NA+O+LA`）。如果要持久化规范化后的
> key，请同时保存 `canonical_version()`，版本变化时重建。

**mongol-norm** 判断两段传统蒙古文（回鹘式，Hudum）是否外形相同，并把它们映射成同一个 key。零依赖
Rust crate，另有 Python 包和命令行工具。

### 为什么

同一个看起来一样的蒙古文词，可以用很多种 Unicode 序列输入——A/E 在词中、词尾同形，O/U、OE/UE 共享字形，
FVS 和连接符（nirugu、ZWJ）也能用不同方式得到同一个字形。结果是：搜索找不到、去重失败、同一个词索引 key
不同。

![五种 sain 编码全部规范化为同一个形式](https://raw.githubusercontent.com/Satsrag/mongol-norm/main/assets/sain-variants.png)

mongol-norm 按 [UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) 的整形规则（不需要字体）
算出字体会画出的**书写单元**：

- `shape(word)` —— 书写单元序列，即可见词形的指纹
- `same_shape(a, b)` —— 两种编码渲染结果是否相同
- `normalize(word)` / `normalize_text(text)` —— 同一个 shape 只输出一个 Unicode 字符串，且前缀稳定：
  词开头部分的 key 就是整个词 key 的开头，最多只差最后一个字母

用途：搜索和索引、去重、语料词频统计、拼写检查前的查词、清洗 OCR 或输入法输出。

### 安装

**Rust**（MSRV 1.82，零依赖，可编译到 `wasm32-unknown-unknown`）：

```bash
cargo add mongol-norm
```

或写进 `Cargo.toml`：

```toml
[dependencies]
mongol-norm = "0.2.3"
```

**Python**（CPython ≥ 3.9，无运行时依赖，不需要 Rust 工具链）：

```bash
pip install mongol-norm
```

预编译 wheel 覆盖 Linux x86_64 / aarch64（glibc 与 musl）、macOS x86_64 / Apple silicon、Windows x64。
其他平台 pip 会从源码包构建，需要 Rust ≥ 1.83。

**命令行**——下面任意一种都会装上 `mongol-norm` 命令：

```bash
cargo install mongol-norm
pip install mongol-norm
```

### 用法

Rust：

```rust
use mongol_norm::{Error, Locale, Shaper};

fn main() -> Result<(), Error> {
    let shaper = Shaper::new(Locale::Mng);

    assert_eq!(shaper.shape_str("ᠰᠠᠢᠨ")?, "S+A+I+I+A");               // 整形
    assert!(shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")?);                      // 是否同形
    assert_eq!(shaper.normalize("ᠰᠡᠢᠨ")?, shaper.normalize("ᠰᠠᠶ᠋ᠢᠨ")?); // 规范化

    // 自由文本：只规范化蒙古文词，其余原样保留
    let text = shaper.normalize_text("Hello ᠰᠡᠢᠨ world")?;
    assert!(text.starts_with("Hello ") && text.ends_with(" world"));
    Ok(())
}
```

Python：

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

命令行：

```bash
mongol-norm shape 'ᠰᠠᠢᠨ'                  # → S+A+I+I+A
mongol-norm same 'ᠰᠠᠢᠨ' 'ᠰᠡᠢᠨ'            # 同形退出码 0，否则 1
mongol-norm normalize 'ᠰᠡᠢᠨ'              # 单个词
mongol-norm normalize-text -i in.txt -o out.txt
mongol-norm normalize --batch -i words.txt -o keys.txt   # 一行一个词
```

完整 API 见 [docs.rs](https://docs.rs/mongol-norm) 和
[Python 文档](https://github.com/Satsrag/mongol-norm/blob/main/python/README.pypi.md)；`mongol-norm --help`
列出全部参数。

### 说明

- 规范化目前只支持 **MNG（回鹘式）**。托忒文、锡伯文、满文（`TOD`/`SIB`/`MCH`）只能整形。
- 内置表无法编码的 shape 会报错而不是乱猜；`normalize_allow_fallback`（Python：`strict=False`）则原样返回。
- 整形 100% 通过 mongfontbuilder 的 `core-hud` 与 GB/T 25914-2023 `eac-hud` 测试集；规范化对全部语料词
  满足 `shape(normalize(x)) == shape(x)`。
- 本项目用 [Claude Code](https://claude.ai/code) 编写，测试与核心代码经人工审核。遇到问题请提
  [issue](https://github.com/Satsrag/mongol-norm/issues)。

更多：[实现原理](https://github.com/Satsrag/mongol-norm/blob/main/docs/internals.md) ·
[数据格式](https://github.com/Satsrag/mongol-norm/blob/main/docs/data-format.md) ·
[开发与测试](https://github.com/Satsrag/mongol-norm/blob/main/docs/development.md) ·
[发布流程](https://github.com/Satsrag/mongol-norm/blob/main/docs/releasing.md)

### 许可

MIT，见 [LICENSE](https://github.com/Satsrag/mongol-norm/blob/main/LICENSE)。整形规则与数据源自
[UTN #57](https://www.unicode.org/notes/tn57/tn57-4.html) 和 Kushim Jiang 的
[mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder)（MIT）；测试集来自 mongfontbuilder 与
GB/T 25914-2023。声明见 [NOTICE](https://github.com/Satsrag/mongol-norm/blob/main/NOTICE)。
