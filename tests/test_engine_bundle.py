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
import yaml

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

    @unittest.skipUnless(platform.system() == "Linux", "Execute actual Linux workflow restore step")
    def test_production_restore_executes_verified_engine_and_rejects_tampering(self):
        workflow = yaml.load((ROOT / ".github/workflows/build.yml").read_text(), Loader=yaml.BaseLoader)
        step = next(s for s in workflow["jobs"]["build"]["steps"]
                    if s.get("name") == "Restore verified shared engine")
        revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            bundle = root / "bundle"
            subprocess.run([sys.executable, str(SCRIPT), "pack", "--target", TARGET,
                            "--revision", revision, "--binary", str(EXE),
                            "--directory", str(bundle)], check=True)
            for tamper in (False, True):
                work = root / ("tampered" if tamper else "valid")
                work.mkdir()
                received = work / "engine-received"
                shutil.copytree(bundle, received)
                if tamper:
                    with (received / "obs-ci").open("ab") as stream:
                        stream.write(b"tampered")
                env_file = work / "env"
                env = {**os.environ, "TARGET": TARGET, "ENGINE_REVISION": revision,
                       "RUNNER_TEMP": str(work), "GITHUB_ENV": str(env_file),
                       "GIT_CONFIG_COUNT": "1", "GIT_CONFIG_KEY_0": f"url.{ROOT.as_uri()}.insteadOf",
                       "GIT_CONFIG_VALUE_0": "https://github.com/myrqyry/obs-plugin-ci.git"}
                result = subprocess.run(["bash", "-c", step["run"]], env=env,
                                        capture_output=True, text=True)
                executable = work / "obs-ci-engine/target/release/obs-ci"
                if tamper:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("Engine receipt mismatch", result.stderr)
                    self.assertFalse(executable.exists())
                    self.assertFalse(env_file.exists())
                else:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(env_file.read_text().strip(), f"OBS_CI_ENGINE_ROOT={work / 'obs-ci-engine'}")
                    self.assertEqual(executable.read_bytes(), EXE.read_bytes())
                    self.assertEqual(subprocess.run([str(executable), "--version"], capture_output=True).returncode, 0)

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
