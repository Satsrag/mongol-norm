# Python bindings design — the PyPI package becomes a wrapper over the Rust core

**Status:** approved 2026-09-02 (design "Rust core + PyO3 bindings", version 0.1.0);
Task 1 (binding crate, packaging, wrapper) implemented and verified in commit
`5f39721`. Plan: `docs/superpowers/plans/2026-09-02-python-bindings.md`.

Supersedes the "dual implementation" arrangement of
`2026-09-01-rust-core-design.md`: the pure-Python runtime is removed; the Rust crate
is the only engine.

## Goal

User directive (verbatim): "这里 python 代码去掉吧 然后 pypi里直接 基于rust 代码封装
发布。但是rust库生成和其他所需的python代码要留下哦" and "只要 像原生python一样能用就行
全平台python", version "0.1.0".

1. Delete the pure-Python shaping/normalizing implementation.
2. Publish `mongol-norm` on PyPI as a package whose engine is the Rust crate
   `crates/mongol-norm`, keeping the public Python API intact.
3. Keep every Python script the project still needs: the Rust-table generator, the
   upstream preprocessing, the normalize-table and golden generators, and the tests.
4. `pip install mongol-norm` must work like any native Python package on every
   platform Python runs on — prebuilt binary wheels for Linux, macOS and Windows,
   Python 3.9+, no Rust toolchain on the user's machine.

## Non-goals

- Changing the engine's behaviour: the Rust crate is byte-identical to the deleted
  Python implementation (verified in the previous phase and again in this one).
- Supporting Python < 3.9 (PyO3 0.29 floor) or Windows on ARM / other niche targets
  in the first release; the sdist still builds there with a Rust toolchain.
- Publishing the binding crate to crates.io (`publish = false`); only the core crate
  is a crates.io crate.

## Architecture

```
Cargo.toml                    workspace: crates/mongol-norm, crates/mongol-norm-py
                              [workspace.package].version = 0.1.0  (single source of truth)
crates/mongol-norm/           the engine (zero deps, MSRV 1.82, crates.io "mongol-norm")
crates/mongol-norm-py/        pyo3 0.29 (abi3-py39) cdylib → mongol_norm/_native.abi3.so
                              rust-version 1.83, publish = false, feature `testing`
mongol_norm/__init__.py       MongolianShaper, NormalizationFallbackError, __version__
mongol_norm/_api.py           the public API: wraps _native, keeps pre-0.1 fidelity
mongol_norm/shaper.py         compat shim (re-exports + `main` console-script entry)
mongol_norm/_data.py          JSON loaders (tooling only; the runtime never reads JSON)
mongol_norm/data/*.json       shipped in the wheel for tooling (docs/data-format.md)
pyproject.toml                maturin backend, dynamic version, requires-python >= 3.9
scripts/gen_rust_tables.py    JSON → crates/mongol-norm/src/generated (unchanged)
scripts/preprocess.py         upstream mongfontbuilder → JSON (unchanged)
scripts/gen_normalize_table.py  battery over the bindings' shape_detailed (ported)
scripts/gen_compat_goldens.py   goldens from the bindings' trace()/normalize (ported)
```

### Binding crate (`crates/mongol-norm-py`)

- `#[pyclass(name = "Shaper", frozen)]` wrapping `mongol_norm::Shaper`; frozen because
  the shaper is immutable and `Send + Sync`, so it needs no GIL-side mutability.
- Thin methods returning plain tuples/lists/strings; `shape_detailed` returns
  `(cp, alias, position, fvs_index, condition, written)` tuples and `trace` returns
  `(positions, transitions, final_conditions, written_by_token, shape)`; the Python
  layer builds the historical dicts.
- Error mapping (`to_py`): `NormalizationFallback` → `_native.FallbackError(text,
  written_units)` (a `ValueError` subclass the wrapper converts to
  `NormalizationFallbackError`); `NormalizeUnsupported` → `RuntimeError` with the
  pre-0.1 wording including "; generate it with scripts/gen_normalize_table.py";
  everything else → `ValueError` with the crate's Display string (already pinned to
  the Python wording).
- `known_written_units()` / `positioned_written_units()` expose the normalize table's
  inventories so the Python layer can validate `normalize_written_units` /
  `normalize_positioned_written_units` input with the original Python messages and
  index semantics (`repr()` formatting stays in Python).
- `cli_main(args) -> int` runs the crate's CLI (`cli::run_args`) on the process's real
  stdio; `version()`; under `--features testing` also
  `_shaper_with_empty_normalize_table(locale)` for the fallback tests.
