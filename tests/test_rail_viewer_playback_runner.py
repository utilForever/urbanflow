"""Exercise runner failures in disposable copies of the real browser tests."""

from pathlib import Path
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory
import unittest


class RunnerFailures(unittest.TestCase):
    def test_failures_exit_unsuccessfully(self):
        root = Path(__file__).resolve().parent.parent
        cases = [
            (
                "assertion",
                "tests/rail_viewer_playback.html",
                'assert(!summary.hidden, "Completed summary is visible on load");',
                'assert(false, "Deliberate assertion failure");',
                "Deliberate assertion failure",
            ),
            (
                "load",
                "tests/rail_viewer_playback.html",
                "./fixtures/rail_service.json",
                "./fixtures/missing.json",
                "Could not load ./fixtures/missing.json: 404",
            ),
            (
                "unfinished",
                "tests/rail_viewer_playback.html",
                "await run();",
                "await new Promise(() => {});",
                "did not finish within the browser budget",
            ),
            (
                "viewer script error",
                "examples/rail_viewer/viewer.html",
                "      renderSnapshot();\n    </script>",
                '      renderSnapshot();\n      throw new Error("Viewer initialization failed");\n'
                "    </script>",
                "Viewer initialization failed",
            ),
            (
                "viewer rejected promise before iframe replacement",
                "examples/rail_viewer/viewer.html",
                "      renderSnapshot();\n    </script>",
                "      renderSnapshot();\n"
                '      if (data.analysis) Promise.reject(new Error("Viewer promise rejected"));\n'
                "    </script>",
                "Viewer promise rejected",
            ),
        ]
        for name, target, before, after, diagnostic in cases:
            with self.subTest(name=name), TemporaryDirectory() as directory:
                copy = Path(directory)
                for relative in (
                    "tests/run_rail_viewer_playback.py",
                    "tests/rail_viewer_playback.html",
                    "tests/fixtures/rail_service.json",
                    "examples/rail_viewer/viewer.html",
                ):
                    destination = copy / relative
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(root / relative, destination)
                page = copy / target
                source = page.read_text(encoding="utf-8")
                self.assertEqual(source.count(before), 1)
                page.write_text(source.replace(before, after), encoding="utf-8")
                result = subprocess.run(
                    [sys.executable, str(copy / "tests/run_rail_viewer_playback.py")],
                    capture_output=True,
                    text=True,
                    timeout=40,
                )
                output = result.stdout + result.stderr
                self.assertEqual(result.returncode, 1, output)
                self.assertIn(diagnostic, output)


if __name__ == "__main__":
    unittest.main()
