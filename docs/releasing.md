# Releasing mongol-norm

PyPI publication is handled by `.github/workflows/publish.yml` using
[PyPI Trusted Publishing](https://docs.pypi.org/trusted-publishers/). No PyPI API
token is stored in GitHub.

## One-time setup

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

## Verify without publishing

Run **Build and publish to PyPI** from the Actions tab with
`workflow_dispatch`. A manual run executes the tests, builds the wheel and
source distribution, checks their metadata, smoke-tests the wheel, and uploads
both files as a workflow artifact. It never runs the publish job.

## Publish a release

1. Update the version in `pyproject.toml` and `mongol_norm/__init__.py` in the
   release commit.
2. Merge the tested release commit to `main`.
3. Create and publish a GitHub Release whose tag is exactly `vX.Y.Z`, matching
   the package version. A draft release does not publish to PyPI.
4. The workflow rebuilds and tests from the tagged commit. If the tag and
   package version differ, it stops before publication.
5. The `publish` job enters the protected `pypi` environment and exchanges its
   GitHub OIDC identity for a short-lived PyPI credential.

Do not upload the same version twice: PyPI release files are immutable. If a
publication fails after any file reaches PyPI, increment the package version
before retrying.

## The Rust crate

`crates/mongol-norm` is versioned in lockstep with the Python package: `[workspace.package]
version` in the root `Cargo.toml` must equal `pyproject.toml` and `mongol_norm.__version__`
(`tests/test_rust_twin.py` and the publish workflows check this). Update all three in the release
commit.

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
verifies the lockstep versions, runs the Rust test suite, packages the crate, checks the
crates.io registry state and uploads the `.crate` file as a workflow artifact. It never runs the
publish job.

### Publish (crate)

Nothing extra: step 3 of the release process above (publishing the `vX.Y.Z` GitHub Release)
triggers both `publish.yml` (PyPI) and `publish-crate.yml` (crates.io). The crate's publish job
skips cleanly when the version already exists on crates.io with the expected checksum, so
re-running a release is safe. crates.io files are immutable, like PyPI's — never reuse a
version number.
