# Online normalize encoder — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

> **Deviations from the plan template (user instruction 2026-09-23, "代码里实现吧 不用提交 弄完我看一下"):**
> no commit steps — everything stays uncommitted in the working tree for review; the plan fixes the
> interfaces, data format and checks instead of pasting every line of code, because it is executed
> inline right away by its author. Execution notes are appended at the end.

**Goal:** replace the FVS-pinned `mng-canonical/2` normalizer with the online prefix-committed encoder
of `docs/superpowers/specs/2026-09-23-online-normalize-design.md` (`mng-canonical/3`): prefix-stable
by construction, ~22% shorter, ~2× faster, no re-shape in `normalize`.

**Architecture:** an offline Rust generator (`examples/gen_normalize_table/`, public API only)
explores the encoder's abstract states with `shape_detailed` as the oracle and writes
`MNG.normalize.json` (schema 2); `gen_rust_tables.py` compiles it into `src/generated/`; the runtime
(`src/normalize.rs` + `src/encoder.rs`) is a left-to-right planner over those tables.

**Tech stack:** Rust (edition 2021, MSRV 1.82, zero dependencies, `std::thread::scope` for the
generator), Python 3 scripts (table compiler, goldens), PyO3 bindings unchanged.

---

## File structure

| file | responsibility |
|---|---|
| `examples/gen_normalize_table/main.rs` | CLI (`--check`, `--out`), orchestration, JSON output |
| `examples/gen_normalize_table/data.rs` | read `MNG.json` → letters/variants/particles (reuses `tests/common/json.rs`) |
| `examples/gen_normalize_table/candidates.rs` | encoding options, public units, cost, preference, context-free flag |
| `examples/gen_normalize_table/context.rs` | abstract state (`Context`) + key packing — mirrors `src/encoder.rs` |
| `examples/gen_normalize_table/explore.rs` | BFS over states, robustness / final probes (the oracle) |
| `examples/gen_normalize_table/compress.rs` | minimal projections, rows |
| `python/mongol_norm/data/MNG.normalize.json` | schema `mongol-normalize-table/2` (generated) |
| `python/scripts/gen_rust_tables.py` | validate + render schema 2 |
| `python/scripts/gen_normalize_table.py` | thin wrapper: `cargo run --release --example gen_normalize_table -- …` |
| `src/tables.rs` | `Candidate`, `Robustness`, `FinalChoice`, `NormalizeData` (schema 2 types) |
| `src/encoder.rs` (new) | `Context`, key packing, the planner, `encode(shape) -> Option<String>` |
| `src/normalize.rs` | `NormalizeTable` (indexes), `normalize*`, fallback, text runs |
| `src/written_units.rs` | uses the new encoder; keeps its re-shape check |
| tests (`tests/*.rs`, `python/tests/*.py`), goldens, docs, CI | updated expectations and checks |

## Interfaces fixed by this plan

**Context** (runtime and generator must agree):

```text
prev            : Option<candidate>      last committed letter (structural tokens skipped)
prev_token      : None|Letter|Mvs|Nirugu|Zwj
mvs_since_prev  : bool                   an MVS came after `prev`
cluster         : Broken|Initial|InitialMedial   consonant run back to an initial consonant
masculine       : bool                   forward masculine marker (nearest vowel back, MVS resets)
particle        : trie node | DEAD       particle-key prefix of the current MVS segment (bare letters only)
promise         : None|Vowel|VowelNotEe|Masc|FemNeut|ConsOrEe   constraint on the next letter
```

Updates. *Letter* `c` (+ promise `p`): cluster ← vowel: Broken; consonant init: Initial; consonant
medi: InitialMedial if cluster ≠ Broken else Broken; other: Broken. masculine ← false for
`e oe ue ee`, true for `a o u` at init/medi, else unchanged. particle ← DEAD if DEAD or `c` has an
FVS, else child(node, alias) or DEAD. prev ← c, prev_token ← Letter, mvs_since_prev ← false,
promise ← p. *MVS*: prev_token ← Mvs, mvs_since_prev ← true, cluster ← Broken, masculine ← false,
particle ← root-after-MVS, promise ← None. *Nirugu/ZWJ*: prev_token ← Nirugu/Zwj, promise ← None.
Positions: the next letter is chain-first iff prev_token ∈ {None, Mvs}; joined-left iff
prev_token ∈ {Nirugu, Zwj}.

**Key packing** (projection = bit set over components, key = fold `key << bits | value` in this order):

