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
