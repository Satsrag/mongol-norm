# mongol-norm

[![crates.io](https://img.shields.io/crates/v/mongol-norm.svg)](https://crates.io/crates/mongol-norm)
[![docs.rs](https://img.shields.io/docsrs/mongol-norm)](https://docs.rs/mongol-norm)
[![PyPI](https://img.shields.io/pypi/v/mongol-norm.svg)](https://pypi.org/project/mongol-norm/)
[![Test](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml/badge.svg)](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml)

[English](#english) | [中文](#中文)

---

<a id="english"></a>
## English

**mongol-norm** is a shape-aware normalizer for Traditional Mongolian (Hudum) script: the full
[UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) shaping pipeline plus an FVS-pinned
canonical normalizer, in a Rust crate with **zero dependencies** that also builds for
`wasm32-unknown-unknown`.

The same visible Mongolian word can be written as many different Unicode sequences, which breaks
search, deduplication and indexing. `shape()` renders a word into its written-unit sequence — what
a font would draw, computed without a font — and `normalize()` rewrites any encoding of a word into
the one canonical encoding for that shape.

The crate is the engine and lives at the repository root. `python/` holds a thin PyO3 binding,
published to PyPI as [`mongol-norm`](https://pypi.org/project/mongol-norm/); both are built from
one version literal, one set of data tables and one fixture set, and return byte-identical results
— see [Python package](#python-package).

### Status

Both halves of the library — shaping and normalization (MNG / Hudum) — are verified against the
same upstream corpora on every push, on Rust stable and the MSRV, and on CPython 3.9 – 3.14.

#### ✅ Shaping

`shape()` and `same_shape()` are cross-validated against two upstream TSV suites:

| Suite | Cases | Pass | Notes |
|---|---|---|---|
| `mongfontbuilder/core-hud.tsv` | 225 | **100%** | curated regression set |
| `mongfontbuilder/eac-hud.tsv` (GB/T 25914-2023) | 3513 | **100%** | 5 cases excluded as UTN ↔ EAC xfail, matching mongfontbuilder's own `pytest.mark.xfail` set |
| Hand-written unit tests | — | **100%** | shape / same_shape / joiner tokens (nirugu, ZWJ) |

Both TSV suites are the standard's *own* written-unit sequences, so they are checked against
`shape_raw` — the engine's output before the four duplicate encodings are folded out (see
[Duplicate encodings](#duplicate-encodings)). For 369 of the 3512 EAC rows and 29 of the 177 core
rows the public `shape` differs from what the standard spells.

#### ✅ Normalization — corpus-scoped guarantees, machine-checked

For written-unit shapes covered by the bundled normalization table, `normalize` / `normalize_text` /
`normalize_written_units` are **pure functions of shape**, with invariants checked in CI over every
corpus encoding:

| Property | Result |
|---|---|
| Round-trip — `shape(normalize(x)) == shape(x)` | **3757 / 3757** corpus encodings (100%) |
| Shape-canonicity — same shape ⟹ same Unicode output | **1991 / 1991** shape groups (100%) |
| Prefix-stability — word and word+suffix share their prefix encoding | **2241 / 2241** real corpus pairs (100%) |

Scope note: normalization is implemented for MNG (Hudum) only — Todo / Sibe / Manchu load shaping
rules but have no normalizer yet. The guarantees above cover the checked corpus and any input whose
written-unit chains can be encoded by the bundled table. For an uncovered out-of-corpus chain,
normalization fails closed with `Error::NormalizationFallback`; call
`normalize_allow_fallback` explicitly when returning the original word unchanged is acceptable.

This project was generated with [Claude Code](https://claude.ai/code) (AI-assisted coding). The
tests and key parts of the core code have been **manually reviewed**, and test coverage is extensive
(corpus round-trip / shape-canonicity / prefix-stability plus the upstream cross-implementation
suites). Treat this as a **preview release** — it should be fine for normal use; if you hit a
problem, please open an [issue or PR](https://github.com/Satsrag/mongol-norm/issues). Shaping logic
is derived from UTN #57 v4 and mongfontbuilder.

### Install

```bash
cargo add mongol-norm
```

or, by hand:

```toml
[dependencies]
mongol-norm = "0.2.0"
```

The CLI installs as a standalone binary:

```bash
cargo install mongol-norm
```

MSRV is **1.82**. API docs: [docs.rs/mongol-norm](https://docs.rs/mongol-norm).

### Usage

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
    assert_eq!(shaper.canonical_version(), Some("mng-canonical/2"));

    // The public shape omits the four duplicate encodings, so ᠠᠷᠠᠳ and ᠠᠷᠠᠤᠠ — one visible
    // word spelled two ways — are one shape and one canonical text
    assert_eq!(shaper.shape_str("ᠠᠷᠠᠳ")?, "A+A+R+A+O+A");
    assert!(shaper.same_shape("ᠠᠷᠠᠳ", "ᠠᠷᠠᠤᠠ")?);
    assert_eq!(shaper.normalize("ᠠᠷᠠᠳ")?, shaper.normalize("ᠠᠷᠠᠤᠠ")?);
    Ok(())
}
```

`shape_detailed` returns the per-token breakdown (code point, alias, position, FVS, condition,
written units) and `trace` returns the per-rule condition transitions behind the phase-trace golden
fixtures. The full API is on [docs.rs](https://docs.rs/mongol-norm).

### Command line

The crate ships a `mongol-norm` binary — the same CLI the Python package installs as its
`mongol-norm` command. It turns a word into its rendered written-unit sequence (`shape`), rewrites
any encoding of a word into the one canonical, FVS-pinned Unicode form (`normalize`), does the same
for every Mongolian word inside free-form text (`normalize-text`), encodes pre-shaped written units
(`normalize-written-units`), and tells whether two encodings render identically (`same`):

```bash
cargo install mongol-norm

# Inline text
mongol-norm shape 'ᠰᠠᠢᠨ'                        # → S+A+I+I+A (the rendered written units)
mongol-norm normalize 'ᠰᠡᠢᠨ'                    # one word → its canonical encoding
mongol-norm normalize --allow-fallback 'ᠰᠡᠢᠨ'   # keep the input if it is uncovered
mongol-norm normalize-text 'Hello ᠰᠡᠢᠨ'         # mixed script: Mongolian runs normalized
mongol-norm normalize-written-units 'B+Aa'      # → ᠪᠠ᠋
mongol-norm normalize-written-units 'BZwj'      # compact PascalCase units
mongol-norm same 'ᠰᠠᠢᠨ' 'ᠰᠡᠢᠨ'                  # exit 0 if identical, 1 if different

# Pipe / stdin (use `-` as the text)
echo 'B+Aa' | mongol-norm normalize-written-units -
cat doc.txt | mongol-norm normalize-text -

# File in / out
mongol-norm normalize-text -i in.txt -o out.txt

# Batch: one word per line in, one canonical per line out
mongol-norm normalize --batch -i words.txt -o canonical.txt
echo 'ᠰᠡᠢᠨ' | mongol-norm normalize --batch -
mongol-norm normalize-written-units --batch -i units.txt -o canonical.txt
```

`--locale MNG|TOD|SIB|MCH` selects the script (default `MNG`; only `MNG` normalizes). It is a
*global* option and goes before the sub-command — `mongol-norm --locale TOD shape 'ᠰᠠᠢᠨ'`, not
after it. `--allow-fallback` keeps an uncovered word instead of failing, `--` ends the options so a
following `-` is text, errors exit with code 2 (`same` prints `true`/`false` and exits 0/1), and
`mongol-norm --help` lists every flag.

`shape` and `normalize` take a *single word* and reject every character outside the Mongolian word
alphabet — including the newline `echo` appends, so pipe them with `printf`, with `--batch` (one
word per line), or use `normalize-text` for free-form text.

`normalize-written-units` accepts compact PascalCase or explicit `+` boundaries. Compact input must
have one unique segmentation; ambiguous input fails closed and must be rewritten with `+`. After
parsing, the same exact-shape validation as the `normalize_written_units` API applies, so a
syntactically valid unit stream can still be rejected when it has no canonical MNG encoding.

<a id="python-package"></a>
### Python package

```bash
pip install mongol-norm
```

The PyPI package is a thin PyO3 wrapper over this crate, built by maturin from `python/`. The engine
and its tables are compiled into the `mongol_norm._native` extension, so there are **no runtime
dependencies** and no Rust toolchain to install.

Prebuilt `cp39-abi3` wheels (one per platform, serving every CPython ≥ 3.9) cover Linux x86_64 and
aarch64 (glibc via manylinux2014, musl via musllinux_1_2), macOS x86_64 and Apple silicon, and
Windows x64; CI tests CPython 3.9 – 3.14. On any other platform pip falls back to the source
distribution, which compiles the extension locally and needs a Rust toolchain ≥ 1.83 (pip fetches
the `maturin` build backend itself).

```python
from mongol_norm import MongolianShaper

shaper = MongolianShaper(locale="MNG")

shaper.shape("ᠰᠠᠢᠨ")                        # → ['S', 'A', 'I', 'I', 'A']
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")           # → True
shaper.normalize("ᠰᠡᠢᠨ")                    # → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'
shaper.normalize_text("Hello ᠰᠡᠢᠨ world")   # → 'Hello ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌ world'
shaper.normalize_written_units(["B", "Aa"]) # → 'ᠪᠠ᠋'
```

The `mongol-norm` console script is the crate's CLI, run in-process. Full Python documentation — the
whole API, error types, the written-unit encoders and the CLI — is in
[`python/README.pypi.md`](https://github.com/Satsrag/mongol-norm/blob/main/python/README.pypi.md),
which is also the package's PyPI page.

**Upgrading from 0.0.x.** The public API (`MongolianShaper`, `NormalizationFallbackError`, the
`mongol-norm` command) is unchanged; `from mongol_norm.shaper import …` and
`python -m mongol_norm.shaper …` still work through a compatibility shim. Differences: Python ≥ 3.9
is required; an unknown locale raises `ValueError` instead of `FileNotFoundError`;
`mongol_norm.rules` and the private shaper internals no longer exist; the CLI is the crate's (adds
`-V/--version`; its remaining intentional differences are listed at the top of `src/cli.rs`). New:
`shaper.trace()`, `shaper.rule_names()`, `shaper.parse_written_units()`.

### Why this project exists

Traditional Mongolian script in Unicode has a fundamental problem: **the same visible word can be
encoded in multiple different Unicode sequences**. This happens because:

1. **Letters share glyphs** — A and E look identical in medial and final positions; O/U and OE/UE share forms; QA and GA share forms depending on vowel harmony.
2. **Multiple encoding paths** — The same tooth glyph (I) can be encoded as I, YA+FVS1, or even two separate I characters.
3. **Redundant FVS usage** — Free Variation Selectors (FVS1–FVS4) can create equivalent sequences that render identically.
4. **Joining controls** — nirugu (U+180A, the visible stem extender) and ZWJ (U+200D) force letters into their joined forms, and inside those joined contexts even more letters collapse to the same glyph (`nirugu+o` and `nirugu+u` render identically).
5. **Suffix particles after MVS/NNBSP** — the same rendered suffix can be spelled with different letters (`MVS+a` and `MVS+e` both render the chachlag form; `MVS+u` and `MVS+ue` render the same connector).

This means:
- **Search fails**: Searching for "sain" (one encoding) won't find the same word in another encoding, even though they look identical.
- **Deduplication breaks**: The same word has multiple Unicode representations.
- **Indexing is unreliable**: Different encodings of the same word produce different keys.

mongol-norm **shapes** the input with the full UTN #57 v4 shaping process (5-step conditional
mapping), **compares** glyph sequences to detect identical visual forms, and **normalizes**
supported shapes to one canonical, FVS-pinned Unicode encoding — same shape ⟹ same Unicode, with an
exact shape round-trip.

**Example**: All five of these encode the word "sain" (good) and look identical:

![Five encodings of "sain" all normalizing to the same canonical form](https://raw.githubusercontent.com/Satsrag/mongol-norm/main/assets/sain-variants.png)

### How it works

The normalizer implements a **lightweight Mongolian shaping engine** — equivalent to what HarfBuzz
does with a font file, but using only the rule data from
[UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) and the
[mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) project. No font files needed.

#### Shaping pipeline (UTN #57 v4 Mongolian-specific phase)

1. **Chachlag** — Suffix forms for A/E after MVS (Mongolian Vowel Separator)
2. **Syllabic** — Consonant/vowel context: onset, devsger, marked, masculine/feminine harmony, dotless
3. **Particle** — MVS particle dictionary lookup for specific suffix words
4. **Devsger** — I after a vowel (vowel_devsger) gets double-tooth form: `I → I+I`
5. **Post-bowed** — Vowel forms change after bowed consonants (G, B, K, P, F)

#### Duplicate encodings

`shape` promises to be a fingerprint of the *visible* word — "same shape ⟹ same normalize" is this
crate's whole reason to exist. Four written units break that promise: they render as exactly the
same ink as a sequence of other units, so two spellings of one word shaped differently.

| Unit | Public shape | Same ink as |
|---|---|---|
| `Dd` (medial and final — the only positions it has) | `O A` | `O:medi` + `A:medi` / `A:fina` |
| `H` medial | `A A` | `A:medi` + `A:medi` |
| `Hx` medial | `N N` | `N:medi` + `N:medi` |

The pairs were compared by rendering them in Noto Sans Mongolian 3.002 — the UTN #57 reference
build — and diffing the pixels: identical advance, differences only at the anti-aliasing level
(58 of 34128 px, max Δ34, for `Dd:medi`). The other five units ZVVNMOD spells with two glyphs
(`Cr:init`, `B2:fina`, `G:fina`, `A:isol`, `Aa:fina`) are **not** duplicates — they render at a
different width, and two of them would expand into themselves.

So `shape` folds all four out, and none of them ever appears in its output. `same_shape`,
`normalize` and the written-unit encoders all see the collapsed sequence; `normalize_written_units`
still *accepts* the duplicates as input and folds them, so existing caller data keeps working.
`ᠠᠷᠠᠳ` and `ᠠᠷᠠᠤᠠ` are one visible word, and now one shape and one canonical text.

UTN #57 and GB/T 25914-2023 keep all four as distinct written units — their EAC vectors spell
`ᠠᠷᠭᠠᠯ` as `A A R Hx A L` — and the engine still produces them. The standard's own sequence stays
reachable through `Shaper::shape_raw` (Python: `MongolianShaper._shape_raw`), which is what the
conformance suites compare against. It is **not part of the public contract**: it is `#[doc(hidden)]`
and may change to fold further duplicates without a major bump.

`shape_detailed` and `trace`'s `written_by_token` report each token's own units, so they are raw
too — the collapse is a whole-word rewrite that no single token can carry. `trace`'s `shape` field
is the public, collapsed sequence.

#### Normalization strategy

Within the normalization table's supported written-unit domain, `normalize` is a **pure function of
shape**: any two encodings that shape identically produce the same Unicode output, and the output
round-trips — `shape(normalize(x)) == shape(x)`. It is also **prefix-stable**. When these goals
conflict the priority is **round-trip > prefix-stable > shortest**.

Per word:

1. **shape** the input into its written-unit sequence. Structural characters — MVS, nirugu, ZWJ — appear verbatim as PascalCase `Mvs` / `Nirugu` / `Zwj` tokens (nirugu renders a visible stem; all three are the evidence for a neighbour's init/medi/fina form). **Split** the shape at these tokens into *chains*; the tokens themselves are copied through unchanged.
2. **encode each chain** (right-to-left, so appending a suffix can't disturb what precedes it):
   1. **partition + table lookup** — the primary path. At each position take the single unit if the table has it (preferred — clean output), else the longest available multi-unit entry, and look up `(position, written-unit) → (letter, FVS)` in an FVS-pinned table. Each value renders its unit **regardless of neighbours**, so the result is a deterministic, O(N), prefix-stable function of the shape.
   2. **velar-feminine refinement** — a `G`/`Gx` velar's forward-coupled vowel (`a`/`o`/`u`) is swapped to its feminine partner (`e`/`oe`/`ue`) for clean output.
   3. **verify** — reshape the candidate in full context; accept only if it equals the target chain shape.
   4. **no search fallback** — the table is total over the corpus (with FVS-first selection there are no gap chains left). If an out-of-corpus shape ever misses the table, normalization fails closed with the input and the uncovered written-unit sequence. Callers may explicitly ask for the lenient variant to return the input unchanged (round-trip preserved, never a mis-encoding). A letter next to a joiner simply looks its unit up at the shifted (joined) position.
3. **post-MVS suffix rule** — a chain directly after MVS takes its **standalone** canonical (drop the MVS, normalize, re-attach), so the spelling never depends on MVS. One exception: chachlag `Aa` after MVS is written the bare letter `a`. (The isolate-`I` → `i+FVS1` spelling is pinned in the table itself — no post-processing pass exists.)

**Prefix-stability** means: if word *A* = word *B* + a suffix and their shapes share a prefix, the
shared region encodes identically except the single boundary unit whose position changes (final in
*B* → medial in *A*). The per-unit table delivers this for free — each unit's encoding depends only
on its own position, never on its neighbours.

**How the table is built** (the *selection method*): offline, a **context-independence battery**
fills each `(position, written-unit)` slot with the `(letter, FVS)` that renders *exactly* that unit
in *every* probed neighbour context (the probes include a bowed consonant, so post-bowed effects
can't hide). Candidate order is **letter-major, FVS-first within the letter** — an FVS exists
precisely to pin a form against context, so the pinned variant of the right letter always beats its
context-sensitive bare form. The result is exported as JSON; the battery lives in
`python/scripts/gen_normalize_table.py`.

> Note: supported output is **FVS-pinned**, not bare — each unit carries the selector that fixes its
> form independent of context. This is what makes "same shape ⟹ same Unicode" and prefix-stability
> hold inside the table's domain.

The exact canonical selection policy is frozen as **`mng-canonical/2`**. It is available as
`Shaper::canonical_version` (Python: `shaper.canonical_version`) and embedded in
`MNG.normalize.json`. Applications that persist normalized search/index keys should store this
version alongside them and rebuild those keys if a future release changes it.

**`mng-canonical/2` (0.2.0) invalidates keys stored under `mng-canonical/1`.** Folding the
duplicate encodings out of `shape` changed the canonical text of every word containing one — 282
of the 1993 corpus shape groups, two of which merged with another group. Rebuild any stored
normalized key.

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
│   ├── normalize.rs    the canonical normalizer
│   ├── written_units.rs  the written-unit and positioned-written-unit encoders
│   ├── cli.rs          the mongol-norm command (src/bin/mongol-norm.rs is a shim)
│   └── generated/      tables generated from the JSON — never hand-edited
├── tests/              the crate's integration tests and the shared fixtures
│   ├── data/           core-hud.tsv, eac-hud.tsv — vendored from mongfontbuilder
│   └── golden/         mng-canonical-v1.jsonl, mng-phase-trace-v1.json
├── README.md           this file: the GitHub landing page, the crates.io README, and a doctest
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
    ├── scripts/        gen_rust_tables.py, gen_normalize_table.py, gen_compat_goldens.py,
    │                   preprocess.py, check_dist_metadata.py
    └── tests/          the Python suite; reads the fixtures from ../../tests/{data,golden}
```

### Data and fixtures

The shaping and normalize rules are flat, language-agnostic JSON in `python/mongol_norm/data/`
(`MNG.json`, `TOD.json`, `SIB.json`, `MCH.json` and `MNG.normalize.json`). Nothing reads it at
runtime: `python/scripts/gen_rust_tables.py` compiles it into the static Rust tables in
`src/generated/`, which is what both the crate and the wheel carry. The wheel still ships the JSON
for tooling. The schema and the consuming algorithm are documented in
[`docs/data-format.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/data-format.md), so a
port in another language needs only a JSON parser.

Both test suites read the same fixtures, which live once under the crate's `tests/`:

| Fixture | Contents |
|---|---|
| `tests/data/core-hud.tsv` | 177 rows — mongfontbuilder's curated regression set (225 cases) |
| `tests/data/eac-hud.tsv` | 3512 rows — GB/T 25914-2023 (3513 cases, 5 UTN-xfail) |
| `tests/golden/mng-canonical-v1.jsonl` | 1991 canonical vectors |
| `tests/golden/mng-phase-trace-v1.json` | 15 phase-trace vectors |

Because the corpus and golden tests read that directory, `cargo test` needs a repository checkout —
the published crate does not include the fixtures.

### Running the tests

Rust, from the repository root:

```bash
cargo test --workspace --locked   # 256 tests: unit + corpus + goldens + properties + CLI + fuzz + this README
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

python -m unittest discover -s tests -p 'test_*.py'   # 253 tests
python -m unittest tests.test_shaper -v               # shape / same_shape / normalize
python -m unittest tests.test_round_trip              # round-trip + canonicity + prefix-stability
python -m unittest tests.test_core_hud tests.test_eac_hud   # the upstream TSV suites
python -m unittest tests.test_rust_twin               # tables fresh, versions in lockstep
```

The generators are run from the repository root; each `--check` fails when the committed output
differs from what it would generate now (all three run in CI):

```bash
python python/scripts/gen_rust_tables.py --check
python python/scripts/gen_normalize_table.py --check
python python/scripts/gen_compat_goldens.py --check
```

Current totals: **256 Rust tests** (unit + property + 177 core-hud and 3512 eac-hud corpus rows,
1993 canonical and 15 phase-trace golden vectors, fuzz, and this README's doctests) and **253 Python
tests**, green on Rust stable / 1.82 (the core crate's MSRV; the binding crate needs 1.83) and
CPython 3.9 – 3.14.

### Use cases

- **Search & retrieval** — index Mongolian text with unique keys per visual word
- **Deduplication** — detect identical words encoded differently
- **Spell checking** — normalize before dictionary lookup
- **Corpus linguistics** — consistent word frequency counts
- **OCR post-processing** — standardize OCR output that may use inconsistent encodings
- **Input method engines** — validate and normalize user input

### Supported locales

| Locale | Script | Status |
|--------|--------|--------|
| MNG | Hudum (Traditional Mongolian) | ✅ Full shaping + normalization |
| TOD | Todo | ⬜ Shaping rules generated, normalization WIP |
| SIB | Sibe | ⬜ Shaping rules generated, normalization WIP |
| MCH | Manchu | ⬜ Shaping rules generated, normalization WIP |

### Documentation

- [docs.rs/mongol-norm](https://docs.rs/mongol-norm) — the full crate API
- [`docs/data-format.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/data-format.md) — the JSON schema and the normalize algorithm, for ports in other languages
- [`python/README.pypi.md`](https://github.com/Satsrag/mongol-norm/blob/main/python/README.pypi.md) — the Python package
- [`docs/releasing.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/releasing.md) — how a release is cut
- [`docs/superpowers/specs/2026-09-01-rust-core-design.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/superpowers/specs/2026-09-01-rust-core-design.md) — the engine design and fidelity contract
- [`docs/superpowers/specs/2026-09-02-python-bindings-design.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/superpowers/specs/2026-09-02-python-bindings-design.md) — the Python bindings design

### Data sources & acknowledgments

- **[UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html)** — Unicode Technical Note: Encoding and Shaping of the Mongolian Script. The authoritative specification for Mongolian shaping rules.
- **[mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder)** by Kushim Jiang — source of the bundled flat variant tables in `python/mongol_norm/data/` (preprocessed from `data.variants` / `data.particles`) and of the `core-hud.tsv` / `eac-hud.tsv` regression suites vendored into `tests/data/`. Both UTN #57 and mongfontbuilder are authored by the same person.
- **[GB/T 25914—2023](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=BD6429DE5A7FC782FAAE13938A07166E)** — China national standard for Traditional Mongolian nominal characters; source of the EAC compliance test set.
- **[Claude Code](https://claude.ai/code)** — this project was developed with AI assistance. The shaping rules are derived from the above sources; Claude Code was used to implement and structure the engine.

### License

MIT License — see [LICENSE](https://github.com/Satsrag/mongol-norm/blob/main/LICENSE).

The shaping rules and bundled data are derived from
[`mongfontbuilder`](https://github.com/Kushim-Jiang/mongfontbuilder) (MIT) and UTN #57. Their
required notices are retained in
[NOTICE](https://github.com/Satsrag/mongol-norm/blob/main/NOTICE).

---

<a id="中文"></a>
## 中文

**mongol-norm** 是传统蒙古文（回鹘式，Hudum）的形态感知规范化器：完整实现
[UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) 整形流程，以及 FVS 钉死的 canonical
规范化器；**零依赖** Rust crate，同时可编译到 `wasm32-unknown-unknown`。

同一个可见的蒙古文词可以写成多种不同的 Unicode 序列，这会破坏搜索、去重和索引。`shape()` 把词渲染成
书写单元序列——即字体会画出的形状，但无需字体——`normalize()` 则把同一个词的任意编码改写成该形态唯一的
canonical 编码。

crate 就是引擎，位于仓库根目录。`python/` 下是它的一层薄 PyO3 绑定，以
[`mongol-norm`](https://pypi.org/project/mongol-norm/) 发布到 PyPI；二者共用一个版本字面量、一套数据表和
一套固件，结果逐字节相同——见 [Python 包](#python-包)。

### 状态

库的两部分——整形与规范化（MNG / Hudum）——每次 push 都对照同一批上游语料验证，覆盖 Rust stable 与 MSRV、
以及 CPython 3.9 – 3.14。

#### ✅ Shaping（整形）

`shape()` 和 `same_shape()` 对照两套上游 TSV 套件交叉验证：

| 套件 | 用例数 | 通过 | 说明 |
|---|---|---|---|
| `mongfontbuilder/core-hud.tsv` | 225 | **100%** | 精选回归集 |
| `mongfontbuilder/eac-hud.tsv` (GB/T 25914-2023) | 3513 | **100%** | 5 个 UTN ↔ EAC 分歧 case 跳过（跟 mongfontbuilder 自己的 `pytest.mark.xfail` 列表一致） |
| 手写单元测试 | — | **100%** | shape / same_shape / joiner token（nirugu、ZWJ） |

两套 TSV 用的都是国标**自己**的书写单元序列，因此对照 `shape_raw` 检查——即引擎折叠四个重复编码之前的输出
（见[重复编码](#重复编码)）。3512 行 EAC 中有 369 行、177 行 core 中有 29 行，公开 `shape` 与国标的拼法不同。

#### ✅ Normalization（规范化）— 语料域保证，机器验证

对于内置规范化表覆盖的 written-unit shape，`normalize` / `normalize_text` /
`normalize_written_units` 是 **shape 的纯函数**，以下不变量在 CI 中对每一条语料编码逐一验证：

| 性质 | 结果 |
|---|---|
| 往返 —— `shape(normalize(x)) == shape(x)` | **3757 / 3757** 语料编码（100%） |
| 同形同码 —— shape 相同 ⟹ 输出 Unicode 相同 | **1991 / 1991** shape 组（100%） |
| 前缀稳定 —— 词与词+后缀共享前缀编码 | **2241 / 2241** 真实语料词对（100%） |

范围说明：规范化目前只实现了 MNG（Hudum）—— Todo / 锡伯文 / 满文已加载 shaping 规则，尚无规范化。上述保证
覆盖已检查语料及内置表可编码的 written-unit chain。语料外 chain 若未被覆盖，默认 fail closed，返回
`Error::NormalizationFallback`；只有明确接受原样回退时才调用 `normalize_allow_fallback`。

本项目由 [Claude Code](https://claude.ai/code)（AI 辅助编码）生成；测试与部分核心代码经**人工审核**，测试
覆盖比较充分（语料往返 / 同形同码 / 前缀稳定 + 上游跨实现套件）。当前为**预览版**，正常使用应无问题；遇到
问题欢迎提 [issue 和 PR](https://github.com/Satsrag/mongol-norm/issues)。Shaping 逻辑源自 UTN #57 v4 和
mongfontbuilder。

### 安装

```bash
cargo add mongol-norm
```

或手写：

```toml
[dependencies]
mongol-norm = "0.2.0"
```

命令行工具装成独立二进制：

```bash
cargo install mongol-norm
```

MSRV 为 **1.82**。API 文档见 [docs.rs/mongol-norm](https://docs.rs/mongol-norm)。

### 使用方法

```rust
use mongol_norm::{Error, Locale, PositionedWrittenUnit, Shaper, UnitPosition, WrittenUnit};

fn main() -> Result<(), Error> {
    let shaper = Shaper::new(Locale::Mng);

    // 字形化：书写单元序列
    assert_eq!(shaper.shape("ᠰᠠᠢᠨ")?.len(), 5); // [S, A, I, I, A]
    assert_eq!(shaper.shape_str("ᠰᠠᠢᠨ")?, "S+A+I+I+A");

    // 比较：两个编码视觉上是否相同？
    assert!(shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")?);

    // 规范化为唯一的 FVS 钉死 canonical 编码（严格模式；未覆盖的 shape 会报错，
    // `normalize_allow_fallback` 则原样返回）
    let canonical = shaper.normalize("ᠰᠡᠢᠨ")?;
    assert_eq!(shaper.normalize("ᠰᠠᠶ᠋ᠢᠨ")?, canonical);

    // 自由文本：只规范化蒙古文词，其余原样保留
    let text = shaper.normalize_text("Hello ᠰᠡᠢᠨ world")?;
    assert!(text.starts_with("Hello ") && text.ends_with(" world"));

    // 书写单元与权威 HUD position
    assert_eq!(
        shaper.normalize_written_units(&[WrittenUnit::B, WrittenUnit::Aa])?,
        "ᠪᠠ᠋"
    );
    let records = [
        PositionedWrittenUnit::new(WrittenUnit::B, UnitPosition::Init),
        PositionedWrittenUnit::new(WrittenUnit::Aa, UnitPosition::Fina),
    ];
    assert_eq!(shaper.normalize_positioned_written_units(&records)?, "ᠪᠠ᠋");
    assert_eq!(shaper.canonical_version(), Some("mng-canonical/2"));

    // 公开 shape 不含四个重复编码，所以 ᠠᠷᠠᠳ 与 ᠠᠷᠠᠤᠠ ——同一个可见词的两种拼法——
    // 是同一个 shape、同一个 canonical 文本
    assert_eq!(shaper.shape_str("ᠠᠷᠠᠳ")?, "A+A+R+A+O+A");
    assert!(shaper.same_shape("ᠠᠷᠠᠳ", "ᠠᠷᠠᠤᠠ")?);
    assert_eq!(shaper.normalize("ᠠᠷᠠᠳ")?, shaper.normalize("ᠠᠷᠠᠤᠠ")?);
    Ok(())
}
```

`shape_detailed` 返回逐 token 的细节（码位、alias、position、FVS、condition、书写单元），`trace` 返回
phase-trace golden 固件背后的逐规则 condition 变化。完整 API 见
[docs.rs](https://docs.rs/mongol-norm)。

### 命令行

crate 自带 `mongol-norm` 二进制——Python 包安装的 `mongol-norm` 命令就是它。`shape` 输出词渲染出的书写
单元序列，`normalize` 把同一个词的任意编码统一成唯一的 canonical（FVS 钉死）形式，`normalize-text` 只
规范化自由文本中的蒙古文词，`normalize-written-units` 编码已 shape 的书写单元，`same` 判断两种编码是否
同形：

```bash
cargo install mongol-norm

# 直接传文本
mongol-norm shape 'ᠰᠠᠢᠨ'                        # → S+A+I+I+A（渲染出的书写单元序列）
mongol-norm normalize 'ᠰᠡᠢᠨ'                    # 单词 → canonical 编码
mongol-norm normalize --allow-fallback 'ᠰᠡᠢᠨ'   # 未覆盖时原样返回
mongol-norm normalize-text 'Hello ᠰᠡᠢᠨ'         # 混合文字：只规范化蒙古文词
mongol-norm normalize-written-units 'B+Aa'      # → ᠪᠠ᠋
mongol-norm normalize-written-units 'BZwj'      # 紧凑 PascalCase 单元串
mongol-norm same 'ᠰᠠᠢᠨ' 'ᠰᠡᠢᠨ'                  # 同形退出码 0，不同为 1

# 管道 / 标准输入（文本位置写 `-`）
echo 'B+Aa' | mongol-norm normalize-written-units -
cat doc.txt | mongol-norm normalize-text -

# 文件输入 / 输出
mongol-norm normalize-text -i in.txt -o out.txt

# 批量：一行一词输入，一行一个 canonical 输出
mongol-norm normalize --batch -i words.txt -o canonical.txt
echo 'ᠰᠡᠢᠨ' | mongol-norm normalize --batch -
mongol-norm normalize-written-units --batch -i units.txt -o canonical.txt
```

`--locale MNG|TOD|SIB|MCH` 选文种（默认 `MNG`，只有 `MNG` 支持规范化）。它是**全局**选项，要写在子命令
之前——`mongol-norm --locale TOD shape 'ᠰᠠᠢᠨ'`，不能写在子命令后面。`--allow-fallback` 原样保留未覆盖
的词，`--` 结束选项（其后的 `-` 当文本），出错退出码 2（`same` 打印 `true`/`false`，退出码 0/1），
`mongol-norm --help` 列出全部参数。

`shape` 和 `normalize` 处理**单个词**，拒绝蒙古文词字母表之外的任何字符——包括 `echo` 追加的换行，所以管道
请用 `printf`，或者用 `--batch`（一行一词），自由文本请用 `normalize-text`。

`normalize-written-units` 接受紧凑 PascalCase 或显式 `+` 边界。紧凑输入必须只有一种合法切分；存在歧义时
fail closed，须改用 `+`。解析后继续执行与 `normalize_written_units` API 相同的 exact-shape 校验，因此语法
合法的 unit stream 若没有 canonical MNG 编码仍会被拒绝。

<a id="python-包"></a>
### Python 包

```bash
pip install mongol-norm
```

PyPI 包是本 crate 之上的一层薄 PyO3 封装，由 maturin 从 `python/` 构建。引擎及其数据表已编译进
`mongol_norm._native` 扩展，因此**零运行时依赖**，也不需要安装 Rust 工具链。

预编译的 `cp39-abi3` wheel（每个平台一个，服务全部 CPython ≥ 3.9）覆盖 Linux x86_64 与 aarch64
（glibc 走 manylinux2014，musl 走 musllinux_1_2）、macOS x86_64 与 Apple silicon、Windows x64；CI 实测
CPython 3.9 – 3.14。其他平台 pip 会回退到源码包（sdist），在本地编译扩展，需要 Rust ≥ 1.83 工具链
（`maturin` 构建后端由 pip 自动获取）。

```python
from mongol_norm import MongolianShaper

shaper = MongolianShaper(locale="MNG")

shaper.shape("ᠰᠠᠢᠨ")                        # → ['S', 'A', 'I', 'I', 'A']
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")           # → True
shaper.normalize("ᠰᠡᠢᠨ")                    # → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'
shaper.normalize_text("Hello ᠰᠡᠢᠨ world")   # → 'Hello ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌ world'
shaper.normalize_written_units(["B", "Aa"]) # → 'ᠪᠠ᠋'
```

`mongol-norm` 控制台脚本就是 crate 的 CLI，在进程内运行。完整的 Python 文档——全部 API、异常类型、书写
单元编码器与命令行——见
[`python/README.pypi.md`](https://github.com/Satsrag/mongol-norm/blob/main/python/README.pypi.md)，它同时
是该包的 PyPI 页面。

**从 0.0.x 升级。** 公开 API（`MongolianShaper`、`NormalizationFallbackError`、`mongol-norm` 命令）不变；
`from mongol_norm.shaper import …` 与 `python -m mongol_norm.shaper …` 通过兼容 shim 仍可用。差异：需要
Python ≥ 3.9；未知 locale 抛 `ValueError`（原为 `FileNotFoundError`）；`mongol_norm.rules` 及 shaper 私有
内部实现已移除；命令行改为 crate 自带的 CLI（新增 `-V/--version`；其余刻意差异见 `src/cli.rs` 顶部）。
新增：`trace()`、`rule_names()`、`parse_written_units()`。

### 为什么做这个项目

传统蒙古文在 Unicode 中存在一个根本性问题：**同一个可见词形可以用多种不同的 Unicode 序列编码**。原因是：

1. **字母共享字形** — A 和 E 在中间和尾部位置外形完全相同；O/U、OE/UE 共享形态；QA 和 GA 根据元音和谐共享形态。
2. **多种编码路径** — 同一个齿形字形可以编码为 I、YA+FVS1，甚至两个独立的 I 字符。
3. **冗余的 FVS 使用** — 自由变体选择符（FVS1–FVS4）可以创建渲染结果完全相同的等价序列。
4. **连接控制符** — nirugu（U+180A，可见的连笔延长符）和 ZWJ（U+200D）会强制字母取连接形，而连接语境下更多字母塌缩成同一字形（`nirugu+o` 和 `nirugu+u` 渲染完全相同）。
5. **MVS/NNBSP 后的后缀词** — 同一个渲染出的后缀可以用不同字母拼写（`MVS+a` 和 `MVS+e` 都渲染 chachlag 形；`MVS+u` 和 `MVS+ue` 同形）。

这意味着：
- **搜索失效**：搜索同一个词的某种编码，找不到另一种编码，尽管它们外形完全一样。
- **去重失败**：同一个词有多种 Unicode 表示。
- **索引不可靠**：同一个词的不同编码产生不同的索引键。

mongol-norm 使用完整的 UTN #57 v4 shaping 过程（5 步条件映射）对输入进行**字形化**，通过比较字形序列
**检测**视觉上相同的词形，并把受支持的 shape **规范化**为唯一的、FVS 钉死的 canonical Unicode 编码——
同 shape ⟹ 同 Unicode，并精确往返还原。

**示例**：以下五种编码都表示 "sain"（好的），外形完全相同：

![五种 sain 编码全部规范化为同一个标准形式](https://raw.githubusercontent.com/Satsrag/mongol-norm/main/assets/sain-variants.png)

### 工作原理

本规范化器实现了一个**轻量级蒙古文 shaping 引擎**——功能相当于 HarfBuzz 配合字体文件所做的事情，但仅使用
[UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) 的规则数据和
[mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) 项目的变体数据。**不需要字体文件**。

#### Shaping 管线（UTN #57 v4 蒙古文特定阶段）

| 步骤 | 名称 | 说明 |
|------|------|------|
| 1 | Chachlag | MVS（蒙古文元音分隔符）后的 a/e 后缀形态 |
| 2 | Syllabic | 辅音/元音上下文：onset/devsger/marked/阴阳和谐/dotless |
| 3 | Particle | MVS 小品词词典查找 |
| 4 | Devsger | 元音后的 i 获得双齿形态：`I → I+I`（vowel_devsger） |
| 5 | Post-bowed | 弓形辅音（G/B/K/P/F）后的元音形态变化 |

#### 重复编码

`shape` 承诺是**可见**词的指纹——“同 shape ⟹ 同 normalize”正是这个 crate 存在的理由。有四个书写单元
打破了这个承诺：它们与另一串单元渲染出完全相同的墨迹，导致同一个词的两种拼法 shape 不同。

| 单元 | 公开 shape | 与之墨迹相同 |
|---|---|---|
| `Dd`（词中与词末——它仅有的两个位置） | `O A` | `O:medi` + `A:medi` / `A:fina` |
| 词中 `H` | `A A` | `A:medi` + `A:medi` |
| 词中 `Hx` | `N N` | `N:medi` + `N:medi` |

判定方法是用 Noto Sans Mongolian 3.002（UTN #57 参考构建）渲染后逐像素比对：advance 完全相同，差异只在
反锯齿层面（`Dd:medi` 为 34128 像素中的 58 个，最大 Δ34）。ZVVNMOD 里另外五个用两个字形拼写的单元
（`Cr:init`、`B2:fina`、`G:fina`、`A:isol`、`Aa:fina`）**不是**重复编码——它们的宽度不同，且其中两个会展开成
自身。

因此 `shape` 把这四个全部折叠掉，输出中永远不会出现。`same_shape`、`normalize` 和书写单元编码器看到的都是
折叠后的序列；`normalize_written_units` 仍然**接受**重复编码作为输入并折叠它们，调用方已有的数据继续可用。
`ᠠᠷᠠᠳ` 与 `ᠠᠷᠠᠤᠠ` 是同一个可见词，现在也是同一个 shape、同一个 canonical 文本。

UTN #57 与 GB/T 25914-2023 把这四个保留为不同的书写单元——其 EAC 向量把 `ᠠᠷᠭᠠᠯ` 拼作 `A A R Hx A L`——
引擎也仍然产出它们。国标自己的序列通过 `Shaper::shape_raw`（Python：`MongolianShaper._shape_raw`）依然可达，
一致性套件比对的就是它。它**不属于公开契约**：标了 `#[doc(hidden)]`，将来可能在不升 major 的情况下折叠更多
重复编码。

`shape_detailed` 与 `trace` 的 `written_by_token` 报告的是每个 token 自身的单元，因此也是原始序列——折叠是
整词级的改写，单个 token 承载不了。`trace` 的 `shape` 字段则是公开的折叠序列。

#### 规范化策略

在规范化表支持的 written-unit 域内，`normalize` 是 **shape 的纯函数**：任意两个 shape 相同的编码，
normalize 输出相同，且往返成立 —— `shape(normalize(x)) == shape(x)`，同时**前缀稳定**。三者冲突时优先级：
**往返 > 前缀稳定 > 最短**。

逐词：

1. **shape** 成书写单元序列。结构字符 —— MVS、nirugu、ZWJ —— 原样输出为 PascalCase `Mvs` / `Nirugu` / `Zwj` token（nirugu 是可见的连笔字形；三者都是邻居字母 init/medi/fina 形的依据）。按这些 token **切成 chain**，token 本身原样拷贝。
2. **逐 chain 编码**（从右往左，这样加后缀不影响前面）：
   1. **划分 + 查表**（主路径）：每个位置优先取单单元（输出干净），否则取最长多单元，查 `(位置, 书写单元) → (字母, FVS)` 的 FVS 钉死表。每个值**不依赖邻居**就渲染出该单元 → 确定性、O(N)、前缀稳定。
   2. **velar 阴性微调**：`G`/`Gx` 前向耦合的元音（`a`/`o`/`u`）换成阴性（`e`/`oe`/`ue`），输出更干净。
   3. **校验**：在完整上下文里重新 shape，只接受与目标 chain shape 一致的结果。
   4. **没有搜索兜底**：FVS 优先的选择下，表对全部语料 chain 是完备的（缺口为零）。语料外的 shape 万一查不到表，默认 fail closed，带上原输入与未覆盖的 written-unit 序列；只有明确接受原样回退时才调用 lenient 变体（保住往返，绝不错编）。紧邻 joiner 的字母只是按移动后的连接位置查表。
3. **MVS 后缀规则**：紧跟 MVS 的 chain 用其 **standalone** canonical（去掉 MVS、归一、再拼回），拼写不依赖 MVS。唯一例外：MVS 后的 chachlag `Aa` 写裸字母 `a`。（孤立 `I` → `i+FVS1` 的拼写已钉进表本身 —— 不存在后处理。）

**前缀稳定**的含义：若词 *A* = 词 *B* + 后缀，且二者 shape 共享前缀，则共享部分编码完全一致，只有那个位置
发生变化的边界单元不同（在 *B* 里是词尾、在 *A* 里变词中）。逐单元表天然保证这点——每个单元的编码只取决
于它自己的位置，与邻居无关。

**表是怎么来的**（*选择方法*）：离线跑一个 **context 无关性电池**——对每个 `(位置, 书写单元)`，挑出在
**所有**探测邻居上下文里都**恰好**渲染出该单元的 `(字母, FVS)`（探针含弓形辅音，post-bowed 效应藏不住）。
候选顺序是**字母优先、字母内 FVS 优先**——FVS 的意义就是把字形从 context 里隔离出来，所以正确字母的钉死形
永远优于其受感染的裸形。结果导出成 JSON；电池在 `python/scripts/gen_normalize_table.py`。

> 注意：受支持输出是 **FVS 钉死**而非 bare —— 每个单元都带着把字形固定住、不受上下文影响的选择符，这正是
> “同 shape ⟹ 同 Unicode”和前缀稳定在表内成立的原因。

当前精确 canonical 选择策略冻结为 **`mng-canonical/2`**。可通过 `Shaper::canonical_version`（Python：
`shaper.canonical_version`）读取，并写入 `MNG.normalize.json`。持久化规范化搜索键/索引键的应用应同时保存
该版本；未来版本若发生变化，应重建这些键。

**`mng-canonical/2`（0.2.0）会使 `mng-canonical/1` 下存储的键失效。** 把重复编码折叠出 `shape` 之后，
凡含有这四个单元之一的词，canonical 文本都变了——1993 个语料 shape 组里有 282 个，其中 2 个与别的组合并。
已存储的规范化键必须重建。

### 仓库结构

```text
mongol-norm/
├── Cargo.toml          crate（根 package）+ [workspace] members = ["python"]；
│                       [workspace.package].version 是唯一的版本字面量
├── Cargo.lock
├── src/                引擎——零依赖，MSRV 1.82，可编译到 wasm32
│   ├── token.rs        分词、结构位置
│   ├── rules.rs        五个 shaping 阶段，一条规则一个函数
│   ├── shaper.rs       变体解析：shape / same_shape / shape_detailed / trace
│   ├── normalize.rs    canonical 规范化算法
│   ├── written_units.rs  书写单元 / 带位置书写单元编码器
│   ├── cli.rs          mongol-norm 命令（src/bin/mongol-norm.rs 只是壳）
│   └── generated/      由 JSON 生成的数据表——请勿手改
├── tests/              crate 的集成测试与共享固件
│   ├── data/           core-hud.tsv、eac-hud.tsv —— 来自 mongfontbuilder
│   └── golden/         mng-canonical-v1.jsonl、mng-phase-trace-v1.json
├── README.md           本文件：GitHub 首页、crates.io README，同时是 doctest
├── LICENSE  NOTICE     crate 自己的许可文件
├── assets/  docs/  .github/workflows/
└── python/             全部 Python 内容
    ├── Cargo.toml      PyO3 绑定 crate mongol-norm-py（需 Rust 1.83；不发布到 crates.io）
    ├── build.rs  src/lib.rs
    ├── pyproject.toml  maturin 后端；版本从 ../Cargo.toml 读取
    ├── README.pypi.md  包在 PyPI 上的长描述
    ├── LICENSE  NOTICE 根目录同名文件的逐字节副本（maturin 不能引用 `..`）
    ├── mongol_norm/    包本体：__init__.py、_api.py（公开 API）、
    │   │               shaper.py（0.0.x 兼容 shim）、_data.py
    │   └── data/       shaping + normalize JSON：表生成器的输入，
    │                   随 wheel 发布供工具脚本使用
    ├── scripts/        gen_rust_tables.py、gen_normalize_table.py、gen_compat_goldens.py、
    │                   preprocess.py、check_dist_metadata.py
    └── tests/          Python 套件；固件从 ../../tests/{data,golden} 读取
```

### 数据与固件

shaping 与 normalize 规则是扁平、语言无关的 JSON，位于 `python/mongol_norm/data/`（`MNG.json`、
`TOD.json`、`SIB.json`、`MCH.json` 与 `MNG.normalize.json`）。运行时不读它：
`python/scripts/gen_rust_tables.py` 把它编译成 `src/generated/` 下的静态 Rust 表，crate 与 wheel 携带的都是
这些表；wheel 仍随包发布 JSON，供工具脚本使用。schema 与消费算法见
[`docs/data-format.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/data-format.md)，其他语言只需
一个 JSON 解析器即可移植。

两套测试读同一批固件，它们只存一份，在 crate 的 `tests/` 下：

| 固件 | 内容 |
|---|---|
| `tests/data/core-hud.tsv` | 177 行 —— mongfontbuilder 的精选回归集（225 个 case） |
| `tests/data/eac-hud.tsv` | 3512 行 —— GB/T 25914-2023（3513 个 case，5 个 UTN-xfail） |
| `tests/golden/mng-canonical-v1.jsonl` | 1991 个 canonical 向量 |
| `tests/golden/mng-phase-trace-v1.json` | 15 个 phase-trace 向量 |

因为语料与 golden 测试读取这个目录，`cargo test` 需要仓库 checkout —— 发布的 crate 不包含固件。

### 运行测试

Rust，在仓库根目录：

```bash
cargo test --workspace --locked   # 256 个测试：单元 + 语料 + golden + 性质 + CLI + fuzz + 本 README
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo package -p mongol-norm      # crates.io 实际收到的内容
```

Python，在 `python/` 下——套件跑的是编译好的扩展，所以先在 virtualenv 里把它构建出来（需要 Rust ≥ 1.83；
`testing` feature 暴露 fallback 测试用的钩子）。任何 Rust 改动之后都要重跑 `maturin develop`：

```bash
cd python
python -m venv .venv && source .venv/bin/activate
pip install 'maturin>=1.15,<2'
maturin develop --locked --features testing        # 构建 mongol_norm/_native

python -m unittest discover -s tests -p 'test_*.py'   # 253 个测试
python -m unittest tests.test_shaper -v               # shape / same_shape / normalize
python -m unittest tests.test_round_trip              # 往返 + 同形同码 + 前缀稳定
python -m unittest tests.test_core_hud tests.test_eac_hud   # 上游 TSV 套件
python -m unittest tests.test_rust_twin               # 生成表新鲜、版本一致
```

生成脚本在仓库根目录运行；每个 `--check` 在 committed 文件与当前应生成结果不一致时失败（三个都在 CI 里跑）：

```bash
python python/scripts/gen_rust_tables.py --check
python python/scripts/gen_normalize_table.py --check
python python/scripts/gen_compat_goldens.py --check
```

当前总数：**256 个 Rust 测试**（单元 + 性质 + 177 条 core-hud 与 3512 条 eac-hud 语料行、1993 个 canonical
与 15 个 phase-trace golden 向量、fuzz，以及本 README 的 doctest）和 **253 个 Python 测试**，在 Rust
stable / 1.82（核心 crate 的 MSRV；绑定 crate 需要 1.83）与 CPython 3.9 – 3.14 上全绿。

### 应用场景

- **搜索与检索** — 为每个可见词形建立唯一索引键
- **文本去重** — 检测编码不同但外形相同的词
- **拼写检查** — 规范化后再查词典
- **语料库语言学** — 一致的词频统计
- **OCR 后处理** — 标准化可能使用不一致编码的 OCR 输出
- **输入法引擎** — 验证和规范化用户输入

### 支持的语种

| Locale | 文字 | 状态 |
|--------|------|------|
| MNG | Hudum（传统蒙文） | ✅ 完整 shaping + 规范化 |
| TOD | Todo（托忒文） | ⬜ shaping 规则已生成，规范化开发中 |
| SIB | Sibe（锡伯文） | ⬜ shaping 规则已生成，规范化开发中 |
| MCH | Manchu（满文） | ⬜ shaping 规则已生成，规范化开发中 |

### 文档

- [docs.rs/mongol-norm](https://docs.rs/mongol-norm) —— 完整的 crate API
- [`docs/data-format.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/data-format.md) —— JSON schema 与 normalize 算法，供其他语言移植
- [`python/README.pypi.md`](https://github.com/Satsrag/mongol-norm/blob/main/python/README.pypi.md) —— Python 包
- [`docs/releasing.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/releasing.md) —— 发版流程
- [`docs/superpowers/specs/2026-09-01-rust-core-design.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/superpowers/specs/2026-09-01-rust-core-design.md) —— 引擎设计与保真约定
- [`docs/superpowers/specs/2026-09-02-python-bindings-design.md`](https://github.com/Satsrag/mongol-norm/blob/main/docs/superpowers/specs/2026-09-02-python-bindings-design.md) —— Python 绑定设计

### 数据来源与致谢

- **[UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html)** — Unicode 技术注释：蒙古文编码与字形化。蒙古文 shaping 规则的权威规范。
- **[mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder)**（Kushim Jiang）— `python/mongol_norm/data/` 内置扁平变体表的来源（从 `data.variants` / `data.particles` 预处理而来），同时也是 vendor 进 `tests/data/` 的 `core-hud.tsv` / `eac-hud.tsv` 回归套件的来源。UTN #57 和 mongfontbuilder 的作者是同一人。
- **[GB/T 25914—2023](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=BD6429DE5A7FC782FAAE13938A07166E)** — 中国国家标准：传统蒙古文名义字符、表现字符和控制字符使用规则；EAC 一致性测试集的来源。
- **[Claude Code](https://claude.ai/code)** — 本项目使用 AI 辅助开发。shaping 规则来源于上述数据；Claude Code 用于实现和组织引擎代码。

### 许可证

MIT License —— 见 [LICENSE](https://github.com/Satsrag/mongol-norm/blob/main/LICENSE)。

整形规则与内置数据派生自 [`mongfontbuilder`](https://github.com/Kushim-Jiang/mongfontbuilder)（MIT）和
UTN #57，其许可证要求的署名保留在
[NOTICE](https://github.com/Satsrag/mongol-norm/blob/main/NOTICE) 中。