| # | component | bits | value |
|---|---|---|---|
| 0 | prev_letter | 6 | 0 none, else cp − 0x1820 + 1 |
| 1 | prev_fvs | 3 | 0 none, 1–4 |
| 2 | prev_position | 3 | 0 none, isol 1, init 2, medi 3, fina 4 |
| 3 | prev_written | 8 | 0 none, else 1 + index into `written_sequences` |
| 4 | prev_token | 3 | none 0, letter 1, mvs 2, nirugu 3, zwj 4 |
| 5 | mvs_since_prev | 1 | |
| 6 | cluster | 1 | 1 iff InitialMedial |
| 7 | masculine | 1 | |
| 8 | particle | 10 | node id, 1023 = DEAD |
| 9 | promise | 3 | 0 none, 1–5 in the order above |

**Robustness masks:** per promise (index 0 = none … 5), a 128-bit set over `WrittenUnit` indices
(`WrittenUnit::ALL` order; the JSON names the bit order in `mask_units`): bit `u` set = the letter may
be committed when the next pending unit (or the closing structural token) is `u`.

**Planner** (`encode`): exactly the sketch of spec §4 — lazy commits, spans longest-first, the
first right-robust option per span (promises tried in the order above for bare letters, only if
the next unit has a letter of that class at medi and at fina), cheapest total wins, ties keep the
first found; a structural token commits the whole tail; the final letter is the first option at
`fina`/`isol` whose final table allows it.

## Tasks

### Task 1: Branch
- [x] `git switch -c feat/online-normalize` (no commits on it).

### Task 2: Generator skeleton — data and candidates
- [x] `examples/gen_normalize_table/{main,data,candidates}.rs`; `#[allow(dead_code)] #[path = "../../tests/common/json.rs"] mod json;`.
- [x] Candidates: named FVS variants (cost 2), bare letters where the variant is the default or carries a
      condition (cost 1), unnamed-FVS forms of each default (cost 2); drop renderings of `Dd`, medial
      `H`/`Hx`, initial `Cr`; public units by position; sort by `(position, units, cost, rank, fvs)`
      with rank = code point except `g` before `h`.
- [x] Context-free = named FVS, or bare/unnamed-FVS where no variant at that position has a condition.
- [x] Check: `cargo run --release --example gen_normalize_table -- --stats` prints candidate counts
      (expect ≈ 430 candidates, ≈ 60 context-sensitive).

### Task 3: Generator — context, oracle, BFS
- [x] `context.rs`: the Context and key packing above.
- [x] `explore.rs`: per state (representative committed text + committed token raws):
      commit robustness for context-sensitive candidates at the state's commit position against
      every next-letter probe (encoder candidates at medi/fina, grouped by first unit, filtered by
      promise) × `X2 ∈ {end, l, MVS, nirugu, ZWJ}`; token-closed robustness against
      `token × X2 (end, MVS, nirugu, ZWJ, every bare letter, a/e/i/o/n + FVS1, g + FVS2) × X3 (end, l, a, MVS, nirugu, i l)`;
      particle completions; never commit bare `g/h` right after `i`; every probe re-checks all
      committed tokens. Finals: every candidate at the state's final position, full check.
      Successors: every committable candidate × promise, and each structural token.
- [x] Level-synchronous BFS with `std::thread::scope` workers.
- [x] Check: `--stats` prints states / probes / seconds.

### Task 4: Generator — compression and JSON
- [x] Per context-sensitive candidate: minimal projection over reachable states; rows `(key, masks[6])`.
- [x] Per final group `(position, units)`: `always` or projection rows `(key, candidate | null)`.
- [x] Emit schema 2 (§ below); `--check` compares with the committed file.

### Task 5: Table compiler
- [x] `gen_rust_tables.py`: validate schema 2, render `Candidate`/`Robustness`/`FinalChoice`/`NormalizeData`,
      masks re-indexed to `WrittenUnit::ALL`.
- [x] Check: `python3 python/scripts/gen_rust_tables.py && cargo build`.

