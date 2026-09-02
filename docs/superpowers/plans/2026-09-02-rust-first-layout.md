# Rust-first repository layout

User directive (2026-09-02, after 0.1.1 shipped): "目录结构改一下 rust主 python次 readme也是
rust主" — the Rust crate is the repository's main project and lives at the root; everything
Python moves under `python/`; the README leads with Rust.

> **Amendments (execution log)** — kept current while the plan runs.

## Target layout

```
mongol-norm/
├── Cargo.toml            [package] mongol-norm (the crate, root package) + [workspace]
│                         members = ["python"]; [workspace.package] keeps the single
│                         version literal (the root package inherits it too)
├── Cargo.lock
├── src/                  crate sources (was crates/mongol-norm/src), incl. src/generated/
├── tests/                Rust integration tests (was crates/mongol-norm/tests) …
│   ├── common/           … their helper module
│   ├── data/             shared fixtures: core-hud.tsv, eac-hud.tsv (unchanged files)
│   └── golden/           shared fixtures: mng-canonical-v1.jsonl, mng-phase-trace-v1.json
├── README.md             Rust-first (crates.io README = this file); Python section short
├── LICENSE, NOTICE       the crate's own files (no copies anywhere any more)
├── assets/, docs/, openwiki/, .github/   unchanged locations
└── python/               everything Python
    ├── Cargo.toml        binding crate mongol-norm-py (was crates/mongol-norm-py):
    │                     mongol-norm = { path = ".." }
    ├── build.rs, src/lib.rs
    ├── pyproject.toml    manifest-path = "Cargo.toml", module-name = "mongol_norm._native",
    │                     readme = "README.md"; sdist ships LICENSE/NOTICE (from ..? see
    │                     below), no tests/scripts/docs any more
    ├── README.md         PyPI-facing README (Python usage), EN + 中文
    ├── mongol_norm/      the package (+ data/*.json — stays here: produced by the Python
    │                     preprocessing tool, shipped in the wheel for tooling)
    ├── scripts/          gen_rust_tables.py, gen_normalize_table.py, gen_compat_goldens.py,
    │                     preprocess.py, check_dist_metadata.py
    └── tests/            the Python suite (+ _support.py); fixtures are read from
                          ../../tests/{data,golden}
```

Decisions:

- The JSON data stays inside the Python package (`python/mongol_norm/data/`): it is produced
  by `python/scripts/preprocess.py` (needs mongfontbuilder), consumed by
  `python/scripts/gen_rust_tables.py`, and shipped in the wheel for tooling — moving it to
  the root would break the wheel's `mongol_norm._data` loaders for no gain.
- Shared test fixtures live once, under the crate's `tests/` (Rust owns them); the Python
  tests reach them with a repo-relative path. Consequently the sdist no longer ships the
  test-suite, scripts, or docs (they need the checkout): the sdist is what pip needs to
  build the package. `LICENSE`/`NOTICE`: maturin needs them inside the sdist root — copy
  them into `python/` only if maturin cannot include `../LICENSE` (try `include` with the
  parent path first; if it is rejected, add `python/LICENSE` + `python/NOTICE` as copies
  and a test that they equal the root files).
- `cargo publish` / `cargo package` stay `-p mongol-norm`; the crate's `include` list keeps
  the package minimal (`src/**`, `Cargo.toml`, `README.md`, `LICENSE`, `NOTICE`).
- Commands move with the files: Python work runs from `python/` (`cd python && maturin
  develop --locked --features testing && python -m unittest discover -s tests -p
  'test_*.py'`); the generator scripts are invoked as `python/scripts/<name>.py` from the
  repo root (they compute the repo root from their own location).

## Touchpoints (every one must be updated; grep for the old paths when done)

- `Cargo.toml` (root): becomes the crate manifest + workspace; remove `crates/mongol-norm/Cargo.toml`.
  Delete `crates/`. `src/lib.rs` `include_str!("../README.md")` still resolves.
- Rust integration tests: `tests/common` fixture paths (`../../tests/data` →
  `tests/data` relative to `CARGO_MANIFEST_DIR`), `tests/round_trip.rs:324`.
- `python/scripts/gen_rust_tables.py`: input `python/mongol_norm/data`, output
  `src/generated`, header comment `Source: python/mongol_norm/data/…`, docstrings, the
  `crates/mongol-norm/src/*.rs` cross-references in comments; regenerate the tables
  (headers change) and keep `--check` fresh.
- `python/scripts/gen_normalize_table.py`, `gen_compat_goldens.py`, `preprocess.py`:
  repo-root / fixture / data / tests-dir paths.
- Python tests: `_REPO_ROOT`/`ROOT` computations, fixture paths, `test_rust_twin.py`
  (Cargo.toml at root, crate at root, `python/Cargo.toml` inherits the version, the
  LICENSE/NOTICE copy test becomes "crate include lists LICENSE/NOTICE" or a python/
  copy check), `test_golden_generation.py` script path, `_support.py` cwd.
- `python/pyproject.toml`: `manifest-path`, `readme`, `include` (drop tests/scripts/docs).
- Workflows: `test.yml` (python job `working-directory: python` for maturin/unittest;
  `--check` scripts as `python/scripts/…`; Rust job unchanged commands), `publish.yml`
  (maturin-action `working-directory: python`, sdist smoke `dist/` under `python/dist`,
  `scripts/check_dist_metadata.py` path, artifact paths), `publish-crate.yml` (comments).
- Docs: `README.md` (rewrite, Rust-first, EN + 中文), new `python/README.md` (PyPI),
  `docs/data-format.md`, `docs/releasing.md` paths and commands; a status note in
  `docs/superpowers/specs/2026-09-02-python-bindings-design.md` pointing here.
- `.gitignore` (`*.so` already; `python/dist/` covered by `dist/`? make it `**/dist/`).

## Verification

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features --locked
-- -D warnings`, `cargo test --workspace --locked`, `cargo package -p mongol-norm --locked`
(inspect the .crate file list), `cargo +1.82.0 test -p mongol-norm --locked`; from
`python/`: `maturin develop --locked --features testing`, the suite (252 tests, 0 skipped),
`maturin sdist` + `pip install` of the tarball in a fresh venv (with
`CARGO_PROFILE_RELEASE_STRIP=none` on the dev Mac) + `python/scripts/check_dist_metadata.py
--require LICENSE --require NOTICE`; the three `--check` scripts; `git status` shows fixtures
unchanged (renames only); CI green on the branch plus a build-only `publish.yml` dispatch.
