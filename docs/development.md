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
│   ├── normalize.rs    the canonical normalizer
│   ├── written_units.rs  the written-unit and positioned-written-unit encoders
│   ├── font.rs         FontShaper — the font backend (feature `font`: harfrust + fonts/)
│   ├── cli.rs          the mongol-norm command (src/bin/mongol-norm.rs is a shim)
│   └── generated/      tables generated from the JSON — never hand-edited
│                       (font_hudum.rs: the font's glyph table, from fonts/hudum.glyphs.txt)
├── fonts/              the font backend's shaping font: hudum.otf (mongfontbuilder's Hudum
│                       font, outlines removed), hudum.glyphs.txt, mongfontbuilder.json (the
│                       pinned upstream commit + checksums), NOTICE.txt, OFL.txt
├── tests/              the crate's integration tests and the shared fixtures
│   ├── data/           core-hud.tsv, eac-hud.tsv — vendored from mongfontbuilder
│   │   └── mongfontbuilder/  core-hudum.tsv, eac-hudum.tsv at the pinned commit
│   │                   (the font backend's fixtures: tests/font_*.rs)
│   └── golden/         mng-canonical-v1.jsonl, mng-phase-trace-v1.json
├── README.md           the GitHub landing page, the crates.io README, and a doctest
├── LICENSE  NOTICE     the crate's own licence files
├── assets/  docs/  .github/workflows/
├── web/                static Hudum playground; browser bindings in a separate Cargo workspace
│   ├── wasm/           thin wasm-bindgen adapter calling the root crate
│   └── public/         HTML/CSS/JS, hudum.otf, and generated (ignored) pkg/ bindings
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
    ├── scripts/        gen_rust_tables.py, gen_normalize_table.py, gen_compat_goldens.py,
    │                   gen_font_shaper.py (+ _font_glyphs.py, _font_dump.py),
    │                   preprocess.py, check_dist_metadata.py
    └── tests/          the Python suite; reads the fixtures from ../../tests/{data,golden}
```

## Running the tests

The browser playground has separate build prerequisites and browser tests; see
[`web/README.md`](../web/README.md). It does not add dependencies to the core workspace.

Rust, from the repository root:

```bash
cargo test --workspace --locked   # 327 tests: unit + corpus + goldens + properties + CLI + fuzz + the README doctest
cargo test -p mongol-norm --features font --locked   # + the font backend (Rust 1.85): the upstream Hudum
                                                     #   suites, font-vs-engine, its own contract, fuzz
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo package -p mongol-norm      # what crates.io would receive (fonts/ included)
```

Python, from `python/` — the suite drives the compiled extension, so build it into a virtualenv
first (needs Rust ≥ 1.83; the `testing` feature exposes the hook the fallback tests use). Re-run
`maturin develop` after any Rust change:

```bash
cd python
python -m venv .venv && source .venv/bin/activate
pip install 'maturin>=1.15,<2'
maturin develop --locked --features testing        # builds mongol_norm/_native

python -m unittest discover -s tests -p 'test_*.py'   # 308 tests
python -m unittest tests.test_shaper -v               # shape / same_shape / normalize
python -m unittest tests.test_round_trip              # round-trip + canonicity + prefix-stability
python -m unittest tests.test_core_hud tests.test_eac_hud   # the upstream TSV suites
python -m unittest tests.test_rust_twin               # tables fresh, versions in lockstep
```

The generators are run from the repository root; each `--check` fails when the committed output
differs from what it would generate now (all four run in CI):

```bash
python python/scripts/gen_rust_tables.py --check
python python/scripts/gen_normalize_table.py --check
python python/scripts/gen_compat_goldens.py --check
python python/scripts/gen_font_shaper.py --check
```

### Updating the bundled shaping font

The font backend tracks one upstream commit, pinned in `fonts/mongfontbuilder.json`. To move it
(needs `git`, `uv` and network access; the checkout's own environment supplies fontTools and
mongfontbuilder — nothing of mongfontbuilder is distributed, only the stripped font):

```bash
python3 python/scripts/gen_font_shaper.py --commit <full SHA>   # clone, build, strip, regenerate
cargo test -p mongol-norm --features font
```

It rewrites `fonts/`, `src/generated/font_hudum.rs` and `tests/data/mongfontbuilder/`, and refuses a
font that shows a written unit the rule data (`MNG.json`) does not have. Review the diff of
`fonts/hudum.glyphs.txt` and of the suites, then update the expected-failure list in
`tests/font_suites.rs` and the pinned disagreements in `tests/font_vs_engine.rs` where the run
tells you to.

### Comparing the two shapers

`tests/font_vs_engine.rs` holds the two backends to the same shape on every fixture word. To see
where they part on anything else — random strings, or the words of your own text — run the
`compare` example (release mode: the font backend is slow unoptimized):

```bash
cargo run --release --example compare --features font -- --random 100000         # random strings
cargo run --release --example compare --features font -- corpus.txt --tsv diff.tsv  # a text file
echo 'ᠹᠲ᠍ᠺ ᠰᠠᠢᠨ' | cargo run --release --example compare --features font            # stdin
```

The report goes to stdout: how many words differ, the differences grouped by the units the two
shapers disagree on (the part between the common prefix and suffix, `T` vs `D` for `F+T+K2` vs
`F+D+K2`) with an example word each, and the first differing words with both shapes and their
code points. `--tsv FILE` also writes every differing word to FILE (columns `word`,
`code_points`, `font`, `engine`, `font_part`, `engine_part`); a word only one side shapes — a
letter the font has no glyph for — is listed with the error. `--seed`, `--max-len`, `--limit` and
`--groups` tune the run; `--help` lists them.

CI runs it on every push, on 100k random strings with a fixed seed (the Rust job's "Compare the
font backend with the rule engine" step). It is a gate (`--fail-on-diff`): the two backends agree
on all of them, so any string they shape differently fails the build. The step reports:

- an **error annotation** on the run — the number of differing strings and the five largest
  groups, shown in the run's *Annotations* and on the pull request's checks (a notice when none
  differs);
- the **report in the job summary** (`--markdown "$GITHUB_STEP_SUMMARY"`) — the counts, every
  group with an example, and the first 100 differing words;
- every differing word in the **`font-vs-engine` artifact** (the TSV), kept for 14 days and
  uploaded when the step fails too.

The seed is fixed, so the result changes only when one of the backends does.

The engine follows the font everywhere, also where GB/T 25914-2023 or its EAC suite read an FVS
otherwise. Two such cases are an undefined FVS on a final n or g before an MVS (EAC XIM11-46,
skipped in `UTN_XFAIL_CASES` like mongfontbuilder's xfail) and two FVS between an initial d and
its final vowel. The WATCHED list in `tests/font_vs_engine.rs` fails when a font bump changes such
a case, and the rule's doc says what to change then.

Two more comparisons widen the net:

- **A fresh random sample on every run**: the job "Fresh random sample (report only)" seeds with
  the run id, so each run shapes 100k new strings. It is a check of its own, so a difference
  stands out on the pull request: that check fails, with an error annotation that names the seed
  and the `font-vs-engine-fresh` artifact, and the report goes into its job summary. It never
  fails the build: the other checks keep their result, and `continue-on-error` keeps the
  workflow run and the README badge green. Keep it out of any required checks. Reproduce a
  difference with `--random 100000 --seed <seed>`, then fix it or explain it.
- **The boundary words** (`boundary_words_shape_alike` in
  `tests/font_vs_engine.rs`, a gate through `cargo test --features font`): about 18k words built
  from templates around an MVS, a joiner, a ZWNJ or an FVS, the contexts the backends parted on
  until #50–#52 and #55, and those a ZWNJ meets. All of them must shape alike.

Current totals: **327 Rust tests** (unit + property + 177 core-hud and 3512 eac-hud corpus rows,
1988 canonical and 15 phase-trace golden vectors, fuzz, the ZWNJ cases and the README doctest; the
`font` feature adds 21) and **308 Python tests**, green on Rust stable / 1.82 (the core crate's
MSRV; the binding crate needs 1.83, the `font` feature 1.85) and CPython 3.9 and 3.14 (CI tests the
ends of the supported range; the extension targets the stable ABI, so the versions in between load
the same build).