### Task 6: Runtime
- [x] `src/tables.rs` types; `src/encoder.rs` (Context, keys, planner); `src/normalize.rs` rewired
      (`normalize_impl`: shape → encode → `debug_assert!` round trip; failure → fallback error/echo);
      `known_units` unchanged (every written unit of the locale's variants + structural);
      `with_empty_normalize_table` → no candidates.
- [x] Check: `cargo test --release -p mongol-norm` (expected failures only where outputs are pinned).

### Task 7: Rust tests
- [x] Update pinned outputs (`tests/normalize.rs`, `tests/written_units.rs`, `tests/positioned.rs`,
      `tests/joiners.rs`, `tests/cli.rs`, unit tests).
- [x] `tests/round_trip.rs`: prefix pairs over all shapes (structural included), pinned counts.
- [x] New `tests/online_encoder.rs`: every input of ≤ 2 symbols, corpus, 2000-word fuzz — round trip,
      committed-prefix nesting for every prefix, no uncovered reachable shape; worked examples.

### Task 8: Python side
- [x] `gen_normalize_table.py` wrapper; `test_normalize_table.py` checks the bundled schema-2 table;
      pinned outputs in `test_shaper.py` & co.; `maturin develop --features testing`; regenerate goldens
      (`gen_compat_goldens.py`).
- [x] Check: `python -m unittest discover -s tests -p 'test_*.py'` (from `python/`).

### Task 9: Docs and CI
- [x] `docs/internals.md` (normalization strategy), `docs/data-format.md` (schema 2 + algorithm),
      `docs/development.md`, `README.md`, `python/README.pypi.md`; CI: normalize-table freshness in the
      Rust job (`cargo run --release --example gen_normalize_table -- --check`).

### Task 10: Verification
- [x] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
      `cargo test --workspace`, MSRV 1.82 test, wasm32 build, `cargo doc`, `cargo package`;
      Python suite; all `--check` scripts; benchmark vs the old encoder; exhaustive ≤ 3 symbols.

## Schema `mongol-normalize-table/2` (MNG.normalize.json)

```json
{
  "schema": "mongol-normalize-table/2",
  "canonical_version": "mng-canonical/3",
  "locale": "MNG",
  "description": "...",
  "constants": {"MVS": "180E", "NIRUGU": "180A", "ZWJ": "200D", "FVS1": "180B", "FVS2": "180C", "FVS3": "180D", "FVS4": "180F"},
  "context_components": [["prev_letter", 6], ["prev_fvs", 3], ["prev_position", 3], ["prev_written", 8],
                         ["prev_token", 3], ["mvs_since_prev", 1], ["cluster", 1], ["masculine", 1],
                         ["particle", 10], ["promise", 3]],
  "promises": [["vowel", ["a", "e", "ee", "i", "o", "u", "oe", "ue"]], ...],
  "particle_nodes": ["", "b", "b ue", "M:", "M:a", ...],
  "written_sequences": ["A", "A+A", ...],
  "mask_units": ["A", "Aa", ...],
  "candidates": [
    {"letter": "a", "cp": "1820", "fvs": null, "position": "medi", "units": "A", "written": "A", "robust": "always"},
    {"letter": "n", "cp": "1828", "fvs": null, "position": "medi", "units": "N", "written": "N",
     "robust": {"projection": ["prev_letter"], "rows": [["<key hex>", ["<mask hex>", "… ×6"]]]}}
  ],
  "finals": [
    {"position": "fina", "units": "A", "choice": 17},
    {"position": "fina", "units": "G", "choice": {"projection": ["prev_letter", "masculine"], "rows": [["<key hex>", 42], ["<key hex>", null]]}}
  ],
  "positioned_units": [{"unit": "A", "position": "fina"}, ...]
}
```

`robust` is `"always"`, `"never"` or a table; `choice` is a candidate index, or a table whose rows
map to a candidate index or `null` (no final letter).

## Execution notes (2026-09-23)

All ten tasks were executed inline in the working tree; after the user's review the result was
committed as one change and opened as a pull request. Deviations from the plan:

- **Finals are validity masks, not a choice.** `finals[].valid` is a mask over the options of the
  `(position, units)` group (or a context table of masks); the runtime takes the first valid option
  in its own preference order. This let **harmony** leave the generator's context (with harmony as
  a key component the tables grew several-fold): it only orders equally short letters, which the
  runtime does (`e oe ue` before `a o u` after a feminine vowel; the harmony survives an MVS, so a
  suffix follows its stem). The same runtime order puts `n` first for a final `A` right after a
  vowel (`s a i+FVS3 i n`, not `… i a`), for the word's last letter and the last letter before a
  structural token.
- **Context tables are a default plus exception rows** (`{"projection", "default", "rows"}`), the
  projection chosen for the fewest rows; robustness values index a pool `mask_sets` (16 distinct
  six-mask sets). The JSON also carries `known_units`.
