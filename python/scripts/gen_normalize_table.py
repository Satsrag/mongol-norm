#!/usr/bin/env python3
"""
Generate the normalize table JSON (python/mongol_norm/data/MNG.normalize.json).

The generator itself is the Rust example ``examples/gen_normalize_table``: it
explores every context the online normalize encoder can reach and asks the
crate's shaping engine — tens of millions of probe shapes — which letters are
safe to commit there, which is why it is Rust, not Python. This script only
runs it, so every generated file keeps a ``python/scripts`` entry point:

    python python/scripts/gen_normalize_table.py            # regenerate
    python python/scripts/gen_normalize_table.py --check    # exit 1 if stale
    python python/scripts/gen_rust_tables.py                # JSON -> src/generated/

It needs a Rust toolchain (``cargo``), not the Python extension. The encoder
and the table format are described in docs/internals.md and
docs/data-format.md.
"""
import argparse
import shutil
import subprocess
import sys
from pathlib import Path

# The repository root (python/scripts/ -> python/ -> root), where cargo runs.
ROOT = Path(__file__).resolve().parents[2]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[1])
    parser.add_argument(
        "--check", action="store_true",
        help="fail instead of writing when the bundled table is stale",
    )
    parser.add_argument(
        "-v", "--verbose", action="store_true",
        help="report the exploration level by level",
    )
    parser.add_argument(
        "--threads", type=int,
        help="worker threads (default: all cores)",
    )
    args = parser.parse_args()

    cargo = shutil.which("cargo")
    if cargo is None:
        parser.error("cargo not found: the generator is the Rust example "
                     "examples/gen_normalize_table")
    command = [cargo, "run", "--quiet", "--release", "--locked",
               "--example", "gen_normalize_table", "--"]
    if args.check:
        command.append("--check")
    if args.verbose:
        command.append("--verbose")
    if args.threads is not None:
        command += ["--threads", str(args.threads)]
    return subprocess.run(command, cwd=str(ROOT)).returncode


if __name__ == "__main__":
    sys.exit(main())
