#!/usr/bin/env python3
"""
Check that every ``License-File`` a wheel or sdist declares is really inside it.

PyPI enforces this for Metadata-Version 2.4 uploads — a missing file is rejected
with "License-File X does not exist in distribution file …" — and ``twine check``
does not (mongol-norm 0.1.0's sdist got through twine and was then refused by
PyPI). Run it on everything in ``python/dist/`` before publishing:

    python python/scripts/check_dist_metadata.py --require LICENSE --require NOTICE \
        --description-contains pypi/pyversions python/dist/*

``--description-contains`` guards the other half of the metadata: which readme became
the PyPI long description. maturin re-roots ``pyproject.toml`` at the sdist root next to
the crate's own ``README.md``, so a rename of ``python/README.pypi.md`` would silently
publish the Rust crate's README as the package page.

Exit status 0 when every archive is consistent, 1 otherwise (with one line per
problem on stderr).
"""
import argparse
import email.parser
import sys
import tarfile
import zipfile
from pathlib import Path


def _metadata_and_members(path):
    """Return (metadata text, set of archive member paths, license root prefix)."""
    if path.suffix == ".whl":
        with zipfile.ZipFile(path) as archive:
            members = set(archive.namelist())
            dist_info = sorted(
                name.split("/", 1)[0]
                for name in members
                if name.count("/") == 1 and name.endswith(".dist-info/METADATA")
            )
            if len(dist_info) != 1:
                raise ValueError(f"expected exactly one .dist-info/METADATA, found {dist_info}")
            text = archive.read(f"{dist_info[0]}/METADATA").decode("utf-8")
            return text, members, f"{dist_info[0]}/licenses/"
    if path.name.endswith(".tar.gz"):
        with tarfile.open(path, "r:gz") as archive:
            members = {member.name for member in archive.getmembers()}
            top_level = {name.split("/", 1)[0] for name in members}
            if len(top_level) != 1:
                raise ValueError(f"expected exactly one top-level directory, found {sorted(top_level)}")
            top = next(iter(top_level))
            pkg_info = archive.extractfile(f"{top}/PKG-INFO")
            if pkg_info is None:
                raise ValueError("no PKG-INFO")
            return pkg_info.read().decode("utf-8"), members, f"{top}/"
    raise ValueError("not a .whl or .tar.gz")


def check(path, required, description_contains=()):
    """Return a list of problems (empty when the archive is consistent)."""
    problems = []
    try:
        text, members, prefix = _metadata_and_members(path)
    except (OSError, ValueError, zipfile.BadZipFile, tarfile.TarError) as error:
        return [f"{path.name}: cannot read: {error}"]
    headers = email.parser.Parser().parsestr(text)
    description = headers.get_payload() or headers.get("Description") or ""
    for marker in description_contains:
        if marker not in description:
            problems.append(
                f"{path.name}: the long description does not contain {marker!r} "
                "— wrong readme file?"
            )
    declared = headers.get_all("License-File") or []
    for name in declared:
        if f"{prefix}{name}" not in members:
            problems.append(f"{path.name}: License-File {name!r} is declared but {prefix}{name} is missing")
    for name in required:
        if name not in declared:
            problems.append(f"{path.name}: metadata does not declare License-File {name!r}")
    if not problems:
        print(f"ok: {path.name} ({len(declared)} license file(s): {', '.join(declared) or '-'})")
    return problems


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("dist", nargs="+", type=Path, help="wheel (.whl) or sdist (.tar.gz) files")
    parser.add_argument(
        "--require",
        action="append",
        default=[],
        metavar="NAME",
        help="a License-File every archive must declare (repeatable)",
    )
    parser.add_argument(
        "--description-contains",
        action="append",
        default=[],
        metavar="TEXT",
        help=(
            "text the long description must contain (repeatable) -- pins which readme "
            "became the PyPI page, which nothing else checks"
        ),
    )
    args = parser.parse_args(argv)
    problems = []
    for path in args.dist:
        problems.extend(check(path, args.require, args.description_contains))
    for problem in problems:
        print(f"error: {problem}", file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
