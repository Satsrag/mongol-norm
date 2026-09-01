# mongol-norm (Rust)

[English](#english) | [中文](#中文)

<a id="english"></a>
## English

Shape-aware normalizer for Traditional Mongolian (Hudum) script — the pure-Rust twin of the
[`mongol-norm`](https://pypi.org/project/mongol-norm/) Python package, living in the same
repository. It implements the UTN #57 v4 shaping pipeline and the FVS-pinned canonical
normalizer with **zero dependencies**, and produces byte-identical output to the Python package
of the same version (verified against the shared corpus and golden fixtures in CI).

```toml
[dependencies]
mongol-norm = "0.0.4"
```

```rust
use mongol_norm::{Locale, PositionedWrittenUnit, Shaper, UnitPosition, WrittenUnit};

let shaper = Shaper::new(Locale::Mng);

// Shape: written-unit sequence
let shape = shaper.shape("ᠰᠠᠢᠨ")?;            // [S, A, I, I, A]
assert_eq!(shaper.shape_str("ᠰᠠᠢᠨ")?, "S+A+I+I+A");

// Compare: visually identical?
assert!(shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")?);

// Normalize a word to its canonical, FVS-pinned encoding (strict: an uncovered shape is an error)
let canonical = shaper.normalize("ᠰᠡᠢᠨ")?;
assert_eq!(shaper.normalize("ᠰᠠᠶ᠋ᠢᠨ")?, canonical);

// Free-form text: Mongolian words normalized, everything else preserved
let text = shaper.normalize_text("Hello ᠰᠡᠢᠨ world")?;

// Written units and authoritative HUD positions
assert_eq!(shaper.normalize_written_units(&[WrittenUnit::B, WrittenUnit::Aa])?, "ᠪᠠ᠋");
let records = [
    PositionedWrittenUnit::new(WrittenUnit::B, UnitPosition::Init),
    PositionedWrittenUnit::new(WrittenUnit::Aa, UnitPosition::Fina),
];
assert_eq!(shaper.normalize_positioned_written_units(&records)?, "ᠪᠠ᠋");
assert_eq!(shaper.canonical_version(), Some("mng-canonical/1"));
# Ok::<(), mongol_norm::Error>(())
```

The `mongol-norm` binary offers the same subcommands as the Python CLI (`shape`, `normalize`,
`normalize-text`, `normalize-written-units`, `same`): `cargo install mongol-norm`.

- Only `MNG` (Hudum) has shaping rules and a normalize table; `TOD` / `SIB` / `MCH` shape
  default and FVS forms only, exactly like the Python package.
- The data tables in `src/generated/` are generated from `mongol_norm/data/*.json` by
  `scripts/gen_rust_tables.py` — never edit them by hand.
- The corpus and golden tests read the repository's shared `tests/` directory, so `cargo test`
  needs a repository checkout (the published crate does not include them).
- Design and fidelity contract: `docs/superpowers/specs/2026-09-01-rust-core-design.md`.

<a id="中文"></a>
## 中文

传统蒙古文（回鹘式）形态感知规范化器的 **纯 Rust 实现**，与同仓库的 Python 包
[`mongol-norm`](https://pypi.org/project/mongol-norm/) 是一对双实现：实现完整的 UTN #57 v4
整形流程和 FVS 钉死的 canonical 规范化，**零依赖**，输出与同版本 Python 包逐字节相同（CI 用共享的
语料和 golden 固件验证）。

- 只有 `MNG`（回鹘式蒙古文）有整形规则和规范化表；`TOD` / `SIB` / `MCH` 与 Python 一样只整形默认形和 FVS 形。
- `src/generated/` 下的数据表由 `scripts/gen_rust_tables.py` 从 `mongol_norm/data/*.json` 生成，请勿手改。
- 语料与 golden 测试读取仓库共享的 `tests/` 目录，因此 `cargo test` 需要仓库 checkout（发布的 crate 不包含它们）。
- 命令行工具 `mongol-norm` 与 Python 版子命令相同：`cargo install mongol-norm`。
