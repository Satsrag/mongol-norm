# Development · 开发与测试

[English](#english) | [中文](#中文)

---

<a id="english"></a>
## English

### Repository layout

```text
mongol-norm/
├── Cargo.toml          the crate (root package) + [workspace] members = ["python"];
│                       [workspace.package].version is the only version literal
├── Cargo.lock
├── src/                the engine — zero dependencies, MSRV 1.82, wasm32-clean
│   ├── token.rs        tokenization, structural positions
│   ├── rules.rs        the five shaping phases, one function per rule
│   ├── shaper.rs       variant resolution: shape / same_shape / shape_detailed / trace
│   ├── normalize.rs    the canonical normalizer: entry points, table indexes
│   ├── encoder.rs      the online (prefix-committed) encoder behind it
│   ├── written_units.rs  the written-unit and positioned-written-unit encoders
│   ├── cli.rs          the mongol-norm command (src/bin/mongol-norm.rs is a shim)
│   └── generated/      tables generated from the JSON — never hand-edited
├── examples/gen_normalize_table/  the normalize-table generator (writes
│                       python/mongol_norm/data/MNG.normalize.json; see docs/internals.md)
├── tests/              the crate's integration tests and the shared fixtures
│   ├── data/           core-hud.tsv, eac-hud.tsv — vendored from mongfontbuilder
│   └── golden/         mng-canonical-v1.jsonl, mng-phase-trace-v1.json
├── README.md           the GitHub landing page, the crates.io README, and a doctest
├── LICENSE  NOTICE     the crate's own licence files
├── assets/  docs/  .github/workflows/
└── python/             everything Python
    ├── Cargo.toml      the PyO3 binding crate mongol-norm-py (Rust 1.83; not on crates.io)
    ├── build.rs  src/lib.rs
    ├── pyproject.toml  maturin backend; the version comes from ../Cargo.toml
    ├── README.pypi.md  the package's PyPI long description
    ├── LICENSE  NOTICE byte-identical copies of the root files (maturin cannot reach `..`)
    ├── mongol_norm/    the package: __init__.py, _api.py (the public API),
    │   │               shaper.py (the 0.0.x compat shim), _data.py
    │   └── data/       the shaping + normalize JSON: input of the table generator,
    │                   shipped in the wheel for tooling
    ├── scripts/        gen_rust_tables.py, gen_normalize_table.py (runs the Rust example),
    │                   gen_compat_goldens.py, preprocess.py, check_dist_metadata.py
    └── tests/          the Python suite; reads the fixtures from ../../tests/{data,golden}
```

### Running the tests

Rust, from the repository root:

```bash
cargo test --workspace --locked   # 279 tests: unit + corpus + goldens + properties + CLI + fuzz + the README doctest
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo package -p mongol-norm      # what crates.io would receive
```

Python, from `python/` — the suite drives the compiled extension, so build it into a virtualenv
first (needs Rust ≥ 1.83; the `testing` feature exposes the hook the fallback tests use). Re-run
`maturin develop` after any Rust change:

```bash
cd python
python -m venv .venv && source .venv/bin/activate
pip install 'maturin>=1.15,<2'
maturin develop --locked --features testing        # builds mongol_norm/_native

python -m unittest discover -s tests -p 'test_*.py'   # 262 tests
python -m unittest tests.test_shaper -v               # shape / same_shape / normalize
python -m unittest tests.test_round_trip              # round-trip + canonicity + prefix-stability
python -m unittest tests.test_core_hud tests.test_eac_hud   # the upstream TSV suites
python -m unittest tests.test_rust_twin               # tables fresh, versions in lockstep
```

The generators are run from the repository root; each `--check` fails when the committed output
differs from what it would generate now (all three run in CI). `gen_rust_tables.py` needs only
Python, `gen_normalize_table.py` only `cargo` (it runs the Rust example
`examples/gen_normalize_table`, about 15 s in release mode), and `gen_compat_goldens.py` drives the
built extension:

```bash
python python/scripts/gen_rust_tables.py --check
python python/scripts/gen_normalize_table.py --check   # = cargo run --release --example gen_normalize_table -- --check
python python/scripts/gen_compat_goldens.py --check
```

Current totals: **279 Rust tests** (unit + property + 177 core-hud and 3512 eac-hud corpus rows,
1990 canonical and 15 phase-trace golden vectors, the online encoder's exhaustive short-input and
prefix checks, fuzz, and the README doctest) and **262 Python tests**, green on Rust stable / 1.82
(the core crate's MSRV; the binding crate needs 1.83) and CPython 3.9 – 3.14.

---

<a id="中文"></a>
## 中文

### 仓库结构

```text
mongol-norm/
├── Cargo.toml          crate（根 package）+ [workspace] members = ["python"]；
│                       [workspace.package].version 是唯一的版本号字面量
├── Cargo.lock
├── src/                引擎——零依赖、MSRV 1.82、可编译到 wasm32
│   ├── token.rs        分词、结构位置
│   ├── rules.rs        五个整形阶段，每条规则一个函数
│   ├── shaper.rs       变体解析：shape / same_shape / shape_detailed / trace
│   ├── normalize.rs    规范化器：入口函数、表索引
│   ├── encoder.rs      它背后的在线（逐字母提交的）编码器
│   ├── written_units.rs  书写单元与带位置书写单元的编码器
│   ├── cli.rs          mongol-norm 命令（src/bin/mongol-norm.rs 只是一层壳）
│   └── generated/      由 JSON 生成的表——不要手改
├── examples/gen_normalize_table/  规范化表生成器（写出
│                       python/mongol_norm/data/MNG.normalize.json；见 docs/internals.md）
├── tests/              crate 的集成测试和共享的测试数据
│   ├── data/           core-hud.tsv、eac-hud.tsv——从 mongfontbuilder 引入
│   └── golden/         mng-canonical-v1.jsonl、mng-phase-trace-v1.json
├── README.md           GitHub 首页、crates.io 的 README，同时是一个 doctest
├── LICENSE  NOTICE     crate 自己的许可文件
├── assets/  docs/  .github/workflows/
└── python/             所有 Python 相关内容
    ├── Cargo.toml      PyO3 绑定 crate mongol-norm-py（Rust 1.83；不发布到 crates.io）
    ├── build.rs  src/lib.rs
    ├── pyproject.toml  maturin 后端；版本号来自 ../Cargo.toml
    ├── README.pypi.md  PyPI 上的长描述
    ├── LICENSE  NOTICE 根目录同名文件的逐字节副本（maturin 访问不到 `..`）
    ├── mongol_norm/    包：__init__.py、_api.py（公开 API）、
    │   │               shaper.py（0.0.x 兼容层）、_data.py
    │   └── data/       整形与规范化 JSON：表生成器的输入，
    │                   随 wheel 发布供工具使用
    ├── scripts/        gen_rust_tables.py、gen_normalize_table.py（调用 Rust example）、
    │                   gen_compat_goldens.py、preprocess.py、check_dist_metadata.py
    └── tests/          Python 测试集；从 ../../tests/{data,golden} 读取测试数据
```

### 运行测试

Rust，在仓库根目录：

```bash
cargo test --workspace --locked   # 279 个测试：单元 + 语料 + golden + 性质测试 + CLI + fuzz + README doctest
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo package -p mongol-norm      # crates.io 会收到的内容
```

Python，在 `python/` 目录——测试驱动的是编译好的扩展，所以先在虚拟环境里构建它（需要 Rust ≥ 1.83；
`testing` feature 打开回退测试要用的钩子）。每次改了 Rust 代码都要重新 `maturin develop`：

```bash
cd python
python -m venv .venv && source .venv/bin/activate
pip install 'maturin>=1.15,<2'
maturin develop --locked --features testing        # 构建 mongol_norm/_native

python -m unittest discover -s tests -p 'test_*.py'   # 262 个测试
python -m unittest tests.test_shaper -v               # shape / same_shape / normalize
python -m unittest tests.test_round_trip              # 往返 + 规范性 + 前缀稳定
python -m unittest tests.test_core_hud tests.test_eac_hud   # 上游的 TSV 测试集
python -m unittest tests.test_rust_twin               # 表是最新的、版本号一致
```

生成器都在仓库根目录运行；每个 `--check` 在已提交的输出与现在生成的结果不一致时失败（三个都在 CI 中运行）。
`gen_rust_tables.py` 只需要 Python，`gen_normalize_table.py` 只需要 `cargo`（它运行 Rust example
`examples/gen_normalize_table`，release 模式约 15 秒），`gen_compat_goldens.py` 驱动构建好的扩展：

```bash
python python/scripts/gen_rust_tables.py --check
python python/scripts/gen_normalize_table.py --check   # = cargo run --release --example gen_normalize_table -- --check
python python/scripts/gen_compat_goldens.py --check
```

当前总计：**279 个 Rust 测试**（单元 + 性质测试 + 177 行 core-hud 和 3512 行 eac-hud 语料、1990 个规范
golden 向量和 15 个阶段追踪 golden 向量、在线编码器的短输入穷举和前缀检查、fuzz，以及 README doctest）和
**262 个 Python 测试**，在 Rust stable / 1.82（核心 crate 的 MSRV；绑定 crate 需要 1.83）和
CPython 3.9 – 3.14 上全部通过。
