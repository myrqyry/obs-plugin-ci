"""Compare the Rust CLI with the currently authoritative workflow scripts."""
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
import warnings
import zipfile

import yaml

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / "target" / "debug" / ("obs-ci.exe" if os.name == "nt" else "obs-ci")
WORKFLOW = yaml.load((ROOT / ".github/workflows/build.yml").read_text(encoding="utf-8"),
                     Loader=yaml.BaseLoader)


def python_step(step, env):
    # Execute the same embedded Python, using this interpreter on every OS.
    code = step["run"].split("<<'PYTHON'\n", 1)[1].rsplit("\nPYTHON", 1)[0]
    return subprocess.run([sys.executable, "-c", code], env=env, capture_output=True)


class Parity(unittest.TestCase):
    def test_planner(self):
        step = WORKFLOW["jobs"]["plan"]["steps"][0]
        cases = [(p, d, s, "example", "public-name", "9")
                 for p in ("full", "linux-obs32")
                 for d, s in (("native", False), ("caller", False), ("caller", True))]
        cases += [
            ("unknown", "native", False, "example", "example", "1"),
            ("full", "unknown", False, "example", "example", "1"),
            ("full", "native", True, "example", "example", "1"),
            ("full", "native", False, "../bad", "example", "1"),
            ("full", "native", False, "example", "bad/name", "1"),
            ("full", "native", False, "example", "example", "0"),
            ("full", "native", False, "example", "example", "01"),
            ("full", "native", False, "example", "example", "1.5"),
        ]
        for profile, driver, source, module, public, minimum in cases:
            with self.subTest(case=(profile, driver, source, module, public, minimum)):
                with tempfile.TemporaryDirectory() as temp:
                    output = Path(temp) / "output"
                    env = {**os.environ, "PROFILE": profile, "DRIVER": driver,
                           "SOURCE_PACKAGE": str(source).lower(), "MODULE": module,
                           "PUBLIC_NAME": public, "MINIMUM_TESTS": minimum,
                           "GITHUB_OUTPUT": str(output)}
                    py = python_step(step, env)
                    args = [str(EXE), "plan", "--module", module, "--package-name", public,
                            "--minimum-tests", minimum, "--profile", profile, "--driver", driver]
                    if source:
                        args.append("--source-package")
                    rust = subprocess.run(args, capture_output=True)
                    self.assertEqual(py.returncode == 0, rust.returncode == 0, rust.stderr)
                    if py.returncode == 0:
                        self.assertEqual(json.loads(output.read_text(encoding="utf-8").split("=", 1)[1]),
                                         json.loads(rust.stdout))

    def test_binary_archive_gates(self):
        step = next(s for s in WORKFLOW["jobs"]["build"]["steps"]
                    if s.get("name") == "Verify artifact handoff")
        cases = [
            ("valid", ["example/module"], True),
            ("empty", [], False),
            ("duplicate", ["example/a", "example/a"], False),
            ("case", ["example/File", "example/file"], False),
            ("unicode case", ["example/Straße", "example/STRASSE"], False),
            ("parent", ["example/../escape"], False),
            ("absolute", ["/outside"], False),
            ("drive", ["C:/outside"], False),
            ("separator", ["example//file"], False),
            ("git", ["example/.git/config"], False),
            ("file parent", ["example", "example/file"], False),
            ("truncated", ["example/module"], False),
        ]
        for extension in ("zip", "tar.gz"):
            for label, names, accepted in cases:
                with self.subTest(extension=extension, case=label):
                    with tempfile.TemporaryDirectory() as temp:
                        work = Path(temp) / "plugin-ci"
                        work.mkdir()
                        archive = work / ("example-test." + extension)
                        if extension == "zip":
                            with warnings.catch_warnings():
                                warnings.simplefilter("ignore", UserWarning)
                                with zipfile.ZipFile(archive, "w") as package:
                                    for name in names:
                                        package.writestr(name, b"module")
                        else:
                            with tarfile.open(archive, "w:gz") as package:
                                for name in names:
                                    entry = tarfile.TarInfo(name)
                                    entry.size = 6
                                    package.addfile(entry, io.BytesIO(b"module"))
                        if label == "truncated":
                            archive.write_bytes(archive.read_bytes()[:-8])
                        original = archive.read_bytes()
                        manifest = work / "SHA256SUMS"
                        manifest.write_text(hashlib.sha256(original).hexdigest()
                                            + "  " + archive.name + "\n", encoding="utf-8")
                        env = {**os.environ, "RUNNER_TEMP": temp, "ARCHIVE": archive.name,
                               "PUBLIC_ARCHIVE": archive.name, "GITHUB_STEP_SUMMARY": ""}
                        py = python_step(step, env)
                        rust = subprocess.run([str(EXE), "audit", str(archive)], capture_output=True)
                        self.assertEqual(py.returncode == 0, accepted, py.stderr)
                        self.assertEqual(rust.returncode == 0, accepted, rust.stderr)
                        self.assertEqual(archive.read_bytes(), original)


if __name__ == "__main__":
    unittest.main()
