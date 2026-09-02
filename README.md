# mongol-norm

[![Test](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml/badge.svg)](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml)

[English](#english) | [中文](#中文)

---

<a id="english"></a>
## English

### Status

Both halves of the library — shaping and normalization (MNG / Hudum) — are verified against the same upstream corpora. CI runs the full suite on Python 3.9 – 3.13 on every push.

#### ✅ Shaping

`shape()` and `same_shape()` are cross-validated against two upstream TSV suites:

| Suite | Cases | Pass | Notes |
|---|---|---|---|
| `mongfontbuilder/core-hud.tsv` | 225 | **100%** | curated regression set |
| `mongfontbuilder/eac-hud.tsv` (GB/T 25914-2023) | 3513 | **100%** | 5 cases excluded as UTN ↔ EAC xfail, matching mongfontbuilder's own `pytest.mark.xfail` set |
| Hand-written unit tests | — | **100%** | shape / same_shape / joiner tokens (nirugu, ZWJ) |

#### ✅ Normalization — corpus-scoped guarantees, machine-checked

For written-unit shapes covered by the bundled normalization table, `normalize()` / `normalize_text()` / `normalize_written_units()` are **pure functions of shape** with invariants checked in CI over every corpus encoding:

| Property | Result |
|---|---|
| Round-trip — `shape(normalize(x)) == shape(x)` | **3757 / 3757** corpus encodings (100%) |
| Shape-canonicity — same shape ⟹ same Unicode output | **1993 / 1993** shape groups (100%) |
| Prefix-stability — word and word+suffix share their prefix encoding | **2237 / 2237** real corpus pairs (100%) |

Scope note: normalization is implemented for MNG (Hudum) only — Todo / Sibe / Manchu load shaping rules but have no normalizer yet. The guarantees above cover the checked corpus and any input whose written-unit chains can be encoded by the bundled table. For an uncovered out-of-corpus chain, normalization raises `NormalizationFallbackError` by default. Pass `strict=False` explicitly only when returning the original word unchanged is acceptable.

This project was generated with [Claude Code](https://claude.ai/code) (AI-assisted coding). The tests and key parts of the core code have been **manually reviewed**, and test coverage is extensive (corpus round-trip / shape-canonicity / prefix-stability plus the upstream cross-implementation suites). Treat this as a **preview release** — it should be fine for normal use; if you hit a problem, please open an [issue or PR](https://github.com/Satsrag/mongol-norm/issues). Shaping logic is derived from UTN #57 v4 and mongfontbuilder.

---

### Why This Project Exists

Traditional Mongolian script in Unicode has a fundamental problem: **the same visible word can be encoded in multiple different Unicode sequences**. This happens because:

1. **Letters share glyphs** — A and E look identical in medial and final positions; O/U and OE/UE share forms; QA and GA share forms depending on vowel harmony.
2. **Multiple encoding paths** — The same tooth glyph (I) can be encoded as I, YA+FVS1, or even two separate I characters.
3. **Redundant FVS usage** — Free Variation Selectors (FVS1–FVS4) can create equivalent sequences that render identically.
4. **Joining controls** — nirugu (U+180A, the visible stem extender) and ZWJ (U+200D) force letters into their joined forms, and inside those joined contexts even more letters collapse to the same glyph (`nirugu+o` and `nirugu+u` render identically).
5. **Suffix particles after MVS/NNBSP** — the same rendered suffix can be spelled with different letters (`MVS+a` and `MVS+e` both render the chachlag form; `MVS+u` and `MVS+ue` render the same connector).

This means:
- **Search fails**: Searching for "sain" (one encoding) won't find the same word in another encoding, even though they look identical.
- **Deduplication breaks**: The same word has multiple Unicode representations.
- **Indexing is unreliable**: Different encodings of the same word produce different keys.

### What This Project Does

This is a **shaping-aware normalizer** for Traditional Mongolian. It:

1. **Shapes** the input using the full UTN #57 v4 shaping process (5-step conditional mapping)
2. **Compares** glyph sequences to detect identical visual forms
3. **Normalizes** supported shapes to one canonical, FVS-pinned Unicode encoding — same shape ⟹ same Unicode, with an exact shape round-trip

**Example**: All five of these encode the word "sain" (good) and look identical:

![Five encodings of "sain" all normalizing to the same canonical form](https://raw.githubusercontent.com/Satsrag/mongol-norm/main/assets/sain-variants.png)

### How It Works

The normalizer implements a **lightweight Mongolian shaping engine** — equivalent to what HarfBuzz does with a font file, but using only the rule data from [UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) and the [mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) project. No font files needed.

#### Shaping Pipeline (UTN #57 v4 Mongolian-Specific Phase)

1. **Chachlag** — Suffix forms for A/E after MVS (Mongolian Vowel Separator)
2. **Syllabic** — Consonant/vowel context: onset, devsger, marked, masculine/feminine harmony, dotless
3. **Particle** — MVS particle dictionary lookup for specific suffix words
4. **Devsger** — I after a vowel (vowel_devsger) gets double-tooth form: `I → I+I`
5. **Post-bowed** — Vowel forms change after bowed consonants (G, B, K, P, F)

#### Normalization Strategy

Within the normalization table's supported written-unit domain, `normalize` is a **pure function of shape**: any two encodings that shape identically produce the same Unicode output, and the output round-trips — `shape(normalize(x)) == shape(x)`. It is also **prefix-stable**. When these goals conflict the priority is **round-trip > prefix-stable > shortest**.

Per word:

1. **shape** the input into its written-unit sequence. Structural characters — MVS, nirugu, ZWJ — appear verbatim as PascalCase `Mvs` / `Nirugu` / `Zwj` tokens (nirugu renders a visible stem; all three are the evidence for a neighbour's init/medi/fina form). **Split** the shape at these tokens into *chains*; the tokens themselves are copied through unchanged.
2. **encode each chain** (right-to-left, so appending a suffix can't disturb what precedes it):
   1. **partition + table lookup** — the primary path. At each position take the single unit if the table has it (preferred — clean output), else the longest available multi-unit entry, and look up `(position, written-unit) → (letter, FVS)` in an FVS-pinned table. Each value renders its unit **regardless of neighbours**, so the result is a deterministic, O(N), prefix-stable function of the shape.
   2. **velar-feminine refinement** — a `G`/`Gx` velar's forward-coupled vowel (`a`/`o`/`u`) is swapped to its feminine partner (`e`/`oe`/`ue`) for clean output.
   3. **verify** — reshape the candidate in full context; accept only if it equals the target chain shape.
   4. **no search fallback** — the table is total over the corpus (with FVS-first selection there are no gap chains left). If an out-of-corpus shape ever misses the table, normalization raises `NormalizationFallbackError` with the input and uncovered written-unit sequence. Callers may explicitly pass `strict=False` to return the input unchanged (round-trip preserved, never a mis-encoding). A letter next to a joiner simply looks its unit up at the shifted (joined) position.
3. **post-MVS suffix rule** — a chain directly after MVS takes its **standalone** canonical (drop the MVS, normalize, re-attach), so the spelling never depends on MVS. One exception: chachlag `Aa` after MVS is written the bare letter `a`. (The isolate-`I` → `i+FVS1` spelling is pinned in the table itself — no post-processing pass exists.)

**Prefix-stability** means: if word *A* = word *B* + a suffix and their shapes share a prefix, the shared region encodes identically except the single boundary unit whose position changes (final in *B* → medial in *A*). The per-unit table delivers this for free — each unit's encoding depends only on its own position, never on its neighbours.

**How the table is built** (the *selection method*): offline, a **context-independence battery** fills each `(position, written-unit)` slot with the `(letter, FVS)` that renders *exactly* that unit in *every* probed neighbour context (the probes include a bowed consonant, so post-bowed effects can't hide). Candidate order is **letter-major, FVS-first within the letter** — an FVS exists precisely to pin a form against context, so the pinned variant of the right letter always beats its context-sensitive bare form. The result is exported and shipped as JSON; the battery lives in `scripts/gen_normalize_table.py`.

> Note: supported output is **FVS-pinned**, not bare — each unit carries the selector that fixes its form independent of context. This is what makes "same shape ⟹ same Unicode" and prefix-stability hold inside the table's domain. The per-unit table is exported as language-agnostic JSON (`mongol_norm/data/MNG.normalize.json`); schema + consuming algorithm are in [docs/data-format.md](docs/data-format.md), so ports in other languages can implement `normalize` with just a JSON parser.

The exact canonical selection policy is frozen as **`mng-canonical/1`**. It is available as `shaper.canonical_version` and embedded in `MNG.normalize.json`. Applications that persist normalized search/index keys should store this version alongside them and rebuild those keys if a future release changes it.

### Installation

`mongol-norm` is a single self-contained package on [PyPI](https://pypi.org/project/mongol-norm/) — the shaping/normalize data is bundled, no runtime dependencies:

```bash
pip install mongol-norm
```

Or from source:

```bash
git clone https://github.com/Satsrag/mongol-norm.git
cd mongol-norm
pip install .
```

### Usage

```python
from mongol_norm import MongolianShaper, NormalizationFallbackError

shaper = MongolianShaper(locale="MNG")  # Hudum Traditional Mongolian

# Shape: get written-unit sequence
shaper.shape("ᠰᠠᠢᠨ")
# → ['S', 'A', 'I', 'I', 'A']

# Compare: are two encodings visually identical?
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")
# → True

shaper.same_shape("ᠰᠠᠢᠨ", "ᠨᠠᠢ᠍ᠮᠠ")
# → False

# Normalize a supported shape to canonical, FVS-pinned Unicode
shaper.normalize("ᠰᠡᠢᠨ")
# → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'

shaper.normalize("ᠰᠠᠶ᠋ᠢᠨ")
# → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'

shaper.normalize("ᠰᠠᠶ᠋ᠶ᠋ᠨ")
# → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'

# Strict is the default. strict=False preserves the input only if fallback occurs.
shaper.normalize("ᠰᠠᠢᠨ", strict=False)

# Normalize an already-shaped written-unit sequence
shaper.normalize_written_units(["B", "Aa"])
# → 'ᠪᠠ᠋'

# Every written unit is PascalCase, including shape() control output
shaper.normalize_written_units(["S", "A", "I", "I", "N", "Mvs", "Aa"])

# Encode authoritative HUD written-unit positions without inferring controls
shaper.normalize_positioned_written_units([
    {"unit": "B", "position": "init"},
    {"unit": "Aa", "position": "fina"},
])
# → 'ᠪᠠ᠋'

# HUD position is not Unicode topology: isolated FA borrows F:init
shaper.normalize_positioned_written_units([
    {"unit": "F", "position": "init"},
])
# → 'ᠹ' (bare U+1839, no ZWJ); F:isol is unsupported

# O:init reuses the O+A canonical prefix, then adds the required trailing ZWJ
shaper.normalize_positioned_written_units([
    {"unit": "O", "position": "init"},
])
# → U+1824 U+180B U+200D
```

`normalize_written_units()` accepts an ordered `Sequence[str]` of shape units,
not nominal Unicode. Letter positions are inferred from unit order and structural
controls; explicit position records are not accepted by this API. All written-unit
names use PascalCase; structural controls are `Mvs`, `Nirugu`, and `Zwj`, exactly
as returned by `shape()`. Old lowercase or all-uppercase control aliases are not
accepted. The API never infers or inserts a structural control: ZWJ is present in
the output only when `Zwj` is present
in the requested sequence. An empty sequence returns an empty string. A malformed
outer input or a non-string item raises `TypeError`; unknown units and sequences
that cannot reshape exactly raise `ValueError` rather than being guessed or
partially encoded.

`normalize_positioned_written_units()` accepts an ordered sequence of exact
built-in `{"unit": str, "position": str}` dict records. Here `position` is the
position of the written unit in the authoritative HUD inventory, not a Unicode
letter's joining topology. It reuses `normalize_written_units()` rather than a
second encoding table. A complete multi-record chain starts with `init` and ends
with `fina`. An incomplete left or right edge gets an implicit `Zwj`; for example
`B:medi, O:medi, G:fina` is normalized as `Zwj, B, O, G`. A single `init` record
is normally normalized without ZWJ, so `F:init` becomes bare U+1839. The sole
exception is `O:init`: it reuses the U+1824 U+180B prefix selected by canonical
`O:init, A:fina`, then adds U+200D. A
single `medi` gets ZWJ on both sides and a single `fina` gets ZWJ on the left.
`F:isol` is absent from the source inventory and fails closed. `Mvs` and `Nirugu`
use `control`; explicit `Zwj` input is rejected. A wrong outer/record/field type
raises `TypeError`; wrong keys, unit, position, chain positions, exact encoding,
or more than 1024 records raises `ValueError`. This API has no CLI subcommand yet.

#### Full-text normalization

`normalize()` operates on single words. For sentences, paragraphs, or mixed-script text, use `normalize_text()` — it normalizes each Mongolian word independently while preserving spaces, punctuation, and non-Mongolian text verbatim.

```python
# Normalize a sentence (each Mongolian word normalized independently)
shaper.normalize_text("ᠰᠡᠢᠨ ᠨᠠᠢ᠍ᠮᠠ")
# → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌ ᠨᠠᠢ᠍ᠮᠠ᠌'

# Mixed script: non-Mongolian text preserved as-is
shaper.normalize_text("Hello ᠰᠡᠢᠨ world")
# → 'Hello ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌ world'
```

#### Batch normalization example

```python
words = ["ᠰᠡᠢᠨ", "ᠰᠠᠢᠨ", "ᠰᠨ᠌ᠢᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ"]
normalized = [shaper.normalize(w) for w in words]
unique = set(normalized)
print(f"{len(words)} inputs → {len(unique)} unique form(s): {unique}")
# 4 inputs → 1 unique form(s): {'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'}
```

#### Command line

After `pip install mongol-norm`, the `mongol-norm` command is on `PATH` (or run `python -m mongol_norm.shaper ...` without installing).

```bash
# Inline text
mongol-norm shape 'ᠰᠠᠢᠨ'                   # → S+A+I+I+A
mongol-norm normalize 'ᠰᠡᠢᠨ'               # canonical form
mongol-norm normalize --allow-fallback 'ᠰᠡᠢᠨ'  # preserve input if fallback occurs
mongol-norm normalize-written-units 'B+Aa'  # → ᠪᠠ᠋
mongol-norm normalize-written-units 'BZwj'  # compact PascalCase units
mongol-norm normalize-text 'Hello ᠰᠡᠢᠨ'    # mixed script
mongol-norm normalize-text --allow-fallback -i in.txt

# Pipe / stdin (use `-` as the text)
echo 'ᠰᠡᠢᠨ' | mongol-norm normalize -
echo 'B+Aa' | mongol-norm normalize-written-units -
cat doc.txt | mongol-norm normalize-text -

# File in / out
mongol-norm normalize-text -i in.txt -o out.txt

# Batch: one word per line in, one canonical per line out
mongol-norm normalize --batch -i words.txt -o canonical.txt
# Written-unit batch: one compact or '+'-joined sequence per line
mongol-norm normalize-written-units --batch -i units.txt -o canonical.txt

# Visual-identity check (exit 0 if same, 1 if different)
mongol-norm same 'ᠰᠠᠢᠨ' 'ᠰᠡᠢᠨ'
```

`normalize-written-units` accepts compact PascalCase or explicit `+` boundaries.
Compact input must have one unique segmentation; ambiguous input fails closed and
must be rewritten with `+`. After parsing, the same exact-shape validation as
`normalize_written_units()` applies, so a syntactically valid unit stream can
still be rejected when it has no canonical MNG encoding.

`normalize` (single-word) skips non-Mongolian characters, so feeding it a multi-line file treats the whole thing as one word. Use `--batch` for one-word-per-line files, or `normalize-text` for free-form text.

### Rust crate

The same normalizer is also available as a zero-dependency Rust crate, `mongol-norm`, developed
in this repository (`crates/mongol-norm/`). It is a **twin implementation**: same data tables
(generated from `mongol_norm/data/*.json`), same corpus and golden fixtures, byte-identical results on
every value-producing operation (only a few error-message spellings differ — see the crate docs),
output, lockstep version numbers.

Published on [crates.io](https://crates.io/crates/mongol-norm) (API docs on
[docs.rs](https://docs.rs/mongol-norm)); the CLI installs with `cargo install mongol-norm`:

```toml
[dependencies]
mongol-norm = "0.0.4"
```

```rust
use mongol_norm::{Error, Locale, Shaper};

fn main() -> Result<(), Error> {
    let shaper = Shaper::new(Locale::Mng);
    assert_eq!(shaper.shape_str("ᠰᠠᠢᠨ")?, "S+A+I+I+A");
    assert!(shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")?);
    // strict; `normalize_allow_fallback` keeps uncovered input unchanged
    let canonical = shaper.normalize("ᠰᠡᠢᠨ")?;
    let text = shaper.normalize_text("Hello ᠰᠡᠢᠨ world")?;
    println!("{canonical}\n{text}");
    Ok(())
}
```

See [`crates/mongol-norm/README.md`](crates/mongol-norm/README.md) for the full API (written-unit
and positioned written-unit encoders, `trace`, the `mongol-norm` binary).

### Running Tests

```bash
cd mongol-norm

# Shaping + same_shape + normalize unit tests
python -m unittest tests.test_shaper -v

# Normalize properties: round-trip + shape-canonicity + prefix-stability
python -m unittest tests.test_round_trip

# Normalize-table export (compute == load)
python -m unittest tests.test_normalize_table

# mongfontbuilder core-hud (225) + GB/T 25914-2023 eac-hud (3513, 5 UTN-xfail)
python -m unittest tests.test_core_hud tests.test_eac_hud

# Or all together (auto-discovers every tests/test_*.py)
python -m unittest discover -s tests -p 'test_*.py'
```

The hand-written suite covers:

| Test class | What it checks |
|------------|---------------|
| `TestShape` | `shape()` returns correct written-unit sequence (sain variants, the 5 shaping phases step-by-step, UTN-vs-EAC divergences) |
| `TestSameShape` | `same_shape()` correctly identifies visually identical vs. distinct encodings |
| `TestNormalize` | `normalize()` produces canonical output; idempotency; normalized result matches original visually |
| `TestNormalizeText` | `normalize_text()` handles multi-word, mixed-script, punctuation, empty input; idempotency; word independence |
| `TestNormalizeWrittenUnits` | public shape-unit input, structural controls, validation, and exact reshape |
| `TestNormalizePositionedWrittenUnits` | authoritative HUD pair inventory, borrowed-position variants, generation-time exact-position audits, no inserted/explicit ZWJ, and fail-closed records |
| `TestNormalizeWrittenUnitsCli` | inline/stdin/batch CLI input, canonical control spelling, and parser errors |
| `TestNNBSP` | NNBSP ↔ MVS equivalence (UTN model) |

Current totals: **224 Python tests** and **255 Rust tests** (unit + property + 177 core-hud + 3512 eac-hud corpus rows, 1993 canonical + 15 phase-trace golden vectors, fuzz), all green on Python 3.9 – 3.13 and Rust stable / 1.82.

The Rust twin has its own suite (needs a repository checkout for the shared fixtures):

```bash
cargo test --workspace            # unit + integration (corpus, goldens, properties, CLI, fuzz)
python -m unittest tests.test_rust_twin   # generated tables fresh, versions in lockstep
```

### Use Cases

- **Search & Retrieval** — Index Mongolian text with unique keys per visual word
- **Deduplication** — Detect identical words encoded differently
- **Spell Checking** — Normalize before dictionary lookup
- **Corpus Linguistics** — Consistent word frequency counts
- **OCR Post-processing** — Standardize OCR output that may use inconsistent encodings
- **Input Method Engines** — Validate and normalize user input

### Project Structure

```
mongol-norm/                          # the repo = the package (single, self-contained)
├── .github/workflows/test.yml        # CI: Python 3.9-3.13 + the Rust job on every push
├── pyproject.toml
├── Cargo.toml                        # Rust workspace root (lockstep version with pyproject.toml)
├── mongol_norm/
│   ├── shaper.py                     # tokenize / assign_positions / shape / normalize
│   ├── rules.py                      # the 5 shaping phases (iii1..iii5) mirroring iii.py
│   ├── _data.py                      # loaders for the bundled JSON
│   └── data/                         # bundled shaping + normalize data
│       ├── MNG.json  TOD.json  SIB.json  MCH.json
│       └── MNG.normalize.json        # per-unit normalize table
├── crates/mongol-norm/               # the Rust twin: src/ (generated/ = tables from the JSON), tests/
├── scripts/                          # dev-only generators (preprocess, gen_normalize_table)
│   └── gen_rust_tables.py            # JSON → crates/mongol-norm/src/generated/*.rs (--check in CI)
├── docs/data-format.md               # JSON schema, for other-language ports
└── tests/
    ├── test_shaper.py  test_round_trip.py  test_normalize_table.py
    ├── test_written_units_api.py
    ├── test_core_hud.py  test_eac_hud.py
    ├── test_rust_twin.py
    └── data/{core,eac}-hud.tsv       # vendored from mongfontbuilder
```

`mongol-norm` has **no runtime dependencies** — the shaping/normalize JSON is bundled in `mongol_norm/data/`. Install with `pip install mongol-norm`.

### Data Sources & Acknowledgments

- **[UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html)** — Unicode Technical Note: Encoding and Shaping of the Mongolian Script. The authoritative specification for Mongolian shaping rules.
- **[mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder)** by Kushim Jiang — Source for the bundled flat variant tables in `mongol_norm/data/` (preprocessed from `data.variants` / `data.particles`) and for the `core-hud.tsv` / `eac-hud.tsv` regression suites we vendor into `tests/data/`. Both UTN #57 and mongfontbuilder are authored by the same person.
- **[GB/T 25914—2023](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=BD6429DE5A7FC782FAAE13938A07166E)** — China national standard for Traditional Mongolian nominal characters; source of the EAC compliance test set.
- **[Claude Code](https://claude.ai/code)** — This project was developed with AI assistance. The shaping rules are derived from the above sources; Claude Code was used to implement and structure the engine.

### Supported Locales

| Locale | Script | Status |
|--------|--------|--------|
| MNG | Hudum (Traditional Mongolian) | ✅ Full shaping + normalization |
| TOD | Todo | ⬜ Shaping rules generated, normalization WIP |
| SIB | Sibe | ⬜ Shaping rules generated, normalization WIP |
| MCH | Manchu | ⬜ Shaping rules generated, normalization WIP |

### Requirements

- Python 3.7+ (CI-tested on 3.9 / 3.10 / 3.11 / 3.12 / 3.13)
- No runtime dependencies (shaping/normalize data is bundled)

### License

MIT License — see [LICENSE](LICENSE).

The shaping rules and bundled data are derived from [`mongfontbuilder`](https://github.com/Kushim-Jiang/mongfontbuilder) (MIT) and UTN #57. Their required notices are retained in [NOTICE](NOTICE).

---

<a id="中文"></a>
## 中文

### 状态

库的两部分 —— 整形与规范化(MNG / Hudum)—— 都对照同一批上游语料验证。CI 在每次 push 上对 Python 3.9 – 3.13 跑完整套件。

#### ✅ Shaping(整形)

`shape()` 和 `same_shape()` 对照两套上游 TSV 套件交叉验证:

| 套件 | 用例数 | 通过 | 说明 |
|---|---|---|---|
| `mongfontbuilder/core-hud.tsv` | 225 | **100%** | 精选回归集 |
| `mongfontbuilder/eac-hud.tsv` (GB/T 25914-2023) | 3513 | **100%** | 5 个 UTN ↔ EAC 分歧 case 跳过(跟 mongfontbuilder 自己的 `pytest.mark.xfail` 列表一致) |
| 手写单元测试 | — | **100%** | shape / same_shape / joiner token(nirugu、ZWJ) |

#### ✅ Normalization(规范化)— 语料域保证,机器验证

对于内置规范化表覆盖的written-unit shape，`normalize()` / `normalize_text()` / `normalize_written_units()` 是 **shape 的纯函数**,以下不变量在 CI 中对每一条语料编码逐一验证:

| 性质 | 结果 |
|---|---|
| 往返 —— `shape(normalize(x)) == shape(x)` | **3757 / 3757** 语料编码(100%) |
| 同形同码 —— shape 相同 ⟹ 输出 Unicode 相同 | **1993 / 1993** shape 组(100%) |
| 前缀稳定 —— 词与词+后缀共享前缀编码 | **2237 / 2237** 真实语料词对(100%) |

范围说明:规范化目前只实现了 MNG(Hudum)—— Todo / 锡伯文 / 满文已加载 shaping 规则,尚无规范化。上述保证覆盖已检查语料及内置表可编码的written-unit chain。语料外chain若未被覆盖，默认抛出`NormalizationFallbackError`；只有调用者明确接受原样回退时才传入`strict=False`。

本项目由 [Claude Code](https://claude.ai/code)(AI 辅助编码)生成;测试与部分核心代码经**人工审核**,测试覆盖比较充分(语料往返 / 同形同码 / 前缀稳定 + 上游跨实现套件)。当前为**预览版**,正常使用应无问题;遇到问题欢迎提 [issue 和 PR](https://github.com/Satsrag/mongol-norm/issues)。Shaping 逻辑源自 UTN #57 v4 和 mongfontbuilder。

---

### 为什么做这个项目

传统蒙古文在 Unicode 中存在一个根本性问题：**同一个可见词形可以用多种不同的 Unicode 序列编码**。原因是：

1. **字母共享字形** — A 和 E 在中间和尾部位置外形完全相同；O/U、OE/UE 共享形态；QA 和 GA 根据元音和谐共享形态。
2. **多种编码路径** — 同一个齿形字形可以编码为 I、YA+FVS1，甚至两个独立的 I 字符。
3. **冗余的 FVS 使用** — 自由变体选择符（FVS1–FVS4）可以创建渲染结果完全相同的等价序列。
4. **连接控制符** — nirugu(U+180A,可见的连笔延长符)和 ZWJ(U+200D)会强制字母取连接形,而连接语境下更多字母塌缩成同一字形(`nirugu+o` 和 `nirugu+u` 渲染完全相同)。
5. **MVS/NNBSP 后的后缀词** — 同一个渲染出的后缀可以用不同字母拼写(`MVS+a` 和 `MVS+e` 都渲染 chachlag 形;`MVS+u` 和 `MVS+ue` 同形)。

这意味着：
- **搜索失效**：搜索同一个词的某种编码，找不到另一种编码，尽管它们外形完全一样。
- **去重失败**：同一个词有多种 Unicode 表示。
- **索引不可靠**：同一个词的不同编码产生不同的索引键。

### 这个项目做什么

这是一个**形态感知的蒙古文规范化器**。它：

1. 使用完整的 UTN #57 v4 shaping 过程（5 步条件映射）对输入进行**字形化**
2. 通过比较字形序列来**检测**视觉上相同的词形
3. 将受支持的shape**规范化**为唯一的、FVS 钉死的 canonical Unicode 编码 —— 同 shape ⟹ 同 Unicode,并精确往返还原

**示例**：以下五种编码都表示 "sain"（好的），外形完全相同：

![五种 sain 编码全部规范化为同一个标准形式](https://raw.githubusercontent.com/Satsrag/mongol-norm/main/assets/sain-variants.png)

### 工作原理

本规范化器实现了一个**轻量级蒙古文 shaping 引擎**——功能相当于 HarfBuzz 配合字体文件所做的事情，但仅使用 [UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) 的规则数据和 [mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) 项目的变体数据。**不需要字体文件**。

#### Shaping 管线（UTN #57 v4 蒙古文特定阶段）

| 步骤 | 名称 | 说明 |
|------|------|------|
| 1 | Chachlag | MVS（蒙古文元音分隔符）后的 a/e 后缀形态 |
| 2 | Syllabic | 辅音/元音上下文：onset/devsger/marked/阴阳和谐/dotless |
| 3 | Particle | MVS 小品词词典查找 |
| 4 | Devsger | 元音后的 i 获得双齿形态：`I → I+I`（vowel_devsger） |
| 5 | Post-bowed | 弓形辅音(G/B/K/P/F)后的元音形态变化 |

#### 规范化策略

在规范化表支持的written-unit域内，`normalize` 是 **shape 的纯函数**:任意两个 shape 相同的编码,normalize 输出相同,且往返成立 —— `shape(normalize(x)) == shape(x)`,同时**前缀稳定**。三者冲突时优先级:**往返 > 前缀稳定 > 最短**。

逐词:

1. **shape** 成书写单元序列。结构字符 —— MVS、nirugu、ZWJ —— 原样输出为PascalCase `Mvs` / `Nirugu` / `Zwj` token(nirugu 是可见的连笔字形;三者都是邻居字母 init/medi/fina 形的依据)。按这些 token **切成 chain**,token 本身原样拷贝。
2. **逐 chain 编码**(从右往左,这样加后缀不影响前面):
   1. **划分 + 查表**(主路径):每个位置优先取单单元(输出干净),否则取最长多单元,查 `(位置, 书写单元) → (字母, FVS)` 的 FVS 钉死表。每个值**不依赖邻居**就渲染出该单元 → 确定性、O(N)、前缀稳定。
   2. **velar 阴性微调**:`G`/`Gx` 前向耦合的元音(`a`/`o`/`u`)换成阴性(`e`/`oe`/`ue`),输出更干净。
   3. **校验**:在完整上下文里重新 shape,只接受与目标 chain shape 一致的结果。
   4. **没有搜索兜底**:FVS 优先的选择下,表对全部语料 chain 是完备的(缺口为零)。语料外的 shape 万一查不到表，默认抛出包含原输入与未覆盖written-unit序列的`NormalizationFallbackError`。只有明确接受原样回退时才传入`strict=False`(保住往返,绝不错编)。紧邻 joiner 的字母只是按移动后的连接位置查表。
3. **MVS 后缀规则**:紧跟 MVS 的 chain 用其 **standalone** canonical(去掉 MVS、归一、再拼回),拼写不依赖 MVS。唯一例外:MVS 后的 chachlag `Aa` 写裸字母 `a`。(孤立 `I` → `i+FVS1` 的拼写已钉进表本身 —— 不存在后处理。)

**前缀稳定**的含义:若词 *A* = 词 *B* + 后缀,且二者 shape 共享前缀,则共享部分编码完全一致,只有那个位置发生变化的边界单元不同(在 *B* 里是词尾、在 *A* 里变词中)。逐单元表天然保证这点 —— 每个单元的编码只取决于它自己的位置,与邻居无关。

**表是怎么来的**(*选择方法*):离线跑一个 **context 无关性电池** —— 对每个 `(位置, 书写单元)`,挑出在**所有**探测邻居上下文里都**恰好**渲染出该单元的 `(字母, FVS)`(探针含弓形辅音,post-bowed 效应藏不住)。候选顺序是**字母优先、字母内 FVS 优先** —— FVS 的意义就是把字形从 context 里隔离出来,所以正确字母的钉死形永远优于其受感染的裸形。结果导出成 JSON 随包发布;电池在 `scripts/gen_normalize_table.py`。

> 注意:受支持输出是 **FVS 钉死**而非 bare —— 每个单元都带着把字形固定住、不受上下文影响的选择符,这正是"同 shape ⟹ 同 Unicode"和前缀稳定在表内成立的原因。逐单元表导出为语言无关的 JSON(`mongol_norm/data/MNG.normalize.json`),schema 与消费算法见 [docs/data-format.md](docs/data-format.md);其他语言只需一个 JSON 解析器即可实现 normalize。

当前精确 canonical 选择策略冻结为 **`mng-canonical/1`**。可通过 `shaper.canonical_version` 读取，并写入 `MNG.normalize.json`。持久化规范化搜索键/索引键的应用应同时保存该版本；未来版本若发生变化，应重建这些键。

### 安装

`mongol-norm` 是单一自包含包,已发布到 [PyPI](https://pypi.org/project/mongol-norm/) —— shaping/normalize 数据内置,零运行时依赖:

```bash
pip install mongol-norm
```

或从源码安装:

```bash
git clone https://github.com/Satsrag/mongol-norm.git
cd mongol-norm
pip install .
```

### 使用方法

```python
from mongol_norm import MongolianShaper, NormalizationFallbackError

shaper = MongolianShaper(locale="MNG")  # Hudum 传统蒙文

# 字形化：获取书写单元序列
shaper.shape("ᠰᠠᠢᠨ")
# → ['S', 'A', 'I', 'I', 'A']

# 比较：两个编码视觉上是否相同？
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")
# → True

shaper.same_shape("ᠰᠠᠢᠨ", "ᠨᠠᠢ᠍ᠮᠠ")
# → False

# 将受支持shape规范化为唯一的FVS钉死canonical Unicode
shaper.normalize("ᠰᠡᠢᠨ")
# → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'

shaper.normalize("ᠰᠠᠶ᠋ᠢᠨ")
# → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'

shaper.normalize("ᠰᠠᠶ᠋ᠶ᠋ᠨ")
# → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'

# strict是默认行为；strict=False只在触发fallback时原样返回
shaper.normalize("ᠰᠠᠢᠨ", strict=False)

# 从已经shape好的书写单元序列直接生成canonical Unicode
shaper.normalize_written_units(["B", "Aa"])
# → 'ᠪᠠ᠋'

# 所有书写单元均为PascalCase，包括shape()输出的结构control
shaper.normalize_written_units(["S", "A", "I", "I", "N", "Mvs", "Aa"])

# 按权威HUD written-unit position编码，不推断或插入control
shaper.normalize_positioned_written_units([
    {"unit": "B", "position": "init"},
    {"unit": "Aa", "position": "fina"},
])
# → 'ᠪᠠ᠋'

# HUD position不是Unicode topology：FA的isolated variant借用F:init
shaper.normalize_positioned_written_units([
    {"unit": "F", "position": "init"},
])
# → 'ᠹ'（裸U+1839，不含ZWJ）；F:isol不受支持

# O:init复用O+A canonical前缀，再添加所需的尾部ZWJ
shaper.normalize_positioned_written_units([
    {"unit": "O", "position": "init"},
])
# → U+1824 U+180B U+200D
```

`normalize_written_units()`接受由shape unit组成的有序`Sequence[str]`，而不是
nominal Unicode。字母位置由单元顺序与结构control推导；此API不接受显式
position record。所有written-unit名称统一使用PascalCase；结构control为
`Mvs`、`Nirugu`、`Zwj`，与`shape()`输出完全一致。旧小写或全大写control别名
不再接受。API绝不自行推断或插入结构control：只有显式包含`Zwj`时，输出才会
包含ZWJ。空序列返回空字符串。非法外层输入
或非字符串单元抛出`TypeError`；未知unit或无法精确重新shape的序列抛出
`ValueError`，不会猜测或返回部分编码结果。

`normalize_positioned_written_units()`接受由严格内建
`{"unit": str, "position": str}` dict record组成的有序序列。这里的`position`
表示权威HUD inventory中的written-unit position，不是Unicode字母在当前序列中的
joining topology。它直接复用`normalize_written_units()`，不再维护第二套编码表。
完整复合链必须以`init`开头、以`fina`结束；左端或右端不完整时自动补`Zwj`。
例如`B:medi, O:medi, G:fina`会按`Zwj, B, O, G`规范化。单个`init`通常不补
ZWJ，因此`F:init`输出裸`U+1839`；唯一特例`O:init`复用canonical
`O:init, A:fina`选出的`U+1824 U+180B`前缀，再添加`U+200D`。单个`medi`
前后补ZWJ，单个`fina`只在左侧补ZWJ。
inventory中不存在的`F:isol`会fail closed。`Mvs`与`Nirugu`使用
`control`，显式`Zwj`输入被拒绝。外层/record/字段类型错误抛出`TypeError`；
keys、unit、position、复合链位置、exact encoding错误以及超过1024条record均
抛出`ValueError`。本API暂不增加CLI命令。

#### 全文规范化

`normalize()` 作用于单个词。对于句子、段落或混合文字文本，使用 `normalize_text()` ——它独立规范化每个蒙古文词，同时原样保留空格、标点和非蒙古文文本。

```python
# 规范化句子（每个蒙古文词独立规范化）
shaper.normalize_text("ᠰᠡᠢᠨ ᠨᠠᠢ᠍ᠮᠠ")
# → 'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌ ᠨᠠᠢ᠍ᠮᠠ᠌'

# 混合文字：非蒙古文文本原样保留
shaper.normalize_text("Hello ᠰᠡᠢᠨ world")
# → 'Hello ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌ world'
```

#### 批量规范化示例

```python
words = ["ᠰᠡᠢᠨ", "ᠰᠠᠢᠨ", "ᠰᠨ᠌ᠢᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ"]
normalized = [shaper.normalize(w) for w in words]
unique = set(normalized)
print(f"{len(words)} 个输入 → {len(unique)} 个唯一形态：{unique}")
# 4 个输入 → 1 个唯一形态：{'ᠰᠠᠢ᠍ᠢ᠍ᠠ᠌'}
```

#### 命令行

`pip install mongol-norm` 之后,`mongol-norm` 命令即在 `PATH` 上(不安装也可用 `python -m mongol_norm.shaper ...`)。

```bash
# 直接传文本
mongol-norm shape 'ᠰᠠᠢᠨ'                   # → S+A+I+I+A
mongol-norm normalize 'ᠰᠡᠢᠨ'               # 输出 canonical
mongol-norm normalize --allow-fallback 'ᠰᠡᠢᠨ'  # 若触发fallback则原样返回
mongol-norm normalize-written-units 'B+Aa'  # → ᠪᠠ᠋
mongol-norm normalize-written-units 'BZwj'  # 紧凑PascalCase单元串
mongol-norm normalize-text 'Hello ᠰᠡᠢᠨ'    # 混合文字
mongol-norm normalize-text --allow-fallback -i in.txt

# 管道 / 标准输入(文本位置写 `-`)
echo 'ᠰᠡᠢᠨ' | mongol-norm normalize -
echo 'B+Aa' | mongol-norm normalize-written-units -
cat doc.txt | mongol-norm normalize-text -

# 文件输入 / 输出
mongol-norm normalize-text -i in.txt -o out.txt

# 批量:一行一词输入,一行一个 canonical 输出
mongol-norm normalize --batch -i words.txt -o canonical.txt
# 书写单元批量:每行一个紧凑或`+`分隔序列
mongol-norm normalize-written-units --batch -i units.txt -o canonical.txt

# 视觉等价检查(相同退出码 0,不同退出码 1)
mongol-norm same 'ᠰᠠᠢᠨ' 'ᠰᠡᠢᠨ'
```

`normalize-written-units`接受紧凑PascalCase或显式`+`边界。紧凑输入必须只有
一种合法切分；存在歧义时fail closed，调用方须改用`+`。解析后继续执行与
`normalize_written_units()`相同的exact-shape校验，因此语法合法的unit stream
若没有canonical MNG编码仍会被拒绝。

`normalize`(单词模式)会跳过非蒙古文字符,多行文件直接喂给它会被当成一整个词;一行一词的文件请用 `--batch`,自由文本请用 `normalize-text`。

### Rust crate（Rust 实现）

同一个规范化器还有一个零依赖的 Rust crate `mongol-norm`，就在本仓库的 `crates/mongol-norm/` 下开发。
它是一份**双实现**：相同的数据表（由 `mongol_norm/data/*.json` 生成）、相同的语料与 golden 固件、
所有产出值的操作逐字节相同的输出（只有个别错误信息的拼写不同，见 crate 文档）、锁步的版本号。

已发布到 [crates.io](https://crates.io/crates/mongol-norm)（API 文档见
[docs.rs](https://docs.rs/mongol-norm)）；命令行工具用 `cargo install mongol-norm` 安装：

```toml
[dependencies]
mongol-norm = "0.0.4"
```

```rust
use mongol_norm::{Error, Locale, Shaper};

fn main() -> Result<(), Error> {
    let shaper = Shaper::new(Locale::Mng);
    assert_eq!(shaper.shape_str("ᠰᠠᠢᠨ")?, "S+A+I+I+A");
    assert!(shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")?);
    // 严格模式；`normalize_allow_fallback` 会原样保留未覆盖的输入
    let canonical = shaper.normalize("ᠰᠡᠢᠨ")?;
    let text = shaper.normalize_text("Hello ᠰᠡᠢᠨ world")?;
    println!("{canonical}\n{text}");
    Ok(())
}
```

完整 API（书写单元 / 带位置书写单元编码、`trace`、`mongol-norm` 命令行）见
[`crates/mongol-norm/README.md`](crates/mongol-norm/README.md)。

### 运行测试

```bash
cd mongol-norm

# 手写 shaper / same_shape / normalize 测试
python -m unittest tests.test_shaper -v

# normalize 性质:往返 + 同 shape 同输出 + 前缀稳定
python -m unittest tests.test_round_trip

# normalize 表导出(compute == load)
python -m unittest tests.test_normalize_table

# mongfontbuilder core-hud(225)+ GB/T 25914-2023 eac-hud(3513,5 个 UTN-xfail)
python -m unittest tests.test_core_hud tests.test_eac_hud

# 或一次跑全部(自动发现所有 tests/test_*.py)
python -m unittest discover -s tests -p 'test_*.py'
```

手写套件覆盖范围:

| 测试类 | 测试内容 |
|--------|---------|
| `TestShape` | `shape()` 输出正确的书写单元序列(sain 变体、5 步 shaping 分步测试、UTN-vs-EAC 分歧) |
| `TestSameShape` | `same_shape()` 正确识别外形相同 vs 不同的编码 |
| `TestNormalize` | `normalize()` 输出规范结果; 幂等性; 规范化后与原始词形视觉相同 |
| `TestNormalizeText` | `normalize_text()` 处理多词、混合文字、标点、空输入; 幂等性; 词独立性 |
| `TestNormalizeWrittenUnits` | 公开shape-unit输入、结构control、校验与精确回形 |
| `TestNormalizePositionedWrittenUnits` | 权威HUD pair inventory、borrowed-position variant、生成期精确position审计、禁止插入/输入ZWJ及fail-closed record |
| `TestNormalizeWrittenUnitsCli` | inline/stdin/batch CLI输入、control标准拼写与解析错误 |
| `TestNNBSP` | NNBSP ↔ MVS 等价性(UTN 模型) |

当前总数: **224 个 Python 测试** 和 **255 个 Rust 测试**(单元 + 性质 + 177 条 core-hud + 3512 条 eac-hud 语料行、1993 个 canonical + 15 个 phase-trace golden 向量、fuzz), 在 Python 3.9 – 3.13 与 Rust stable / 1.82 上全绿。

Rust 版有自己的测试套件（共享固件需要仓库 checkout）：

```bash
cargo test --workspace            # 单元 + 集成（语料、golden、性质、CLI、fuzz）
python -m unittest tests.test_rust_twin   # 生成表新鲜、版本锁步
```

### 应用场景

- **搜索与检索** — 为每个可见词形建立唯一索引键
- **文本去重** — 检测编码不同但外形相同的词
- **拼写检查** — 规范化后再查词典
- **语料库语言学** — 一致的词频统计
- **OCR 后处理** — 标准化可能使用不一致编码的 OCR 输出
- **输入法引擎** — 验证和规范化用户输入

### 项目结构

```
mongol-norm/                          # 仓库 = 包(单一自包含)
├── .github/workflows/test.yml        # CI: 每次 push 跑 Python 3.9-3.13 + Rust job
├── pyproject.toml
├── Cargo.toml                        # Rust 工作区根（版本与 pyproject.toml 锁步）
├── mongol_norm/
│   ├── shaper.py                     # tokenize / assign_positions / shape / normalize
│   ├── rules.py                      # 5 步 shaping 阶段 (iii1..iii5) 镜像 iii.py
│   ├── _data.py                      # 内置 JSON 的加载器
│   └── data/                         # 内置 shaping + normalize 数据
│       ├── MNG.json  TOD.json  SIB.json  MCH.json
│       └── MNG.normalize.json        # 逐单元 normalize 表
├── crates/mongol-norm/               # Rust 双实现：src/（generated/ = 由 JSON 生成的表）、tests/
├── scripts/                          # 仅开发用的生成脚本 (preprocess, gen_normalize_table)
│   └── gen_rust_tables.py            # JSON → crates/mongol-norm/src/generated/*.rs（CI 跑 --check）
├── docs/data-format.md               # JSON schema, 供其他语言移植
└── tests/
    ├── test_shaper.py  test_round_trip.py  test_normalize_table.py
    ├── test_written_units_api.py
    ├── test_core_hud.py  test_eac_hud.py
    ├── test_rust_twin.py
    └── data/{core,eac}-hud.tsv       # 来自 mongfontbuilder
```

`mongol-norm` **没有运行时依赖** —— shaping/normalize JSON 内置在 `mongol_norm/data/`。安装:`pip install mongol-norm`。

### 数据来源与致谢

- **[UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html)** — Unicode 技术注释：蒙古文编码与字形化。蒙古文 shaping 规则的权威规范。
- **[mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder)**(Kushim Jiang)— `mongol_norm/data/` 内置扁平变体表的来源(从 `data.variants` / `data.particles` 预处理而来), 同时也是我们 vendor 进 `tests/data/` 的 `core-hud.tsv` / `eac-hud.tsv` 回归套件的来源。UTN #57 和 mongfontbuilder 的作者是同一人。
- **[GB/T 25914—2023](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=BD6429DE5A7FC782FAAE13938A07166E)** — 中国国家标准：传统蒙古文名义字符、表现字符和控制字符使用规则; EAC 一致性测试集的来源。
- **[Claude Code](https://claude.ai/code)** — 本项目使用 AI 辅助开发。shaping 规则来源于上述数据；Claude Code 用于实现和组织引擎代码。

### 支持的语种

| Locale | 文字 | 状态 |
|--------|------|------|
| MNG | Hudum（传统蒙文） | ✅ 完整 shaping + 规范化 |
| TOD | Todo（托忒文） | ⬜ shaping 规则已生成，规范化开发中 |
| SIB | Sibe（锡伯文） | ⬜ shaping 规则已生成，规范化开发中 |
| MCH | Manchu（满文） | ⬜ shaping 规则已生成，规范化开发中 |

### 环境要求

- Python 3.7+(CI 实测矩阵: 3.9 / 3.10 / 3.11 / 3.12 / 3.13)
- 无运行时依赖(shaping/normalize 数据已内置)

### 许可证

MIT License —— 见 [LICENSE](LICENSE)。

整形规则与内置数据派生自 [`mongfontbuilder`](https://github.com/Kushim-Jiang/mongfontbuilder)(MIT)和 UTN #57,其许可证要求的署名保留在 [NOTICE](NOTICE) 中。