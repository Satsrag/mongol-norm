# Releasing mongol-norm · 发布流程

[English](#english) | [中文](#中文)

---

<a id="english"></a>
## English

One GitHub Release (tag `vX.Y.Z`) publishes two artifacts from the same commit:

- the PyPI package `mongol-norm` — wheels and an sdist, `.github/workflows/publish.yml`;
- the crate `mongol-norm` on crates.io — `.github/workflows/publish-crate.yml`.

Both use Trusted Publishing (GitHub OIDC) from a protected GitHub environment; no PyPI or
crates.io token is stored in GitHub.

### The version

The version literal lives in exactly one place: `[workspace.package] version` in the root
`Cargo.toml`. Everything else derives from it — the engine crate (the root package) and
the binding crate use `version.workspace = true`, maturin reads it through
`python/Cargo.toml` (`python/pyproject.toml` declares `dynamic = ["version"]`), and
`mongol_norm.__version__` is read from the extension module at import time.
`python/tests/test_rust_twin.py` and both publish workflows check that the runtime, the
crate and (on a release) the tag agree with it.

To bump it: edit the literal in the root `Cargo.toml`, run `cargo update -w` so
`Cargo.lock` records the new workspace version, run the suites, commit.

### One-time setup (PyPI)

A maintainer of the `mongol-norm` PyPI project must add a trusted publisher with
these exact values:

| Field | Value |
| --- | --- |
| PyPI project | `mongol-norm` |
| GitHub owner | `Satsrag` |
| Repository | `mongol-norm` |
| Workflow | `publish.yml` |
| Environment | `pypi` |

Create a GitHub environment named `pypi` as well. Required reviewers may be
configured on that environment if every publication should require manual
approval.

### Build wheels without publishing

Run **Build and publish to PyPI** from the Actions tab (**Run workflow**, on any branch).
A manual `workflow_dispatch` run executes the verification job (in-place build + Python
suite), builds the whole wheel matrix and the sdist, smoke-tests every natively runnable
wheel and uploads each `python/dist/` as a workflow artifact (`dist-linux-x86_64`,
`dist-musllinux-aarch64`, `dist-macos-aarch64`, `dist-sdist`, …). Everything Python runs
with `working-directory: python`, where `pyproject.toml` lives, so `--out dist` is
`python/dist`. It never runs the publish job.

Use it to get a CI-built wheel for a platform you cannot build on — for example, download
`dist-macos-aarch64` from the run's Summary page and `pip install` the wheel it contains
in a fresh virtual environment — and to rehearse a release before tagging.

### Publish a release

1. Bump `[workspace.package] version` in `Cargo.toml` (see above) and commit: this is the
   release commit.
2. Merge the tested release commit to `main`.
3. Create and publish a GitHub Release whose tag is exactly `vX.Y.Z`, matching the
   workspace version. A draft release does not publish.
4. `publish.yml` runs from the tagged commit:
   - `verify` checks that the tag equals `v` + the workspace version and that the commit
     is an ancestor of `main`, builds the extension in place from `python/` (`maturin
     develop`), checks `mongol_norm.__version__` against the workspace version, and runs
     the Python suite;
   - the wheel jobs build the matrix below and smoke-test every wheel that can run on
     its build runner (`pip install --no-index --find-links python/dist mongol-norm`,
     import, shape `ᠰᠠᠢᠨ`, `mongol-norm shape ᠰᠠᠢᠨ`);
   - `sdist` builds the source distribution, validates its metadata (`twine check
     --strict` plus `python/scripts/check_dist_metadata.py`, which verifies that every
     declared `License-File` is inside the archive — PyPI rejects the upload otherwise,
     and twine does not check it — and that the long description really came from
     `python/README.pypi.md`) and installs it with pip, compiling the extension the way
     a user without a wheel would.

   If any of these fails, nothing is published.
5. The `publish` job enters the protected `pypi` environment, downloads all
   distributions into one `dist/`, checks that the set is complete (7 wheels + 1 sdist),
   runs the same metadata validation over all of them, and exchanges its GitHub OIDC identity for a short-lived PyPI credential
   (`pypa/gh-action-pypi-publish`, which also attaches PEP 740 attestations).
6. `publish-crate.yml` runs in parallel from the same release (see below).

Do not upload the same version twice: PyPI release files are immutable. If a
publication fails after any file reaches PyPI, increment the version before retrying.

#### The wheel matrix

All wheels are `cp39-abi3`: one wheel per platform serves every CPython ≥ 3.9. Builds
use `PyO3/maturin-action` (with `working-directory: python`) and `--release --locked`;
`MATURIN_VERSION` in `publish.yml` pins the maturin release used for the wheels and must
stay inside `[build-system] requires` in `python/pyproject.toml`, which governs the sdist
builds.

| Distribution | Runner | Build | Smoke-tested on the runner |
| --- | --- | --- | --- |
| manylinux2014 x86_64 | `ubuntu-latest`, `manylinux2014_x86_64` container | native | yes |
| manylinux2014 aarch64 | `ubuntu-latest`, `manylinux2014-cross:aarch64` container | cross | no |
| musllinux_1_2 x86_64 | `ubuntu-latest`, `rust-musl-cross` container | cross (musl) | no (glibc host) |
| musllinux_1_2 aarch64 | `ubuntu-latest`, `rust-musl-cross` container | cross | no |
| macOS x86_64 | `macos-latest` (Apple silicon) | cross | no |
| macOS arm64 | `macos-latest` | native | yes |
| Windows x64 | `windows-latest` | native | yes |
| sdist | `ubuntu-latest` | `maturin sdist` | installed with pip (compiles) |

Platforms outside this matrix (and `pip install --no-binary mongol-norm`) build from the
sdist, which needs a Rust toolchain ≥ 1.83 on the machine; pip fetches maturin itself
(`[build-system] requires` in `python/pyproject.toml`).

Because the binding crate depends on the workspace-root engine crate, maturin roots the
sdist at the workspace root. It carries exactly what pip needs to build the package: the
root `Cargo.toml` + `Cargo.lock` + `src/` + `LICENSE`/`NOTICE`/`README.md`, the binding
crate as `python/` (with its own `LICENSE`/`NOTICE`/`README.pypi.md`), and — re-rooted to
the archive root — `pyproject.toml` (rewritten to `manifest-path = "python/Cargo.toml"`),
`README.pypi.md` and `mongol_norm/`. `python/tests` and `python/scripts` are dropped by
`[tool.maturin] exclude`: the suite and the generators need the repository checkout and
its shared fixtures under `tests/`, so the sdist is buildable but not self-testable.

The PyPI long description comes from `python/README.pypi.md`, deliberately *not* named
`README.md`: the re-rooted `pyproject.toml` sits next to the crate's own `README.md` in
the sdist, and with the same name a wheel built from the sdist would carry the crate
README instead.

### The Rust crate

The engine crate (the repository root package, `mongol-norm`) is versioned in lockstep
with the Python package through the single workspace literal described above;
`publish-crate.yml` verifies that lockstep and the tag the same way `publish.yml` does.

crates.io publication is handled by `.github/workflows/publish-crate.yml` using
[crates.io Trusted Publishing](https://crates.io/docs/trusted-publishing): the same `vX.Y.Z`
GitHub Release that publishes to PyPI also publishes the crate, and no registry token is stored
in GitHub. (`v0.0.4` itself was published manually on 2026-09-02 with a scoped, short-lived
token, because a brand-new crate's first version cannot use Trusted Publishing.)

#### One-time setup (crate)

On [crates.io](https://crates.io/crates/mongol-norm/settings), under *Trusted Publishing*, add a
GitHub publisher with these exact values:

| Field | Value |
| --- | --- |
| Repository owner | `Satsrag` |
| Repository name | `mongol-norm` |
| Workflow filename | `publish-crate.yml` |
| Environment | `crates-io` |

Create a GitHub environment named `crates-io` as well (repository Settings → Environments).
Required reviewers may be configured on it if every crate publication should require manual
approval, mirroring the `pypi` environment.

#### Verify without publishing (crate)

Run **Publish crate to crates.io** from the Actions tab with `workflow_dispatch`. A manual run
verifies the lockstep versions, runs the engine crate's test suite (`cargo test -p mongol-norm
--locked`), packages the crate, checks the
crates.io registry state and uploads the `.crate` file as a workflow artifact. It never runs the
publish job.

#### Publish (crate)

Nothing extra: step 3 of the release process above (publishing the `vX.Y.Z` GitHub Release)
triggers both `publish.yml` (PyPI) and `publish-crate.yml` (crates.io). The crate's publish job
skips cleanly when the version already exists on crates.io with the expected checksum, so
re-running a release is safe. crates.io files are immutable, like PyPI's — never reuse a
version number.

---

<a id="中文"></a>
## 中文

一个 GitHub Release（tag `vX.Y.Z`）从同一个提交发布两个产物：

- PyPI 包 `mongol-norm`——wheel 和 sdist，`.github/workflows/publish.yml`；
- crates.io 上的 crate `mongol-norm`——`.github/workflows/publish-crate.yml`。

两者都通过受保护的 GitHub environment 使用 Trusted Publishing（GitHub OIDC）；GitHub 里不保存任何 PyPI 或
crates.io 的 token。

### 版本号

版本号字面量只存在于一个地方：根目录 `Cargo.toml` 里的 `[workspace.package] version`。其他地方都从它派生——
引擎 crate（根 package）和绑定 crate 使用 `version.workspace = true`，maturin 通过 `python/Cargo.toml` 读取它
（`python/pyproject.toml` 声明了 `dynamic = ["version"]`），`mongol_norm.__version__` 在导入时从扩展模块读取。
`python/tests/test_rust_twin.py` 和两个发布 workflow 都会检查运行时、crate 以及（发布时的）tag 与它一致。

升版本：修改根目录 `Cargo.toml` 里的字面量，运行 `cargo update -w` 让 `Cargo.lock` 记下新的 workspace
版本，跑完测试，提交。

### 一次性设置（PyPI）

PyPI 项目 `mongol-norm` 的维护者需要添加一个 trusted publisher，取值必须完全如下：

| 字段 | 取值 |
| --- | --- |
| PyPI project | `mongol-norm` |
| GitHub owner | `Satsrag` |
| Repository | `mongol-norm` |
| Workflow | `publish.yml` |
| Environment | `pypi` |

同时创建一个名为 `pypi` 的 GitHub environment。如果每次发布都需要人工批准，可以在这个 environment 上配置
required reviewers。

### 只构建 wheel、不发布

在 Actions 页面运行 **Build and publish to PyPI**（**Run workflow**，任意分支）。手动触发的
`workflow_dispatch` 运行会执行验证任务（原地构建 + Python 测试），构建整个 wheel 矩阵和 sdist，对每个能在
本机运行的 wheel 做冒烟测试，并把每个 `python/dist/` 作为 workflow artifact 上传（`dist-linux-x86_64`、
`dist-musllinux-aarch64`、`dist-macos-aarch64`、`dist-sdist`，……）。所有 Python 步骤都以
`working-directory: python`（`pyproject.toml` 所在目录）运行，所以 `--out dist` 就是 `python/dist`。它从不
运行发布任务。

它可以用来获取在自己机器上无法构建的平台的 CI wheel——例如从运行的 Summary 页面下载 `dist-macos-aarch64`，
在一个新的虚拟环境里 `pip install` 其中的 wheel——也可以在打 tag 之前演练一次发布。

### 发布一个版本

1. 升级 `Cargo.toml` 中的 `[workspace.package] version`（见上文）并提交：这就是发布提交。
2. 把测试通过的发布提交合并到 `main`。
3. 创建并发布一个 GitHub Release，tag 必须恰好是 `vX.Y.Z`，与 workspace 版本一致。草稿 release 不会触发发布。
4. `publish.yml` 从打 tag 的提交运行：
   - `verify` 检查 tag 等于 `v` + workspace 版本、该提交是 `main` 的祖先，在 `python/` 原地构建扩展
     （`maturin develop`），检查 `mongol_norm.__version__` 与 workspace 版本一致，并运行 Python 测试；
   - wheel 任务构建下面的矩阵，并对每个能在构建机上运行的 wheel 做冒烟测试
     （`pip install --no-index --find-links python/dist mongol-norm`、导入、对 `ᠰᠠᠢᠨ` 整形、
     `mongol-norm shape ᠰᠠᠢᠨ`）；
   - `sdist` 构建源码包，校验它的元数据（`twine check --strict` 加上 `python/scripts/check_dist_metadata.py`：
     后者检查每个声明的 `License-File` 都在压缩包里——否则 PyPI 会拒绝上传，而 twine 不检查这一点——并检查
     长描述确实来自 `python/README.pypi.md`），然后用 pip 安装它，像没有 wheel 的用户那样编译扩展。

   其中任何一步失败，都不会发布任何东西。
5. `publish` 任务进入受保护的 `pypi` environment，把所有分发包下载到同一个 `dist/`，检查集合完整
   （7 个 wheel + 1 个 sdist），对所有文件运行同样的元数据校验，然后用 GitHub OIDC 身份换取一个短期有效的
   PyPI 凭据（`pypa/gh-action-pypi-publish`，它也会附加 PEP 740 attestations）。
6. `publish-crate.yml` 由同一个 release 并行触发（见下文）。

不要重复上传同一个版本：PyPI 上的发布文件不可修改。如果发布在任何文件已经到达 PyPI 之后失败，重试前先升级版本号。

#### wheel 矩阵

所有 wheel 都是 `cp39-abi3`：每个平台一个 wheel，覆盖所有 CPython ≥ 3.9。构建使用 `PyO3/maturin-action`
（`working-directory: python`）以及 `--release --locked`；`publish.yml` 里的 `MATURIN_VERSION` 固定了构建 wheel
用的 maturin 版本，它必须落在 `python/pyproject.toml` 的 `[build-system] requires` 范围内（后者决定 sdist 构建
用的 maturin）。

| 分发包 | 运行环境 | 构建方式 | 是否在构建机上冒烟测试 |
| --- | --- | --- | --- |
| manylinux2014 x86_64 | `ubuntu-latest`，`manylinux2014_x86_64` 容器 | 原生 | 是 |
| manylinux2014 aarch64 | `ubuntu-latest`，`manylinux2014-cross:aarch64` 容器 | 交叉编译 | 否 |
| musllinux_1_2 x86_64 | `ubuntu-latest`，`rust-musl-cross` 容器 | 交叉编译（musl） | 否（宿主是 glibc） |
| musllinux_1_2 aarch64 | `ubuntu-latest`，`rust-musl-cross` 容器 | 交叉编译 | 否 |
| macOS x86_64 | `macos-latest`（Apple silicon） | 交叉编译 | 否 |
| macOS arm64 | `macos-latest` | 原生 | 是 |
| Windows x64 | `windows-latest` | 原生 | 是 |
| sdist | `ubuntu-latest` | `maturin sdist` | 用 pip 安装（会编译） |

矩阵之外的平台（以及 `pip install --no-binary mongol-norm`）从 sdist 构建，需要本机有 Rust ≥ 1.83 的工具链；
maturin 由 pip 自己获取（`python/pyproject.toml` 的 `[build-system] requires`）。

由于绑定 crate 依赖位于 workspace 根目录的引擎 crate，maturin 以 workspace 根目录为 sdist 的根。sdist 恰好
包含 pip 构建所需的内容：根目录的 `Cargo.toml` + `Cargo.lock` + `src/` + `LICENSE`/`NOTICE`/`README.md`，
作为 `python/` 的绑定 crate（带它自己的 `LICENSE`/`NOTICE`/`README.pypi.md`），以及重新放到压缩包根目录的
`pyproject.toml`（改写为 `manifest-path = "python/Cargo.toml"`）、`README.pypi.md` 和 `mongol_norm/`。
`python/tests` 和 `python/scripts` 被 `[tool.maturin] exclude` 排除：测试和生成器需要仓库检出以及 `tests/`
下共享的测试数据，所以 sdist 能构建，但不能自测。

PyPI 长描述来自 `python/README.pypi.md`，刻意*不*叫 `README.md`：重新放到根目录的 `pyproject.toml` 在 sdist 里
与 crate 自己的 `README.md` 相邻，如果同名，从 sdist 构建的 wheel 会带上 crate 的 README。

### Rust crate

引擎 crate（仓库根 package，`mongol-norm`）通过上面说的唯一 workspace 字面量与 Python 包保持版本同步；
`publish-crate.yml` 用与 `publish.yml` 相同的方式验证版本同步和 tag。

crates.io 发布由 `.github/workflows/publish-crate.yml` 通过
[crates.io Trusted Publishing](https://crates.io/docs/trusted-publishing) 完成：发布到 PyPI 的同一个
`vX.Y.Z` GitHub Release 也会发布 crate，GitHub 里不保存任何 registry token。（`v0.0.4` 本身是在 2026-09-02
用一个范围受限、短期有效的 token 手动发布的，因为全新 crate 的第一个版本不能使用 Trusted Publishing。）

#### 一次性设置（crate）

在 [crates.io](https://crates.io/crates/mongol-norm/settings) 的 *Trusted Publishing* 下添加一个 GitHub
publisher，取值必须完全如下：

| 字段 | 取值 |
| --- | --- |
| Repository owner | `Satsrag` |
| Repository name | `mongol-norm` |
| Workflow filename | `publish-crate.yml` |
| Environment | `crates-io` |

同时创建一个名为 `crates-io` 的 GitHub environment（仓库 Settings → Environments）。如果每次 crate 发布都需要
人工批准，可以在上面配置 required reviewers，与 `pypi` environment 保持一致。

#### 只验证、不发布（crate）

在 Actions 页面以 `workflow_dispatch` 运行 **Publish crate to crates.io**。手动运行会验证版本同步，运行引擎
crate 的测试（`cargo test -p mongol-norm --locked`），打包 crate，检查 crates.io 上的 registry 状态，并把
`.crate` 文件作为 workflow artifact 上传。它从不运行发布任务。

#### 发布（crate）

不需要额外操作：上面发布流程的第 3 步（发布 `vX.Y.Z` GitHub Release）会同时触发 `publish.yml`（PyPI）和
`publish-crate.yml`（crates.io）。如果 crates.io 上已经存在该版本且校验和一致，crate 的发布任务会干净地跳过，
所以重新运行一次发布是安全的。crates.io 的文件和 PyPI 一样不可修改——绝不要重复使用版本号。
