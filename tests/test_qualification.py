"""End-to-end qualification fixture, without GitHub or an OBS SDK."""
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ENGINE = ROOT / "target/debug" / ("obs-ci.exe" if os.name == "nt" else "obs-ci")
SCRIPT = ROOT / "scripts/qualify.py"
TARGETS = {
    ("Linux", "x86_64"): "linux-x86_64",
    ("Windows", "amd64"): "windows-x64",
    ("Windows", "x86_64"): "windows-x64",
    ("Darwin", "arm64"): "macos-arm64",
    ("Darwin", "x86_64"): "macos-x86_64",
}


def run(*args, check=True):
    return subprocess.run([sys.executable, str(SCRIPT), *map(str, args)],
                          cwd=ROOT, capture_output=True, text=True, check=check)


class Qualification(unittest.TestCase):
    def test_matrix_source_host_and_selection(self):
        args = ["--engine", ENGINE]
        full = json.loads(run("plan", *args).stdout)["include"]
        self.assertEqual(len(full), 8)
        self.assertEqual([(r["target"], r["obs"]) for r in full if r["emit_source"]],
                         [("linux-x86_64", "32.2.0")])
        filtered = json.loads(run("plan", *args, "--obs-versions", "33.0.3").stdout)["include"]
        self.assertEqual(len(filtered), 4)
        self.assertEqual([(r["target"], r["obs"]) for r in filtered if r["emit_source"]],
                         [("linux-x86_64", "33.0.3")])
        windows = json.loads(run("plan", *args, "--targets", "windows-x64").stdout)["include"]
        self.assertFalse(any(r["emit_source"] for r in windows))
        self.assertNotEqual(run("plan", *args, "--targets", "unknown", check=False).returncode, 0)

    def test_local_fixture_round_trip_and_tamper_detection(self):
        target = TARGETS[(platform.system(), platform.machine().lower())]
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            downloads = root / "downloads"
            downloads.mkdir()
            args = ["--engine", ENGINE]
            plan = json.loads(run("plan", *args, "--targets", target).stdout)
            rows = plan["include"]
            self.assertEqual(len(rows), 2)
            self.assertEqual(sum(row["emit_source"] for row in rows), int(target.startswith("linux")))
            for row in rows:
                build = ["build", *args, "--work", downloads, "--target", row["target"],
                         "--obs-version", row["obs"]]
                if row["emit_source"]:
                    build.append("--emit-source")
                run(*build)
            report = root / "qualification.json"
            collect = ["collect", *args, "--downloads", downloads, "--matrix-json",
                       json.dumps(plan), "--output", report]
            run(*collect)
            receipt = json.loads(report.read_text(encoding="utf-8"))
            self.assertEqual(receipt["scope"], "synthetic-fixture-only")
            self.assertEqual(len(receipt["modules"]), 2)
            self.assertEqual(receipt["source"] is not None, target.startswith("linux"))
            if receipt["source"] is not None:
                self.assertTrue(receipt["source"]["payload_verified"])
            first = downloads / f"rust-fixture-{target}-obs32.2.0-test"
            module_receipt = first / "MODULE-RECEIPT.json"
            original = module_receipt.read_bytes()
            data = json.loads(original)
            data["architecture"] = "wrong"
            module_receipt.write_text(json.dumps(data), encoding="utf-8")
            self.assertNotEqual(run(*collect, check=False).returncode, 0)
            self.assertFalse(report.exists())
            module_receipt.write_bytes(original)
            (first / "unexpected.txt").write_text("unexpected")
            self.assertNotEqual(run(*collect, check=False).returncode, 0)
            (first / "unexpected.txt").unlink()
            run(*collect)
            binary = next(first.glob("rust-fixture-*"))
            binary.write_bytes(binary.read_bytes() + b"tampered")
            self.assertNotEqual(run(*collect, check=False).returncode, 0)
            self.assertFalse(report.exists())


if __name__ == "__main__":
    unittest.main()
