# Python bindings implementation plan

Spec: `docs/superpowers/specs/2026-09-02-python-bindings-design.md`. Branch
`feat/python-bindings` (from `main` at 51c3736). Version 0.1.0.

> **Amendments (execution log)** — kept current while the plan runs.
>
> - Task 1 was prototyped directly (not via a subagent) because the PyO3/maturin
>   integration needed compiler-driven iteration; landed as commit `5f39721` with the
>   differential described in the spec.
> - Local release wheels do not load on the dev Mac (macOS 27.0 beta, CLT/SDK 27.0,
>   ld-27037.1): `dlopen` fails with "mis-aligned LINKEDIT string pool". Root cause
>   established from dyld sources and PyPI samples: the beta linker does not pad the
>   symbol-string pool to 8 bytes (`stroff = indirectsymoff + 4·nindirectsyms`, so
>   any odd indirect-symbol count misaligns it), and the macOS 27 dyld enforces the
>   new alignment only for binaries linked against SDK ≥ 27 (`Policy::_enforcementEpoch`
>   keys off `LC_BUILD_VERSION.sdk`). Wheels built against a stable SDK — e.g.
>   `rpds_py` 2026.6.3 (SDK 26.5, misaligned) — load fine on the same machine, so
>   CI-built wheels are unaffected. Consequence for Task 5: the release-wheel smoke
>   test on this Mac uses a CI-built wheel (publish.yml gains `workflow_dispatch` for
>   build-only runs), not a locally built one; the debug wheel (`maturin develop`) is
>   fine because its indirect-symbol count happens to be even.

## Working environment

- Python venv with maturin 1.15: `/private/tmp/claude-501/-Volumes-SN570-Users-satsrag-orca-mongol-norm/d059826c-ac63-451a-baf0-2feb60aadbca/scratchpad/venv`
  (`source …/venv/bin/activate`). The package is installed editable there; after any
  Rust change run `maturin develop --features testing` from the repo root.
