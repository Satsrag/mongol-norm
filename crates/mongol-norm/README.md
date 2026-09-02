# mongol-norm (Rust)

[![crates.io](https://img.shields.io/crates/v/mongol-norm.svg)](https://crates.io/crates/mongol-norm)
[![docs.rs](https://img.shields.io/docsrs/mongol-norm)](https://docs.rs/mongol-norm)

[English](#english) | [中文](#中文)

<a id="english"></a>
## English

Shape-aware normalizer for Traditional Mongolian (Hudum) script — the pure-Rust twin of the
[`mongol-norm`](https://pypi.org/project/mongol-norm/) Python package, living in the same
repository. It implements the UTN #57 v4 shaping pipeline and the FVS-pinned canonical
normalizer with **zero dependencies**, and produces byte-identical results to the Python package on
every value-producing operation (shaping, normalization, the written-unit APIs, the CLI results;
only some error-message spellings and `shape_detailed`'s alias of structural tokens differ)
of the same version (verified against the shared corpus and golden fixtures in CI).

```toml
[dependencies]
mongol-norm = "0.0.4"
```

(The crate is developed in this repository; a git dependency works too.)

```rust
use mongol_norm::{Error, Locale, PositionedWrittenUnit, Shaper, UnitPosition, WrittenUnit};

fn main() -> Result<(), Error> {
    let shaper = Shaper::new(Locale::Mng);

    // Shape: written-unit sequence
    let shape = shaper.shape("ᠰᠠᠢᠨ")?;
    assert_eq!(shape.len(), 5); // [S, A, I, I, A]
    assert_eq!(shaper.shape_str("ᠰᠠᠢᠨ")?, "S+A+I+I+A");

    // Compare: visually identical?
    assert!(shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")?);

    // Normalize a word to its canonical, FVS-pinned encoding (strict: an uncovered shape is an
    // error — `normalize_allow_fallback` returns such input unchanged instead)
    let canonical = shaper.normalize("ᠰᠡᠢᠨ")?;
    assert_eq!(shaper.normalize("ᠰᠠᠶ᠋ᠢᠨ")?, canonical);

    // Free-form text: Mongolian words normalized, everything else preserved
    let text = shaper.normalize_text("Hello ᠰᠡᠢᠨ world")?;
    assert!(text.starts_with("Hello ") && text.ends_with(" world"));

    // Written units and authoritative HUD positions
    let units = [WrittenUnit::B, WrittenUnit::Aa];
    assert_eq!(shaper.normalize_written_units(&units)?, "ᠪᠠ᠋");
    let records = [
        PositionedWrittenUnit::new(WrittenUnit::B, UnitPosition::Init),
        PositionedWrittenUnit::new(WrittenUnit::Aa, UnitPosition::Fina),
    ];
    assert_eq!(shaper.normalize_positioned_written_units(&records)?, "ᠪᠠ᠋");
    assert_eq!(shaper.canonical_version(), Some("mng-canonical/1"));
    Ok(())
}
```

### Command line

The crate ships a `mongol-norm` binary with the same subcommands as the Python CLI. It turns a
word into its rendered written-unit sequence (`shape`), rewrites any encoding of a word into the
one canonical, FVS-pinned Unicode form (`normalize`), does the same for every Mongolian word
inside free-form text (`normalize-text`), encodes pre-shaped written units
(`normalize-written-units`), and tells whether two encodings render identically (`same`):

```sh
cargo install mongol-norm

mongol-norm shape 'ᠰᠠᠢᠨ'                    # → S+A+I+I+A (the rendered written units)
mongol-norm normalize 'ᠰᠡᠢᠨ'                # one word → its canonical encoding
mongol-norm same 'ᠰᠠᠢᠨ' 'ᠰᠡᠢᠨ'             # exit 0: the two encodings render identically
mongol-norm normalize-text 'Hello ᠰᠡᠢᠨ'     # free text: Mongolian runs normalized, rest kept
mongol-norm normalize-written-units 'B+Aa'   # encode pre-shaped written units
echo 'ᠰᠡᠢᠨ' | mongol-norm normalize -       # stdin; files via -i FILE / -o FILE
mongol-norm normalize --batch -i words.txt -o canonical.txt   # one word per line
```

`--locale MNG|TOD|SIB|MCH` selects the script (default `MNG`; only `MNG` normalizes),
`--allow-fallback` keeps an uncovered word instead of failing, errors exit with code 2
(`same` exits 0/1 for same/different), and `mongol-norm --help` lists every flag.

- Only `MNG` (Hudum) has shaping rules and a normalize table; `TOD` / `SIB` / `MCH` shape
  default and FVS forms only, exactly like the Python package.
- The data tables in `src/generated/` are generated from `mongol_norm/data/*.json` by
  `scripts/gen_rust_tables.py` — never edit them by hand.
- The corpus and golden tests read the repository's shared `tests/` directory, so `cargo test`
  needs a repository checkout (the published crate does not include them).
- Design and fidelity contract:
  [`docs/superpowers/specs/2026-09-01-rust-core-design.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/superpowers/specs/2026-09-01-rust-core-design.md).

<a id="中文"></a>
## 中文

传统蒙古文（回鹘式）形态感知规范化器的 **纯 Rust 实现**，与同仓库的 Python 包
[`mongol-norm`](https://pypi.org/project/mongol-norm/) 是一对双实现：实现完整的 UTN #57 v4
整形流程和 FVS 钉死的 canonical 规范化，**零依赖**，所有产出值的操作与同版本 Python 包逐字节相同（个别错误信息拼写除外；CI 用共享的
语料和 golden 固件验证）。

```toml
[dependencies]
mongol-norm = "0.0.4"
```

（crate 在本仓库中开发，git 依赖亦可。）

- 只有 `MNG`（回鹘式蒙古文）有整形规则和规范化表；`TOD` / `SIB` / `MCH` 与 Python 一样只整形默认形和 FVS 形。
- `src/generated/` 下的数据表由 `scripts/gen_rust_tables.py` 从 `mongol_norm/data/*.json` 生成，请勿手改。
- 语料与 golden 测试读取仓库共享的 `tests/` 目录，因此 `cargo test` 需要仓库 checkout（发布的 crate 不包含它们）。
- 命令行工具 `mongol-norm` 与 Python 版子命令相同：`shape` 输出词的书写单元序列，`normalize`
  把同一个词的任意编码统一成唯一的 canonical（FVS 钉死）形式，`normalize-text` 只规范化自由文本中的
  蒙古文词，`normalize-written-units` 编码已 shape 的书写单元，`same` 判断两种编码是否同形：

```sh
cargo install mongol-norm

mongol-norm shape 'ᠰᠠᠢᠨ'                    # → S+A+I+I+A（渲染出的书写单元序列）
mongol-norm normalize 'ᠰᠡᠢᠨ'                # 单词 → canonical 编码
mongol-norm same 'ᠰᠠᠢᠨ' 'ᠰᠡᠢᠨ'             # 两种编码同形则退出码 0
mongol-norm normalize-text 'Hello ᠰᠡᠢᠨ'     # 自由文本：只规范化蒙古文词，其余原样
echo 'ᠰᠡᠢᠨ' | mongol-norm normalize -       # stdin；文件用 -i / -o，逐行批量用 --batch
```

  （`--locale` 选文种，`--allow-fallback` 原样保留未覆盖的词；出错退出码 2，`same` 用 0/1。）

- 设计与保真约定见
  [`docs/superpowers/specs/2026-09-01-rust-core-design.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/superpowers/specs/2026-09-01-rust-core-design.md)。
