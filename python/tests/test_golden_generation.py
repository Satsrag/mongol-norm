"""The committed compatibility golden fixtures must be reproducible."""
import subprocess
import sys
import unittest

from tests._support import PYTHON_DIR, REPO_ROOT


_SCRIPT = PYTHON_DIR / "scripts" / "gen_compat_goldens.py"


class TestCompatibilityGoldenGeneration(unittest.TestCase):
    def test_committed_goldens_are_fresh(self):
        result = subprocess.run(
            [sys.executable, str(_SCRIPT), "--check"],
            cwd=str(REPO_ROOT),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            encoding="utf-8",
            timeout=300,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
