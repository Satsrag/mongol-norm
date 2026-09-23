# Development

## Repository layout

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

## Running the tests

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