- Python suite: `python -m unittest discover -s tests -p 'test_*.py'`.
- Rust: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`, `cargo test --workspace`.
- Do not commit; the orchestrator commits after review. Do not touch files owned by
  another task (listed per task).

## Task 1 — binding crate, packaging, Python wrapper  ✅ (commit 5f39721)

Owned files: `Cargo.toml`, `Cargo.lock`, `crates/mongol-norm-py/**`,
`crates/mongol-norm/{Cargo.toml,src/cli.rs,src/normalize.rs,src/shaper.rs,src/written_units.rs}`,
`pyproject.toml`, `MANIFEST.in` (deleted), `mongol_norm/{__init__,_api,shaper}.py`,
`mongol_norm/rules.py` (deleted), `.gitignore`.

Verification done: builds with `maturin develop`, differential vs the deleted
implementation (7511 inputs × 12 operations, zero mismatches), wheel contains
`_native.abi3.so` + `data/*.json` + licenses, sdist contains workspace + scripts +
tests + docs, `cargo fmt/clippy/test` clean.

## Task 2 — port the generator scripts and migrate the Python tests

Owned files: `scripts/gen_normalize_table.py`, `scripts/gen_compat_goldens.py`,
`tests/**` (all test files), and — only if a binding gap is found — `mongol_norm/_api.py`
/ `crates/mongol-norm-py/src/lib.rs` (report any such change explicitly).

Rules:

- Tests and scripts use only the public API (`MongolianShaper`: `shape`, `shape_str`,
  `same_shape`, `shape_detailed`, `trace`, `rule_names`, `parse_written_units`,
  `canonical_version`, `normalize`, `normalize_text`, `normalize_written_units`,
  `normalize_positioned_written_units`, attribute `locale`), `mongol_norm._data`
  (`load_rules`, `load_normalize_table`), `mongol_norm.__version__`, and — for the
  fallback tests only — `mongol_norm._native._shaper_with_empty_normalize_table(locale)`
  wrapped with `MongolianShaper._wrap(...)`, guarded by
  `unittest.skipUnless(hasattr(_native, "_shaper_with_empty_normalize_table"), …)`.
- `tests/golden/*` and `mongol_norm/data/MNG.normalize.json` must NOT change:
  `python scripts/gen_compat_goldens.py --check` and the `test_normalize_table`
  compute-vs-bundled test are the fidelity proof of the ported scripts. If either
  disagrees, the port is wrong (not the fixture).
- Drop a test only when it exercised deleted internals; name each dropped test and
  the Rust unit test (`crates/mongol-norm/src/*.rs`, `#[cfg(test)]`) that covers the
  same behaviour in your final report. Do not drop behaviour that is still
  observable through the public API — rewrite the test instead.

### 2a `scripts/gen_normalize_table.py`

It imported `MongolianShaper` internals. Port to: `shaper.shape_detailed(text)` for
the battery (unchanged call shape); everything else from the JSON via
`mongol_norm._data.load_rules(shaper.locale)`:

- `shaper._rules["letters"]` → `rules["letters"]` (positioned units, variants);
- `shaper._candidates_map` → build locally exactly as the deleted
  `MongolianShaper._build_candidates_map` did: every letter variant (all positions,
  including archaic/unrecommended) keyed `(position, tuple(written))` →
  `(cp, fvs)` candidates, plus bare `fvs=0` fallbacks for code points that only appear
  under a non-zero FVS. Read the deleted implementation with
  `git show 51c3736:mongol_norm/shaper.py` (search `_build_candidates_map`,
  `cp_to_alias`, `alias_to_cp`, `feminine_vowels`) and reproduce its data model;
- `shaper.cp_to_alias` / `shaper.alias_to_cp` / `shaper.feminine_vowels` →
  from `rules["letters"]` and `rules["categories"]["vowelFeminine"]`;
- constants `_VELAR_FEM_UNITS = {"G", "Gx"}`, `_MASC_TO_FEM_CP = {0x1820: 0x1821,
  0x1823: 0x1825, 0x1824: 0x1826}`, `CANONICAL_VERSION`, `FVS_INT_TO_CP`, `MVS_CP`,
  `NIRUGU_CP`, `ZWJ_CP` → defined in the script (they were imported from
  `mongol_norm.shaper`; copy the values from the deleted file).

Keep `compute_normalize_tables(shaper)` (tests call it) and the CLI (`--check`, output
path). Prove: `python scripts/gen_normalize_table.py --check` (or equivalent) reports
the bundled table is current.

### 2b `scripts/gen_compat_goldens.py`

- `phase_trace._trace(shaper, codepoints)` → `shaper.trace(text)`;
- `[rule.name for rule in shaper._shaping_rules]` → `shaper.rule_names()`;
- `shaper.canonical_version` unchanged; the helpers it imports from the test modules
  (`tests.test_round_trip._ALIAS_TO_CP`, `tests.test_canonical_golden._shape_groups`)
  must survive the test migration (keep those module-level names).

Prove: `python scripts/gen_compat_goldens.py --check` passes with the committed
fixtures.

### 2c Tests

- `tests/test_shaper.py`: position tests (`tokenize`/`assign_positions`) →
  `shape_detailed(...)[i]["position"]`; NNBSP tests → `shape_detailed` (`cp` is
  `"U+180E"`, `alias == "mvs"`); `test_nnbsp_chachlag_trigger` (`rules.run_rules`) →
  the `condition` field; the four fallback tests (normalize / normalize_text, strict
  and non-strict) → the `testing` hook via `MongolianShaper._wrap`.
- `tests/test_cli.py`: the strict-fallback tests monkeypatched
  `MongolianShaper._build_unit_enc`; the CLI now runs in Rust, so replace them with
  tests that need no fallback (e.g. `--allow-fallback` is accepted and leaves a normal
  word unchanged; strict `normalize`/`normalize-text` batch errors report `line N:`
  for a non-Mongolian character). The Rust `cli::tests` cover the fallback path.
  Check how the existing CLI tests invoke `main` — the Rust CLI writes to the real
  file descriptors, so in-process `redirect_stdout` capture does not work; use the
  subprocess pattern the other CLI tests already use.
- `tests/test_round_trip.py`: `_encode_chain_canonical` / `_compute_chain_canonical`
  → `normalize_written_units`; drop the `_chain_canon_cache` assertions;
  `particles_data["MNG"]` → `load_rules("MNG")["particles"]`; `_unit_enc`-derived
  vocabulary + `_parse_written_units(text, known)` → `shaper.parse_written_units`;
  `_STRUCTURAL_CHARS` → a local constant. Keep `_ALIAS_TO_CP`.
- `tests/test_written_units_api.py`: the synthetic-vocabulary
  `_parse_written_units("AAA", {"A", "AA"})` test → drop (Rust
  `written_units::tests` cover longest-match parsing); CLI tests keep the subprocess
  pattern.
- `tests/test_positioned_written_units_api.py`: `_build_unit_enc` /
  `_positioned_units` → `load_normalize_table("MNG")` (see `docs/data-format.md`
  for the positioned inventory field). All TypeError/ValueError tests stay (the
  validation lives in `_api.py`).
- `tests/test_phase_trace_golden.py`: `_trace` → `shaper.trace`; `_shaping_rules`
  → `shaper.rule_names()`; `_resolve_token_written` → `written_by_token` from the
  trace.
- `tests/test_normalize_table.py`: keep compute==bundled, positioned inventory and
  JSON-serialisable tests; the loader-based tests (reject wrong version, load a spec
  identically, normalize identical when loaded, loaded shaper handles particles)
  exercised the deleted Python loader → drop, naming the Rust `normalize::tests` /
  generated-table freshness test as coverage.
- `tests/test_rust_twin.py`: the version test compares the root `Cargo.toml`
  `[workspace.package].version` with `mongol_norm.__version__` (pyproject no longer
  holds a literal; also assert `pyproject.toml` has `dynamic = ["version"]`).
- New `tests/test_bindings.py`: `shape_detailed` exact dict for a word with an FVS,
  an MVS and an NNBSP (`{"cp","alias","position","fvs","condition","written"}`,
  `"+FVS1"`, `"U+180E"`, `"mvs"`); `trace` dict keys/shape; `NormalizationFallbackError`
  is a `ValueError` with `.text` and `.written_units` (tuple) via the hook;
  `RuntimeError` text for `MongolianShaper("TOD").canonical_version` /
  `normalize_written_units(["A"])` equals
  `"no bundled normalize table for locale 'TOD'; generate it with scripts/gen_normalize_table.py"`;
  `MongolianShaper("XX")` → `ValueError`; `mongol_norm.__version__ ==
  mongol_norm._native.version()`; `mongol_norm.shaper` re-exports `MongolianShaper`,
  `NormalizationFallbackError`, `main`; `mongol-norm --help` / `--version`? (check
  what the Rust CLI supports before asserting) via subprocess.

Done when: the whole suite passes in the venv, both `--check` scripts pass, and the
report lists dropped tests with their Rust counterparts.

## Task 3 — CI: test.yml and publish.yml

Owned files: `.github/workflows/test.yml`, `.github/workflows/publish.yml`,
`docs/releasing.md`. Do not modify `publish-crate.yml`.

Conventions: every action pinned to a full commit SHA with a `# vX.Y.Z` comment (see
the existing workflows); `timeout-minutes` on jobs; `dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c # v1`
for Rust; `PyO3/maturin-action@e83996d129638aa358a18fbd1dfb82f0b0fb5d3b # v1.51.0`.
`maturin generate-ci github` (available in the venv) prints the reference layout —
use it for syntax, adapt to the conventions and to trusted publishing.

### `test.yml`

- Python job: matrix 3.9, 3.10, 3.11, 3.12, 3.13, 3.14 (drop 3.7/3.8). Steps: checkout,
  setup-python, Rust toolchain (stable), `python -m venv .venv`, `pip install maturin`,
  `maturin develop --features testing`, then the unittest command. Keep any existing
  script `--check` steps.
- Rust job: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets
  --all-features --locked -- -D warnings`; `cargo test --workspace --locked`;
  wasm32 lib build, rustdoc, MSRV 1.82 test and `cargo package` stay **core-crate only**
  (`-p mongol-norm`); the binding crate needs 1.83 and a Python on the runner (the
  ubuntu image has one).

### `publish.yml`

Trigger unchanged (`release: [published]`). Jobs:

- `verify` (ubuntu): tag `v<version>` equals `[workspace.package].version` in
  `Cargo.toml` (the only literal now), release commit is on `main`; build with
  `maturin develop --features testing` and run the Python suite.
- Wheel jobs with `PyO3/maturin-action` (`args: --release --out dist`, `sccache: true`
  optional): `linux` (ubuntu-latest; targets x86_64, aarch64; `manylinux: auto`),
  `musllinux` (targets x86_64, aarch64; `manylinux: musllinux_1_2`), `windows`
  (windows-latest; target x64), `macos` (macos-latest; targets x86_64, aarch64).
  Each natively runnable build (linux x86_64, windows x64, macos aarch64) gets a smoke
  step: `pip install --no-index --find-links dist mongol-norm`, `python -c "import
  mongol_norm; print(mongol_norm.MongolianShaper().shape('ᠰᠠᠢᠨ'))"`, and
  `mongol-norm shape ᠰᠠᠢᠨ`. Upload each `dist/` as an artifact with a unique name.
- `sdist` (maturin `command: sdist`).
- `publish`: `needs` all of the above, `if: github.repository == 'Satsrag/mongol-norm'`,
  `environment: pypi`, `permissions: id-token: write`, download all artifacts into
  `dist/`, list them, then the existing pinned `pypa/gh-action-pypi-publish` step.
  No API token anywhere.

### `docs/releasing.md`

Update the release procedure: bump only `[workspace.package].version` in `Cargo.toml`
(then `cargo update -w` for the lock), tag/release `vX.Y.Z` publishes wheels + sdist
to PyPI (this workflow) and the crate to crates.io (`publish-crate.yml`); describe the
wheel matrix and the sdist fallback (needs Rust ≥ 1.83); keep the crates.io trusted
publishing section as is.

Done when: `actionlint` is unavailable locally, so validate YAML by
`python -c "import yaml,sys; yaml.safe_load(open(p))"` for each file (PyYAML is in
the venv or `pip install pyyaml`), and re-read the maturin-action README for option
names you used (`target`, `manylinux`, `args`, `command`, `sccache`).

## Task 4 — documentation

Owned files: `README.md`, `crates/mongol-norm/README.md`, `docs/data-format.md`,
`docs/superpowers/specs/2026-09-01-rust-core-design.md` (status note only),
`openwiki/**` untouched (generated).

- `README.md` (EN and 中文 halves, keep them in sync): status/introduction no longer
  "pure Python" — the engine is the Rust crate, the PyPI package is its binding;
  Installation: `pip install mongol-norm` ships prebuilt wheels (Linux x86_64/aarch64
  glibc+musl, macOS Intel/Apple silicon, Windows x64; Python ≥ 3.9), sdist needs Rust
  ≥ 1.83; Requirements section; project-structure tree (`mongol_norm/_api.py`,
  `_native` extension, `shaper.py` shim, no `rules.py`, `crates/mongol-norm-py/`);
  Running-tests block: `pip install maturin && maturin develop --features testing`
  before the unittest command; the "Rust crate" section now says the crate is the
  engine behind the Python package (not a twin); test totals updated to the real
  numbers after Task 2 (ask the orchestrator or run the suites); remove the
  "twin/dual implementation" wording.
- `crates/mongol-norm/README.md`: one line noting the PyPI package `mongol-norm` is
  built on this crate (`crates/mongol-norm-py`).
- `docs/data-format.md`: the runtime no longer reads the JSON — the JSON is the
  source for `scripts/gen_rust_tables.py` and tooling; "reference implementation"
  pointers → `crates/mongol-norm/src/`; `mongol_norm._data` loaders remain for
  scripts/tests.
- `docs/superpowers/specs/2026-09-01-rust-core-design.md`: add a status line at the
  top pointing to the 2026-09-02 spec (the Python runtime was replaced by bindings).

## Task 5 — final verification and PR

- Full Python suite in the venv; `cargo fmt/clippy/test/doc`; MSRV 1.82 for the core
  crate (`cargo +1.82.0 test -p mongol-norm --locked` if the toolchain is installed);
  `gen_rust_tables.py --check`, `gen_compat_goldens.py --check`.
- `maturin build --release` → install the wheel into a fresh venv → `mongol-norm shape
  ᠰᠠᠢᠨ`, `python -c "import mongol_norm; print(mongol_norm.__version__)"`.
- Push, CI green, PR to `main` with a summary of API differences and the wheel
  matrix; the user merges.

## Task 6 — release 0.1.0 (after merge)

Create GitHub release `v0.1.0` on `main`; `publish.yml` and `publish-crate.yml` run.
Verify on PyPI (`pip install mongol-norm==0.1.0` in a clean venv on the dev Mac) and
crates.io. Update memory files.