- **Generator battery as implemented** (`explore.rs`): next-letter probes are every encoder option
  whose first unit is the next pending unit and which keeps the promise, followed by
  `{end, l, MVS, nirugu, ZWJ}`; token-closed commits are probed with the token followed by every
  encoder option × `{end, l, a, MVS, nirugu, i l}`; particle completions; every token-closed commit
  also checks the public shape up to the token (`chain_end_ok`). The set of next units a state is
  asked about is part of the node identity (`allowed_next`), which keeps the per-context answers
  single-valued. Of the unnamed FVS forms only the lowest per `(letter, position)` is a candidate
  (the rules that read the FVS value never concern an unnamed one); an isolated `I` is never bare
  `j` (the `mng-canonical/2` spelling rule `i+FVS1` kept).
- The CLI flag is `--verbose` (per-level exploration report), not `--stats`.
- `python/tests/test_normalize_table.py` no longer regenerates the table (that needs `cargo` and
  ~15 s per interpreter); it checks the bundled table against the engine. The freshness check runs
  once, in CI's Rust job.
- The canonical golden keeps its file name (`mng-canonical-v1.jsonl`, schema
  `mongol-norm-canonical-golden/1`); its manifest carries `canonical_version: mng-canonical/3`.

Results:

- Generator: 406 candidates (100 context-dependent), 2 705 contexts, 48 963 483 probes, 996
  robustness rows, 893 final rows, ~12 s on 8 cores; `MNG.normalize.json` 95 KB.
- Corpus (1 990 shape groups): **6.62** code points per group, 0.64 FVS (`mng-canonical/2`: 8.46,
  2.25 FVS). 1 656 golden vectors change: 1 562 shorter, 91 the same length, 3 longer (the
  structural-token cases of spec §5).
- Speed (release, single thread, corpus words): `normalize` 0.92–1.00 µs per word, of which
  `shape` 0.58 µs (`mng-canonical/2`: 2.72–2.91 µs).
- Verification: every input of ≤ 3 symbols (5 671 614 inputs, 62 816 distinct shapes) and 20 000
  random words — 0 unencodable, 0 round-trip failures, 0 prefix-nesting violations (scratchpad
  `verify_crate`; `tests/online_encoder.rs` keeps ≤ 2 symbols, 2 000 random words and every prefix of
  every corpus word). An independent Python port written from `docs/data-format.md` alone
  reproduces the engine on 26 637 distinct shapes (corpus, every corpus prefix, all ≤ 2-symbol
  inputs, 20 000 random words) with 0 differences. `cargo fmt`, clippy `-D warnings`,
  `cargo test --workspace` (279, debug: every output re-shaped by `debug_assert!`), MSRV 1.82,
  wasm32, rustdoc `-D warnings`, `cargo package`, the Python suite (262) and the three `--check`
  scripts are green.

Behaviour changes worth reviewing (aliases):

| input | `mng-canonical/2` | `mng-canonical/3` |
|---|---|---|
| ᠰᠠᠢᠨ `s a i n` | `s a i+fvs3 i+fvs3 a+fvs2` | `s a i+fvs3 i n` |
| ᠮᠣᠩᠭᠣᠯ `m o ng g o l` | `m o a g+fvs2 n+fvs1 n+fvs1 o l` | `m o ng n+fvs1 n o l` |
| ᠬᠡᠷ `h e r` | `g+fvs2 e r` | `g e r` |
| `B Aa` (`normalize_written_units`) | `b a+fvs1` | `b a` |
| `A:init` / `I:init` (positioned API) | `a+fvs1 ZWJ` / `i+fvs1 ZWJ` | `e ZWJ` / `j ZWJ` |
| `ZWJ d` | `ZWJ o a+fvs2` | `ZWJ o n` |
| ᠲᠠᠯ᠎ᠠ᠎ᠶᠢᠨ `t a l mvs a mvs y i n` | `t a l mvs a mvs i+fvs1 i+fvs3 a+fvs2` | `t a l mvs a mvs j i n` |
| ᠣᠷᠣᠨ᠎ᠤ `o r o n mvs u` | `a+fvs1 o r o a+fvs2 mvs u+fvs1` | `o r o a mvs u` |
| ᠲᠡᠷᠡ `t e r e` | `t a r a+fvs2` | `t a r a` |
| ᠪᠡᠯᠭᠡ `b e l g e` | `b a l g+fvs2 e+fvs1` | `b a l g e` |

Suffixes after an MVS are no longer spelled as if they stood alone; they follow the stem's harmony
(`tests/round_trip.rs::suffix_spelling_follows_the_stem_harmony`). Words without a harmony clue or
with an ambiguous glyph get the preference order, not the dictionary spelling (ᠲᠡᠷᠡ → `t a r a`,
ᠪᠡᠯᠭᠡ → `b a l g e`, ᠬᠡᠯᠡ → `g e l e`) — spec §10.