- Core-crate additions: feature `testing` (exposes `Shaper::with_empty_normalize_table`),
  `Shaper::known_written_units`, `Shaper::positioned_written_units`, `cli::run_args`.

### Python layer (`mongol_norm/_api.py`)

Same class/exception names, signatures, defaults, return shapes and error messages as
the deleted implementation (docstrings kept, bilingual). Additions: `trace(text)`
(phase-trace golden format), `rule_names()`, `parse_written_units(text)`; the
`MongolianShaper._wrap(native)` classmethod for test hooks. `canonical_version` is a
property that raises `RuntimeError` for locales without a table, as before.

`main()` flushes Python's stdio and `sys.exit(_native.cli_main(sys.argv[1:]))`; the
`mongol-norm` console script keeps pointing at `mongol_norm.shaper:main`.

### Behavioural differences from 0.0.4 (documented, accepted)

- `MongolianShaper("XX")` raises `ValueError("unknown locale 'XX'")` instead of
  `FileNotFoundError`.
- The CLI is the Rust CLI; its intentional differences from the argparse CLI are
  listed at the top of `crates/mongol-norm/src/cli.rs` (help text layout, error
  prefixes).
- `mongol_norm.rules`, `MongolianShaper.tokenize/assign_positions` and the other
  private internals no longer exist; `mongol_norm.shaper` only re-exports.
- Python 3.7/3.8 are no longer supported (`requires-python >= 3.9`).

### Versioning

One literal: `[workspace.package].version` in the root `Cargo.toml`. maturin reads it
through the binding crate (`version.workspace = true`); `mongol_norm.__version__` is
`_native.version()` at import time; `pyproject.toml` declares `dynamic = ["version"]`.
`tests/test_rust_twin.py` asserts `mongol_norm.__version__` equals the workspace
version; the release workflows check the tag against the same literal.

### Packaging and distribution

- Wheels: `cp39-abi3` (one wheel per platform for all Python ≥ 3.9) for
  Linux x86_64 + aarch64 (manylinux2014 and musllinux_1_2), macOS x86_64 + arm64,
  Windows x64; plus an sdist that carries the workspace, scripts, tests and docs so it
  is buildable and self-testable with a Rust ≥ 1.83 toolchain.
- The wheel ships `mongol_norm/data/*.json` (tooling data) and the `LICENSE`/`NOTICE`.
- `publish.yml`: on GitHub release, build the matrix with `PyO3/maturin-action`,
  smoke-test each natively runnable wheel (`pip install` from `dist/`, import, shape
  a word, run the console script), then publish all files through the existing PyPI
  trusted publisher (environment `pypi`, `pypa/gh-action-pypi-publish`). The crate's
  `publish-crate.yml` is unchanged; one release tag publishes both.
- `test.yml`: Python matrix 3.9–3.14 builds the extension with `maturin develop
  --features testing` before running the unittest suite; the Rust job now builds and
  clippies the whole workspace (Python present on the runner), keeps MSRV 1.82 /
  wasm32 / `cargo package` for the core crate only.

### Tests and generators

- The Python suite tests only the public API plus `mongol_norm._data` and the
  `testing` hook. Tests that exercised deleted internals (tokenizer objects, private
  encoders, synthetic vocabularies, swapped normalize tables) are dropped in favour
  of the Rust unit tests that already cover them; each dropped test is named in the
  plan with its Rust counterpart.
- Goldens (`tests/golden/*`) and `mongol_norm/data/MNG.normalize.json` are regenerated
  by the ported scripts and must remain byte-identical (`--check` in CI); the upstream
  HUD TSVs remain the independent oracle.
- New `tests/test_bindings.py` pins the wrapper layer: dict formats, error mapping and
  messages, version lockstep, shim re-exports, console-script behaviour.

## Verification

- Differential against the deleted Python implementation (from git history) over
  7511 inputs × 12 operations (shape, shape_str, shape_detailed, trace, normalize
  strict/lenient, normalize_text strict/lenient, normalize_written_units,
  normalize_positioned_written_units, parse_written_units, canonical_version; error
  type + message + attributes compared): zero mismatches (commit `5f39721`).
- Full Python suite green on 3.9–3.14; full Rust suite, clippy, fmt, rustdoc green.
- Fresh generated artifacts: `gen_rust_tables.py --check`, `gen_compat_goldens.py
  --check`, `test_normalize_table` compute-vs-bundled.
- A release-mode wheel installs into a fresh venv and `mongol-norm shape ᠰᠠᠢᠨ` works.
