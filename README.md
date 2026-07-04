# mongol-norm

[![Test](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml/badge.svg)](https://github.com/Satsrag/mongol-norm/actions/workflows/test.yml)

[English](#english) | [中文](#中文)

---

<a id="english"></a>
## English

### Status

The two pieces of the library are at very different maturity levels — be aware before you depend on `normalize()`.

#### ✅ Shaping — usable

`shape()` and `same_shape()` (MNG / Hudum) are cross-validated against two upstream TSV suites:

| Suite | Cases | Pass | Notes |
|---|---|---|---|
| `mongfontbuilder/core-hud.tsv` | 177 | **100%** | curated regression set |
| `mongfontbuilder/eac-hud.tsv` (GB/T 25914-2023) | 3512 | **100%** | 5 cases excluded as UTN ↔ EAC xfail, matching mongfontbuilder's own `pytest.mark.xfail` set |
| Hand-written shaper unit tests | 113 | **100%** | shape / same_shape / normalize / normalize_text |

CI runs the full suite on Python 3.9 – 3.13 on every push. Within these constraints — MNG / Hudum locale, the shaping rules covered by UTN #57 v4 and mongfontbuilder — the shaper should be reliable enough to build on.

#### ⚠️ Normalization — NOT reliable yet, work-in-progress

`normalize()` and `normalize_text()` are the **focus of upcoming work**, not the current strength of the library. Today they have:

- only the hand-written round-trip tests (no third-party corpus regression set)
- known mis-canonicalization on words with ambiguous vowel harmony (no unambiguous o/u/oe/ue vowel in the word)
- no implementation at all for Todo / Sibe / Manchu (shaping rules are loaded for those locales, normalization is not)

If you are building on this library now, treat the shape / same-shape APIs as stable and the normalize APIs as **experimental** — they will keep changing as the canonical-form logic is rebuilt against a real corpus.

**Contributions of normalization test data and bug reports are very welcome** — see [Help Wanted](#help-wanted-test-data) below.

This project was developed with [Claude Code](https://claude.ai/code) (AI-assisted coding). Shaping logic is derived from UTN #57 v4 and mongfontbuilder; the implementation is independently tested against the upstream TSV regression suites.

---

### Why This Project Exists

Traditional Mongolian script in Unicode has a fundamental problem: **the same visible word can be encoded in multiple different Unicode sequences**. This happens because:

1. **Letters share glyphs** — A and E look identical in medial and final positions; O and U share forms; QA and GA share forms depending on vowel harmony.
2. **Multiple encoding paths** — The same tooth glyph (I) can be encoded as I, YA+FVS1, or even two separate I characters.

![Mongolian letters A E O U QA GA I rendered in Noto Sans Mongolian](assets/letters.png)
3. **Redundant FVS usage** — Free Variation Selectors (FVS1–FVS4) can create equivalent sequences that render identically.

This means:
- **Search fails**: Searching for "sain" (one encoding) won't find the same word in another encoding, even though they look identical.
- **Deduplication breaks**: The same word has multiple Unicode representations.
- **Indexing is unreliable**: Different encodings of the same word produce different keys.

### What This Project Does

This is a **shaping-aware normalizer** for Traditional Mongolian. It:

1. **Shapes** the input using the full UTN #57 v4 shaping process (5-step conditional mapping)
2. **Compares** glyph sequences to detect identical visual forms
3. **Normalizes** to a canonical, human-readable, bare Unicode encoding

**Example**: All five of these encode the word "sain" (good) and look identical:

![Five encodings of "sain" all normalizing to the same canonical form](assets/sain-variants.png)

### How It Works

The normalizer implements a **lightweight Mongolian shaping engine** — equivalent to what HarfBuzz does with a font file, but using only the rule data from [UTN #57 v4](https://www.unicode.org/notes/tn57/tn57-4.html) and the [mongfontbuilder](https://github.com/Kushim-Jiang/mongfontbuilder) project. No font files needed.

#### Shaping Pipeline (UTN #57 v4 Mongolian-Specific Phase)

1. **Chachlag** — Suffix forms for A/E after MVS (Mongolian Vowel Separator)
2. **Syllabic** — Consonant/vowel context: onset, devsger, marked, masculine/feminine harmony, dotless
3. **Particle** — MVS particle dictionary lookup for specific suffix words
4. **Devsger** — I after a vowel (vowel_devsger) gets double-tooth form: `I → I+I`
5. **Post-bowed** — Vowel forms change after bowed consonants (G, B, K, P, F)

#### Normalization Strategy

`normalize` is a **pure function of shape**: any two encodings that shape identically produce the same Unicode output, and the output always round-trips — `shape(normalize(x)) == shape(x)`. It is also **prefix-stable**. When these goals conflict the priority is **round-trip > prefix-stable > shortest**.

Per word:

1. **shape** the input into its written-unit sequence. Structural characters — MVS, nirugu, ZWJ — appear verbatim as `mvs` / `nirugu` / `zwj` tokens (nirugu renders a visible stem; all three are the evidence for a neighbour's init/medi/fina form). **Split** the shape at these tokens into *chains*; the tokens themselves are copied through unchanged.
2. **encode each chain** (right-to-left, so appending a suffix can't disturb what precedes it):
   1. **partition + table lookup** — the primary path. At each position take the single unit if the table has it (preferred — clean output), else the longest available multi-unit entry, and look up `(position, written-unit) → (letter, FVS)` in an FVS-pinned table. Each value renders its unit **regardless of neighbours**, so the result is a deterministic, O(N), prefix-stable function of the shape.
   2. **velar-feminine refinement** — a `G`/`Gx` velar's forward-coupled vowel (`a`/`o`/`u`) is swapped to its feminine partner (`e`/`oe`/`ue`) for clean output.
   3. **verify** — reshape the candidate in full context; accept only if it equals the target chain shape.
   4. **gap chains** — a handful of chains can't be expressed by table entries alone (bowed-consonant + following-vowel forms like `B+A`; `Sh+I` clusters). These fall back to an exhaustive search over letter partitions and FVS combinations (~13 corpus chains). A letter next to a joiner simply looks its unit up at the shifted (joined) position — no wrapping tricks needed.
3. **post-MVS suffix rule** — a chain directly after MVS takes its **standalone** canonical (drop the MVS, normalize, re-attach), so the spelling never depends on MVS. One exception: chachlag `Aa` after MVS is written the bare letter `a`. (The isolate-`I` → `i+FVS1` spelling is pinned in the table itself — no post-processing pass exists.)

**Prefix-stability** means: if word *A* = word *B* + a suffix and their shapes share a prefix, the shared region encodes identically except the single boundary unit whose position changes (final in *B* → medial in *A*). The per-unit table delivers this for free — each unit's encoding depends only on its own position, never on its neighbours.

**How the table is built** (the *selection method*): offline, a **context-independence battery** fills each `(position, written-unit)` slot with the `(letter, FVS)` — tried masculine-first, bare-first — that renders *exactly* that unit in *every* probed neighbour context. The result is exported and shipped as JSON; see `MongolianShaper.compute_normalize_tables()`.

> Note: output is **FVS-pinned**, not bare — each unit carries the selector that fixes its form independent of context. This is what makes "same shape ⟹ same Unicode" and prefix-stability hold. The per-unit table is exported as language-agnostic JSON (`mongol_norm/data/MNG.normalize.json`); schema + consuming algorithm are in [docs/data-format.md](docs/data-format.md), so ports in other languages can implement `normalize` with just a JSON parser.

### Installation

`mongol-norm` is a single self-contained package — the shaping/normalize data is bundled, no separate data package or runtime dependency. Not on PyPI yet — install from source:

```bash
git clone https://github.com/Satsrag/mongol-norm.git
cd mongol-norm
pip install .
```

### Usage

```python
from mongol_norm import MongolianShaper

shaper = MongolianShaper(locale="MNG")  # Hudum Traditional Mongolian

# Shape: get written-unit sequence
shaper.shape("ᠰᠠᠢᠨ")
# → ['S', 'A', 'I', 'I', 'A']

# Compare: are two encodings visually identical?
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")
# → True

shaper.same_shape("ᠰᠠᠢᠨ", "ᠨᠠᠢᠮᠠ")
# → False

# Normalize: canonical bare Unicode
shaper.normalize("ᠰᠡᠢᠨ")
# → 'ᠰᠠᠢᠨ'

shaper.normalize("ᠰᠠᠶ᠋ᠢᠨ")
# → 'ᠰᠠᠢᠨ'

shaper.normalize("ᠰᠠᠶ᠋ᠶ᠋ᠨ")
# → 'ᠰᠠᠢᠨ'
```

#### Full-text normalization

`normalize()` operates on single words. For sentences, paragraphs, or mixed-script text, use `normalize_text()` — it normalizes each Mongolian word independently while preserving spaces, punctuation, and non-Mongolian text verbatim.

```python
# Normalize a sentence (each Mongolian word normalized independently)
shaper.normalize_text("ᠰᠡᠢᠨ ᠨᠠᠢᠮᠠ")
# → 'ᠰᠠᠢᠨ ᠨᠠᠢᠮᠠ'

# Mixed script: non-Mongolian text preserved as-is
shaper.normalize_text("Hello ᠰᠡᠢᠨ world")
# → 'Hello ᠰᠠᠢᠨ world'
```

#### Batch normalization example

```python
words = ["ᠰᠡᠢᠨ", "ᠰᠠᠢᠨ", "ᠰᠨ᠌ᠢᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ"]
normalized = [shaper.normalize(w) for w in words]
unique = set(normalized)
print(f"{len(words)} inputs → {len(unique)} unique form(s): {unique}")
# 4 inputs → 1 unique form(s): {'ᠰᠠᠢᠨ'}
```

#### Command line

After `pip install .`, the `mongol-norm` command is on `PATH` (or run `python -m mongol_norm.shaper ...` without installing).

```bash
# Inline text
mongol-norm shape 'ᠰᠠᠢᠨ'                   # → S+A+I+I+A
mongol-norm normalize 'ᠰᠡᠢᠨ'               # canonical form
mongol-norm normalize-text 'Hello ᠰᠡᠢᠨ'    # mixed-script

# Pipe / stdin (use `-` as the text)
echo 'ᠰᠡᠢᠨ' | mongol-norm normalize -
cat doc.txt | mongol-norm normalize-text -

# File in / out
mongol-norm normalize-text -i in.txt -o out.txt

# Batch: one word per line in, one canonical per line out
mongol-norm normalize --batch -i words.txt -o canonical.txt

# Visual-identity check (exit 0 if same, 1 if different)
mongol-norm same 'ᠰᠠᠢᠨ' 'ᠰᠡᠢᠨ'
```

`normalize` (single-word) skips non-Mongolian characters, so feeding it a multi-line file treats the whole thing as one word. Use `--batch` for one-word-per-line files, or `normalize-text` for free-form text.

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
| `TestNNBSP` | NNBSP ↔ MVS equivalence (UTN model) |

Current totals: **113 hand-written + 177 core-hud + 3507 eac-hud = 3797 test cases**, all green on Python 3.9 – 3.13.

### Help Wanted: Test Data

Shaping is now verified against the mongfontbuilder + GB/T 25914 cross-implementation suites (3684/3684 = 100%, with 5 UTN-xfail mirroring mongfontbuilder's own). **What's still missing is a real-world normalization corpus** — a set of (input encoding, expected normalized form) pairs verified by a human expert.

Useful contributions:

- **Word pairs**: Two Unicode sequences that render identically, with the expected canonical form
- **Counterexamples**: A word where the current normalizer produces wrong output (open an issue with the input, actual output, and expected output)
- **Word lists**: Any existing Mongolian wordlist or dictionary with consistent Unicode encoding
- **Rendered images**: A screenshot of a Mongolian word in a correct UTN57-compliant font alongside its Unicode byte sequence — useful for verifying visual identity

If you can contribute, please **open an issue or pull request** at [github.com/Satsrag/mongol-norm](https://github.com/Satsrag/mongol-norm).

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
├── .github/workflows/test.yml        # CI: Python 3.9-3.13 on every push
├── pyproject.toml
├── mongol_norm/
│   ├── shaper.py                     # tokenize / assign_positions / shape / normalize
│   ├── rules.py                      # the 5 shaping phases (iii1..iii5) mirroring iii.py
│   ├── _data.py                      # loaders for the bundled JSON
│   └── data/                         # bundled shaping + normalize data
│       ├── MNG.json  TOD.json  SIB.json  MCH.json
│       └── MNG.normalize.json        # per-unit normalize table
├── scripts/                          # dev-only generators (preprocess, gen_normalize_table)
├── docs/data-format.md               # JSON schema, for other-language ports
└── tests/
    ├── test_shaper.py  test_round_trip.py  test_normalize_table.py
    ├── test_core_hud.py  test_eac_hud.py
    └── data/{core,eac}-hud.tsv       # vendored from mongfontbuilder
```

`mongol-norm` has **no runtime dependencies** — the shaping/normalize JSON is bundled in `mongol_norm/data/`. Not on PyPI yet — install from source.

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

- Python 3.9+ (CI matrix: 3.9 / 3.10 / 3.11 / 3.12 / 3.13)
- No runtime dependencies (shaping/normalize data is bundled)

### License

SIL Open Font License 1.1 (`OFL-1.1`) — consistent with upstream `mongfontbuilder` and UTN #57 sources.

---

<a id="中文"></a>
## 中文

### 状态

库的两个部分成熟度差距很大 — 依赖 `normalize()` 之前请先看清楚。

#### ✅ Shaping(整形)— 可用

`shape()` 和 `same_shape()`(MNG / Hudum)对照两套上游 TSV 套件交叉验证:

| 套件 | 用例数 | 通过 | 说明 |
|---|---|---|---|
| `mongfontbuilder/core-hud.tsv` | 177 | **100%** | 精选回归集 |
| `mongfontbuilder/eac-hud.tsv` (GB/T 25914-2023) | 3512 | **100%** | 5 个 UTN ↔ EAC 分歧 case 跳过(跟 mongfontbuilder 自己的 `pytest.mark.xfail` 列表一致) |
| 手写 shaper 单元测试 | 113 | **100%** | shape / same_shape / normalize / normalize_text |

CI 在每次 push 上对 Python 3.9 – 3.13 跑完整套件。在 MNG / Hudum 语种、UTN #57 v4 和 mongfontbuilder 覆盖的整形规则范围内,shape 这块应该足够可靠拿去落地使用。

#### ⚠️ Normalization(规范化)— 目前不靠谱,后期重点

`normalize()` 和 `normalize_text()` 是**后续更新的重点**,不是目前库的强项。当前状态:

- 只有手写的往返测试,**没有第三方语料库回归集**
- 已知 bug: 元音和谐不明确的词(整个词里没有 o/u/oe/ue 这种不模糊元音)会输出错误的规范形式
- Todo / 锡伯文 / 满文完全没实现规范化(虽然加载了 shaping 规则)

如果你现在要基于本库开发,把 shape / same_shape 当稳定 API,把 normalize 当**实验性 API** — 因为规范化逻辑会基于真实语料重写,持续变动。

**欢迎贡献规范化测试数据和报告问题** — 详见下方[求助:测试数据](#求助测试数据)。

本项目由 [Claude Code](https://claude.ai/code)(AI 辅助编码)开发。Shaping 逻辑源自 UTN #57 v4 和 mongfontbuilder; 实现独立对照上游 TSV 回归套件验证。

---

### 为什么做这个项目

传统蒙古文在 Unicode 中存在一个根本性问题：**同一个可见词形可以用多种不同的 Unicode 序列编码**。原因是：

1. **字母共享字形** — A 和 E 在中间和尾部位置外形完全相同；O 和 U 共享形态；QA 和 GA 根据元音和谐共享形态。
2. **多种编码路径** — 同一个齿形字形可以编码为 I、YA+FVS1，甚至两个独立的 I 字符。

![蒙古文字母 A E O U QA GA I 在 Noto Sans Mongolian 字体下的渲染](assets/letters.png)
3. **冗余的 FVS 使用** — 自由变体选择符（FVS1–FVS4）可以创建渲染结果完全相同的等价序列。

这意味着：
- **搜索失效**：搜索同一个词的某种编码，找不到另一种编码，尽管它们外形完全一样。
- **去重失败**：同一个词有多种 Unicode 表示。
- **索引不可靠**：同一个词的不同编码产生不同的索引键。

### 这个项目做什么

这是一个**形态感知的蒙古文规范化器**。它：

1. 使用完整的 UTN #57 v4 shaping 过程（5 步条件映射）对输入进行**字形化**
2. 通过比较字形序列来**检测**视觉上相同的词形
3. **规范化**为唯一的、人类可读的、bare Unicode 编码

**示例**：以下五种编码都表示 "sain"（好的），外形完全相同：

![五种 sain 编码全部规范化为同一个标准形式](assets/sain-variants.png)

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

`normalize` 是 **shape 的纯函数**:任意两个 shape 相同的编码,normalize 输出相同,且始终往返成立 —— `shape(normalize(x)) == shape(x)`,同时**前缀稳定**。三者冲突时优先级:**往返 > 前缀稳定 > 最短**。

逐词:

1. **shape** 成书写单元序列。结构字符 —— MVS、nirugu、ZWJ —— 原样输出为 `mvs` / `nirugu` / `zwj` token(nirugu 是可见的连笔字形;三者都是邻居字母 init/medi/fina 形的依据)。按这些 token **切成 chain**,token 本身原样拷贝。
2. **逐 chain 编码**(从右往左,这样加后缀不影响前面):
   1. **划分 + 查表**(主路径):每个位置优先取单单元(输出干净),否则取最长多单元,查 `(位置, 书写单元) → (字母, FVS)` 的 FVS 钉死表。每个值**不依赖邻居**就渲染出该单元 → 确定性、O(N)、前缀稳定。
   2. **velar 阴性微调**:`G`/`Gx` 前向耦合的元音(`a`/`o`/`u`)换成阴性(`e`/`oe`/`ue`),输出更干净。
   3. **校验**:在完整上下文里重新 shape,只接受与目标 chain shape 一致的结果。
   4. **缺口 chain**:少数 chain 仅靠表条目表达不了(弓形辅音+后随元音形如 `B+A`;`Sh+I` 簇),回退到对字母划分×FVS 组合的穷举搜索(语料中约 13 个)。紧邻 joiner 的字母只是按移动后的连接位置查表 —— 不再需要任何包裹技巧。
3. **MVS 后缀规则**:紧跟 MVS 的 chain 用其 **standalone** canonical(去掉 MVS、归一、再拼回),拼写不依赖 MVS。唯一例外:MVS 后的 chachlag `Aa` 写裸字母 `a`。(孤立 `I` → `i+FVS1` 的拼写已钉进表本身 —— 不存在后处理。)

**前缀稳定**的含义:若词 *A* = 词 *B* + 后缀,且二者 shape 共享前缀,则共享部分编码完全一致,只有那个位置发生变化的边界单元不同(在 *B* 里是词尾、在 *A* 里变词中)。逐单元表天然保证这点 —— 每个单元的编码只取决于它自己的位置,与邻居无关。

**表是怎么来的**(*选择方法*):离线跑一个 **context 无关性电池** —— 对每个 `(位置, 书写单元)`,按"阳性优先、裸形优先"挑出那个在**所有**探测邻居上下文里都**恰好**渲染出该单元的 `(字母, FVS)`。结果导出成 JSON 随包发布;见 `MongolianShaper.compute_normalize_tables()`。

> 注意:输出是 **FVS 钉死**而非 bare —— 每个单元都带着把字形固定住、不受上下文影响的选择符,这正是"同 shape ⟹ 同 Unicode"和前缀稳定成立的原因。逐单元表导出为语言无关的 JSON(`mongol_norm/data/MNG.normalize.json`),schema 与消费算法见 [docs/data-format.md](docs/data-format.md);其他语言只需一个 JSON 解析器即可实现 normalize。

### 安装

`mongol-norm` 是单一自包含包 —— shaping/normalize 数据已内置,没有独立数据包,也没有运行时依赖。还没发布到 PyPI,从源码本地安装:

```bash
git clone https://github.com/Satsrag/mongol-norm.git
cd mongol-norm
pip install .
```

### 使用方法

```python
from mongol_norm import MongolianShaper

shaper = MongolianShaper(locale="MNG")  # Hudum 传统蒙文

# 字形化：获取书写单元序列
shaper.shape("ᠰᠠᠢᠨ")
# → ['S', 'A', 'I', 'I', 'A']

# 比较：两个编码视觉上是否相同？
shaper.same_shape("ᠰᠠᠢᠨ", "ᠰᠡᠢᠨ")
# → True

shaper.same_shape("ᠰᠠᠢᠨ", "ᠨᠠᠢᠮᠠ")
# → False

# 规范化：输出唯一的 bare Unicode
shaper.normalize("ᠰᠡᠢᠨ")
# → 'ᠰᠠᠢᠨ'

shaper.normalize("ᠰᠠᠶ᠋ᠢᠨ")
# → 'ᠰᠠᠢᠨ'

shaper.normalize("ᠰᠠᠶ᠋ᠶ᠋ᠨ")
# → 'ᠰᠠᠢᠨ'
```

#### 全文规范化

`normalize()` 作用于单个词。对于句子、段落或混合文字文本，使用 `normalize_text()` ——它独立规范化每个蒙古文词，同时原样保留空格、标点和非蒙古文文本。

```python
# 规范化句子（每个蒙古文词独立规范化）
shaper.normalize_text("ᠰᠡᠢᠨ ᠨᠠᠢᠮᠠ")
# → 'ᠰᠠᠢᠨ ᠨᠠᠢᠮᠠ'

# 混合文字：非蒙古文文本原样保留
shaper.normalize_text("Hello ᠰᠡᠢᠨ world")
# → 'Hello ᠰᠠᠢᠨ world'
```

#### 批量规范化示例

```python
words = ["ᠰᠡᠢᠨ", "ᠰᠠᠢᠨ", "ᠰᠨ᠌ᠢᠢᠨ", "ᠰᠠᠶ᠋ᠢᠨ"]
normalized = [shaper.normalize(w) for w in words]
unique = set(normalized)
print(f"{len(words)} 个输入 → {len(unique)} 个唯一形态：{unique}")
# 4 个输入 → 1 个唯一形态：{'ᠰᠠᠢᠨ'}
```

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
| `TestNNBSP` | NNBSP ↔ MVS 等价性(UTN 模型) |

当前总数: **113 手写 + 177 core-hud + 3507 eac-hud = 3797 个测试用例**, 在 Python 3.9 – 3.13 上全绿。

### 求助：测试数据

Shaping 已经对照 mongfontbuilder + GB/T 25914 跨实现套件验证完毕(3684/3684 = 100%, 5 个 UTN-xfail 跟 mongfontbuilder 自己一致)。**目前还缺一个真实世界的规范化语料库**——由人工专家验证的(输入编码, 期望规范形式)对。

欢迎以下形式的贡献：

- **词对**：两个渲染结果相同的 Unicode 序列，及其期望的规范形式
- **反例**：当前规范化器输出错误的词（请提 issue，附上输入、实际输出、期望输出）
- **词表**：任何具有一致 Unicode 编码的蒙古文词表或词典
- **渲染截图**：在正确的 UTN57 兼容字体中渲染的蒙古文词图片，附上对应的 Unicode 字节序列——有助于验证视觉等价性

欢迎在 [github.com/Satsrag/mongol-norm](https://github.com/Satsrag/mongol-norm) **提 issue 或 PR**。

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
├── .github/workflows/test.yml        # CI: 每次 push 跑 Python 3.9-3.13
├── pyproject.toml
├── mongol_norm/
│   ├── shaper.py                     # tokenize / assign_positions / shape / normalize
│   ├── rules.py                      # 5 步 shaping 阶段 (iii1..iii5) 镜像 iii.py
│   ├── _data.py                      # 内置 JSON 的加载器
│   └── data/                         # 内置 shaping + normalize 数据
│       ├── MNG.json  TOD.json  SIB.json  MCH.json
│       └── MNG.normalize.json        # 逐单元 normalize 表
├── scripts/                          # 仅开发用的生成脚本 (preprocess, gen_normalize_table)
├── docs/data-format.md               # JSON schema, 供其他语言移植
└── tests/
    ├── test_shaper.py  test_round_trip.py  test_normalize_table.py
    ├── test_core_hud.py  test_eac_hud.py
    └── data/{core,eac}-hud.tsv       # 来自 mongfontbuilder
```

`mongol-norm` **没有运行时依赖** —— shaping/normalize JSON 内置在 `mongol_norm/data/`。还没上 PyPI, 从源码安装。

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

- Python 3.9+ (CI 矩阵: 3.9 / 3.10 / 3.11 / 3.12 / 3.13)
- 无运行时依赖(shaping/normalize 数据已内置)

### 许可证

SIL Open Font License 1.1 (`OFL-1.1`) — 跟上游 `mongfontbuilder` 和 UTN #57 一致。