"""The committed compatibility golden fixtures must be reproducible."""
import subprocess
import sys
import unittest
from pathlib import Path


_ROOT = Path(__file__).resolve().parent.parent
_SCRIPT = _ROOT / "scripts" / "gen_compat_goldens.py"


class TestCompatibilityGoldenGeneration(unittest.TestCase):
    def test_committed_goldens_are_fresh(self):
        result = subprocess.run(
            [sys.executable, str(_SCRIPT), "--check"],
            cwd=str(_ROOT),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            universal_newlines=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
