"""Exercise engine artifact transport without executing unverified downloads."""
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/engine_bundle.py"
EXE = ROOT / "target/debug" / ("obs-ci.exe" if os.name == "nt" else "obs-ci")
TARGET = {
    ("Linux", "x86_64"): "linux-x86_64",
    ("Windows", "amd64"): "windows-x64",
    ("Windows", "x86_64"): "windows-x64",
    ("Darwin", "arm64"): "macos-arm64",
    ("Darwin", "x86_64"): "macos-x86_64",
}[(platform.system(), platform.machine().lower())]


class EngineBundle(unittest.TestCase):
    def test_matrix_deduplicates_obs_versions_and_omits_linux(self):
        def matrix(targets):
            rows = [{"target": target, "obs": obs} for target in targets
                    for obs in ("32.2.0", "33.0.3")]
            result = subprocess.check_output([sys.executable, str(SCRIPT), "matrix",
                "--matrix-json", json.dumps({"include": rows})], text=True, stderr=subprocess.PIPE)
            return json.loads(result)["include"]
        self.assertEqual(matrix(["linux-x86_64"]), [])
        self.assertEqual([row["target"] for row in matrix(
            ["macos-arm64", "linux-x86_64", "windows-x64", "macos-x86_64"])],
            ["windows-x64", "macos-arm64", "macos-x86_64"])
        with self.assertRaises(subprocess.CalledProcessError):
            matrix(["bogus"])

    def test_transport_and_tamper_detection(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            bundle = root / "bundle"
            revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
            common = ["--target", TARGET, "--revision", revision]
            def run(*args):
                return subprocess.run([sys.executable, str(SCRIPT), *map(str, args)],
                                      cwd=ROOT, capture_output=True, text=True)
            packed = run("pack", *common, "--binary", EXE, "--directory", bundle)
            self.assertEqual(packed.returncode, 0, packed.stderr)
            received = root / "received"
            shutil.copytree(bundle, received)
            verified = run("verify", *common, "--directory", received)
            self.assertEqual(verified.returncode, 0, verified.stderr)
            binary = Path(verified.stdout.strip())
            other = "windows-x64" if TARGET != "windows-x64" else "linux-x86_64"
            self.assertNotEqual(run("verify", "--target", other, "--revision", revision,
                                    "--directory", received).returncode, 0)
            self.assertNotEqual(run("verify", "--target", TARGET, "--revision", "0" * 40,
                                    "--directory", received).returncode, 0)
            self.assertEqual(binary.read_bytes(), EXE.read_bytes())
            self.assertTrue(subprocess.run([str(binary), "--version"], capture_output=True).returncode == 0)
            receipt = received / "ENGINE.json"
            before = receipt.read_bytes()
            for key, value in [("revision", "0" * 40), ("target", "other"),
                               ("lock_sha256", "0" * 64), ("sha256", "0" * 64),
                               ("bytes", 0), ("rustc", "wrong")]:
                data = json.loads(before)
                data[key] = value
                receipt.write_text(json.dumps(data), encoding="utf-8")
                self.assertNotEqual(run("verify", *common, "--directory", received).returncode, 0, key)
            receipt.write_bytes(before)
            (received / "extra").write_bytes(b"x")
            self.assertNotEqual(run("verify", *common, "--directory", received).returncode, 0)
            (received / "extra").unlink()
            binary.write_bytes(binary.read_bytes() + b"tampered")
            self.assertNotEqual(run("verify", *common, "--directory", received).returncode, 0)
            self.assertNotEqual(run("pack", *common, "--binary", EXE, "--directory", bundle).returncode, 0)


if __name__ == "__main__":
    unittest.main()
