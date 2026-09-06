# Releasing mongol-norm

One GitHub Release (tag `vX.Y.Z`) publishes two artifacts from the same commit:

- the PyPI package `mongol-norm` — wheels and an sdist, `.github/workflows/publish.yml`;
- the crate `mongol-norm` on crates.io — `.github/workflows/publish-crate.yml`.

Both use Trusted Publishing (GitHub OIDC) from a protected GitHub environment; no PyPI or
crates.io token is stored in GitHub.

## The version

The version literal lives in exactly one place: `[workspace.package] version` in the root
`Cargo.toml`. Everything else derives from it — the engine crate (the root package) and
the binding crate use `version.workspace = true`, maturin reads it through
`python/Cargo.toml` (`python/pyproject.toml` declares `dynamic = ["version"]`), and
`mongol_norm.__version__` is read from the extension module at import time.
`python/tests/test_rust_twin.py` and both publish workflows check that the runtime, the
crate and (on a release) the tag agree with it.

To bump it: edit the literal in the root `Cargo.toml`, run `cargo update -w` so
`Cargo.lock` records the new workspace version, run the suites, commit.

## One-time setup (PyPI)

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

## Build wheels without publishing

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

## Publish a release

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

### The wheel matrix

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

## The Rust crate

The engine crate (the repository root package, `mongol-norm`) is versioned in lockstep
with the Python package through the single workspace literal described above;
`publish-crate.yml` verifies that lockstep and the tag the same way `publish.yml` does.

crates.io publication is handled by `.github/workflows/publish-crate.yml` using
[crates.io Trusted Publishing](https://crates.io/docs/trusted-publishing): the same `vX.Y.Z`
GitHub Release that publishes to PyPI also publishes the crate, and no registry token is stored
in GitHub. (`v0.0.4` itself was published manually on 2026-09-02 with a scoped, short-lived
token, because a brand-new crate's first version cannot use Trusted Publishing.)

### One-time setup (crate)

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

### Verify without publishing (crate)

Run **Publish crate to crates.io** from the Actions tab with `workflow_dispatch`. A manual run
verifies the lockstep versions, runs the engine crate's test suite (`cargo test -p mongol-norm
--locked`), packages the crate, checks the
crates.io registry state and uploads the `.crate` file as a workflow artifact. It never runs the
publish job.

### Publish (crate)

Nothing extra: step 3 of the release process above (publishing the `vX.Y.Z` GitHub Release)
triggers both `publish.yml` (PyPI) and `publish-crate.yml` (crates.io). The crate's publish job
skips cleanly when the version already exists on crates.io with the expected checksum, so
re-running a release is safe. crates.io files are immutable, like PyPI's — never reuse a
version number.
