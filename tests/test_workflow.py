"""Contract tests for the reusable native build workflow."""
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
import yaml

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github/workflows/build.yml"

class Contract(unittest.TestCase):
    def setUp(self):
        self.workflow = yaml.load(WORKFLOW.read_text(), Loader=yaml.BaseLoader)
        self.job = self.workflow["jobs"]["build"]
        self.steps = self.job["steps"]

    def test_all_eight_distinct_platform_version_pairs(self):
        rows = self.job["strategy"]["matrix"]["include"]
        self.assertEqual({(r["target"], r["obs"]) for r in rows},
            {(p,v) for p in ("linux-x86_64","windows-x64","macos-arm64","macos-x86_64")
             for v in ("32.2.0","33.0.3")})
        self.assertEqual(len(rows),8)

    def test_upload_has_exact_paths_and_no_release_permission(self):
        upload = next(s for s in self.steps if s.get("name") == "Upload verified test package")
        self.assertEqual(self.workflow["permissions"], {"contents":"read"})
        self.assertEqual(upload["with"]["if-no-files-found"], "error")
        self.assertEqual(upload["with"]["path"].splitlines(),
                         ["${{ runner.temp }}/plugin-ci/${{ env.PUBLIC_ARCHIVE }}",
                          "${{ runner.temp }}/plugin-ci/SHA256SUMS"])
        self.assertIn("inputs.package-name", upload["with"]["name"])
        self.assertIn("matrix.target", upload["with"]["name"])
        self.assertIn("matrix.obs", upload["with"]["name"])

    def test_checksum_gate_accepts_only_matching_named_package(self):
        step = next(s for s in self.steps if s.get("name")=="Verify artifact handoff")
        with tempfile.TemporaryDirectory() as temp:
            p=Path(temp)/"plugin-ci"; p.mkdir()
            archive=p/"example-linux-x86_64-obs32.2.0-test.tar.gz"
            archive.write_bytes(b"test package")
            check=p/"SHA256SUMS"
            expected=hashlib.sha256(archive.read_bytes()).hexdigest()+"  "+archive.name+"\n"
            env={**os.environ,"RUNNER_TEMP":temp,"ARCHIVE":archive.name,"PUBLIC_ARCHIVE":archive.name}
            def gate():
                return subprocess.run(["bash","-c",step["run"]],env=env,
                    capture_output=True).returncode
            check.write_text(expected)
            self.assertEqual(gate(),0)
            archive.write_bytes(b"corrupted")
            self.assertNotEqual(gate(),0)
            archive.write_bytes(b"test package")
            check.write_text(expected+"extra entry\n")
            self.assertNotEqual(gate(),0)
            check.unlink()
            self.assertNotEqual(gate(),0)
            check.write_text(expected)
            archive.unlink()
            self.assertNotEqual(gate(),0)

    def test_public_name_preserves_verified_bytes_and_updates_manifest(self):
        step = next(s for s in self.steps if s.get("name")=="Verify artifact handoff")
        with tempfile.TemporaryDirectory() as temp:
            p=Path(temp)/"plugin-ci"; p.mkdir()
            old=p/"obs-streaming-media-linux-x86_64-obs32.2.0-test.tar.gz"
            new=p/"streaming-media-source-linux-x86_64-obs32.2.0-test.tar.gz"
            old.write_bytes(b"verified native module archive")
            digest=hashlib.sha256(old.read_bytes()).hexdigest()
            manifest=p/"SHA256SUMS"
            manifest.write_text(digest+"  "+old.name+"\n")
            env={**os.environ,"RUNNER_TEMP":temp,"ARCHIVE":old.name,"PUBLIC_ARCHIVE":new.name}
            def gate():
                return subprocess.run(["bash","-c",step["run"]],env=env,capture_output=True)
            new.write_bytes(b"existing package")
            self.assertNotEqual(gate().returncode,0)
            self.assertEqual(new.read_bytes(),b"existing package")
            new.unlink()
            result=gate()
            self.assertEqual(result.returncode,0,result.stderr.decode())
            self.assertFalse(old.exists())
            self.assertEqual(hashlib.sha256(new.read_bytes()).hexdigest(),digest)
            self.assertEqual(manifest.read_text(),digest+"  "+new.name+"\n")

if __name__ == "__main__":
    unittest.main()
