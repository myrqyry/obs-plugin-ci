"""Contract tests for the reusable native build workflow."""
import hashlib
import io
import tarfile
import zipfile
import stat
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
import yaml

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github/workflows/build.yml"

def make_archive(path, entries=(("example/bin/plugin.so", b"module"),)):
    if path.suffix == ".zip":
        with zipfile.ZipFile(path, "w") as archive:
            for name, data in entries:
                archive.writestr(name, data)
    else:
        with tarfile.open(path, "w:gz") as archive:
            for name, data in entries:
                member = tarfile.TarInfo(name)
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))


class Contract(unittest.TestCase):
    def setUp(self):
        self.workflow = yaml.load(WORKFLOW.read_text(), Loader=yaml.BaseLoader)
        self.job = self.workflow["jobs"]["build"]
        self.steps = self.job["steps"]

    def plan(self, profile, driver="native", source="false", **inputs):
        step = next(s for s in self.workflow["jobs"]["plan"]["steps"]
                    if s.get("name") == "Select qualified matrix")
        with tempfile.TemporaryDirectory() as temp:
            output=Path(temp)/"output"
            env={**os.environ,"PROFILE":profile,"DRIVER":driver,
                 "SOURCE_PACKAGE":source,"GITHUB_OUTPUT":str(output),
                 "MODULE":"example","PUBLIC_NAME":"example","MINIMUM_TESTS":"1", **inputs}
            result=subprocess.run(["bash","-c",step["run"]],env=env,capture_output=True)
            if result.returncode:
                return None
            return json.loads(output.read_text().strip().split("=",1)[1])["include"]

    def test_all_eight_distinct_platform_version_pairs(self):
        rows = self.plan("full")
        self.assertEqual({(r["target"], r["obs"]) for r in rows},
            {(p,v) for p in ("linux-x86_64","windows-x64","macos-arm64","macos-x86_64")
             for v in ("32.2.0","33.0.3")})
        self.assertEqual(len(rows),8)

    def test_qualified_linux_subset_and_invalid_profiles(self):
        rows=self.plan("linux-obs32","caller","true")
        self.assertEqual(len(rows),1)
        self.assertEqual((rows[0]["target"],rows[0]["obs"],rows[0]["runner"]),
                         ("linux-x86_64","32.2.0","ubuntu-24.04"))
        self.assertIsNone(self.plan("typo"))
        self.assertIsNone(self.plan("linux-obs32","unknown"))
        self.assertIsNone(self.plan("full","native","true"))

    def test_invalid_build_inputs_fail_before_matrix_expansion(self):
        for key, values in {
            "MODULE": ["", "../module", "has space"],
            "PUBLIC_NAME": ["", "bad/name", "bad\nname"],
            "MINIMUM_TESTS": ["0", "-1", "1.5", "not-a-number"],
        }.items():
            for value in values:
                with self.subTest(key=key, value=value):
                    self.assertIsNone(self.plan("full", **{key: value}))

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
            make_archive(archive)
            check=p/"SHA256SUMS"
            original=archive.read_bytes()
            expected=hashlib.sha256(original).hexdigest()+"  "+archive.name+"\n"
            env={**os.environ,"RUNNER_TEMP":temp,"ARCHIVE":archive.name,"PUBLIC_ARCHIVE":archive.name}
            def gate():
                return subprocess.run(["bash","-c",step["run"]],env=env,
                    capture_output=True).returncode
            check.write_text(expected)
            self.assertEqual(gate(),0)
            archive.write_bytes(b"corrupted")
            self.assertNotEqual(gate(),0)
            archive.write_bytes(original)
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
            make_archive(old)
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

    def test_binary_archive_structure_is_verified_even_with_matching_digest(self):
        step = next(s for s in self.steps if s.get("name") == "Verify artifact handoff")
        for extension in ("tar.gz", "zip"):
            with tempfile.TemporaryDirectory() as temp:
                work = Path(temp) / "plugin-ci"
                work.mkdir()
                archive = work / ("example-test." + extension)
                env = {**os.environ, "RUNNER_TEMP": temp, "ARCHIVE": archive.name,
                       "PUBLIC_ARCHIVE": archive.name}
                def gate():
                    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
                    (work / "SHA256SUMS").write_text(digest + "  " + archive.name + "\n")
                    return subprocess.run(["bash", "-c", step["run"]], env=env,
                                          capture_output=True)
                make_archive(archive)
                result = gate()
                self.assertEqual(result.returncode, 0, result.stderr.decode())
                archive.write_bytes(b"not an archive")
                self.assertNotEqual(gate().returncode, 0)
                for entries in ((), (("../outside", b"x"),), (("/absolute", b"x"),),
                                (("C:/absolute", b"x"),), (("folder/../outside", b"x"),),
                                (("folder\\outside", b"x"),), (("folder/.git/config", b"x"),),
                                (("folder/.env", b"x"),), (("folder/debug.pdb", b"x"),),
                                (("folder/file", b"x"), ("folder/File", b"y")),
                                (("folder", b"x"), ("folder/file", b"y")),
                                (("Folder/a", b"x"), ("folder/b", b"y"))):
                    with self.subTest(extension=extension, entries=entries):
                        make_archive(archive, entries)
                        self.assertNotEqual(gate().returncode, 0)
                if extension == "zip":
                    with zipfile.ZipFile(archive, "w") as z:
                        link = zipfile.ZipInfo("folder/link")
                        link.create_system = 3
                        link.external_attr = (stat.S_IFLNK | 0o777) << 16
                        z.writestr(link, "../outside")
                else:
                    with tarfile.open(archive, "w:gz") as t:
                        link = tarfile.TarInfo("folder/link")
                        link.type = tarfile.SYMTYPE
                        link.linkname = "../outside"
                        t.addfile(link)
                self.assertNotEqual(gate().returncode, 0)
                if extension == "zip":
                    make_archive(archive, (("folder/file", b"unique-payload-marker"),))
                    data = archive.read_bytes()
                    self.assertIn(b"unique-payload-marker", data)
                    archive.write_bytes(data.replace(b"unique-payload-marker", b"broken-payload-marker"))
                    self.assertNotEqual(gate().returncode, 0)
                make_archive(archive)
                data = archive.read_bytes()
                archive.write_bytes(data[:-8])
                self.assertNotEqual(gate().returncode, 0)

    def test_case_only_public_name_preserves_bytes_and_exact_spelling(self):
        step = next(s for s in self.steps if s.get("name") == "Verify artifact handoff")
        with tempfile.TemporaryDirectory() as temp:
            work = Path(temp) / "plugin-ci"
            work.mkdir()
            old = work / "FrameRot-test.tar.gz"
            new = work / "framerot-test.tar.gz"
            make_archive(old)
            original = old.read_bytes()
            digest = hashlib.sha256(original).hexdigest()
            manifest = work / "SHA256SUMS"
            manifest.write_text(digest + "  " + old.name + "\n")
            env = {**os.environ, "RUNNER_TEMP": temp, "ARCHIVE": old.name,
                   "PUBLIC_ARCHIVE": new.name}
            result = subprocess.run(["bash", "-c", step["run"]], env=env, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            self.assertEqual({p.name for p in work.iterdir()}, {new.name, manifest.name})
            self.assertEqual(new.read_bytes(), original)
            self.assertEqual(manifest.read_text(), digest + "  " + new.name + "\n")

    def test_case_insensitive_alias_does_not_count_as_collision(self):
        step = next(s for s in self.steps if s.get("name") == "Verify artifact handoff")
        script = step["run"].split("<<'PYTHON'\n", 1)[1].rsplit("\nPYTHON", 1)[0]
        with tempfile.TemporaryDirectory() as temp:
            work = Path(temp) / "plugin-ci"
            work.mkdir()
            old = work / "FrameRot-test.tar.gz"
            new = work / "framerot-test.tar.gz"
            make_archive(old)
            digest = hashlib.sha256(old.read_bytes()).hexdigest()
            (work / "SHA256SUMS").write_text(digest + "  " + old.name + "\n")
            env = {**os.environ, "RUNNER_TEMP": temp, "ARCHIVE": old.name,
                   "PUBLIC_ARCHIVE": new.name}
            # Model a case-insensitive filesystem's alias lookup on Linux.
            prefix = """
from pathlib import Path
from unittest.mock import patch
import os
old = Path(os.environ["RUNNER_TEMP"]) / "plugin-ci" / os.environ["ARCHIVE"]
new = old.with_name(os.environ["PUBLIC_ARCHIVE"])
exists = Path.exists
samefile = Path.samefile
def alias_exists(path):
    return exists(old) if path.name == new.name and not exists(path) else exists(path)
def alias_samefile(path, other):
    if {path.name, other.name} == {old.name, new.name} and exists(old) and not exists(new):
        return True
    return samefile(path, other)
with patch.object(Path, "exists", alias_exists), patch.object(Path, "samefile", alias_samefile):
    exec(SCRIPT)
"""
            result = subprocess.run(["python3", "-c", prefix.replace("SCRIPT", repr(script))],
                                    env=env, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            self.assertEqual({p.name for p in work.iterdir()}, {new.name, "SHA256SUMS"})

    def test_source_archive_handoff_and_exact_upload(self):
        step=next(s for s in self.steps if s.get("name")=="Verify source package")
        upload=next(s for s in self.steps if s.get("name")=="Upload verified source package")
        self.assertEqual(upload["with"]["if-no-files-found"],"error")
        self.assertEqual(upload["with"]["path"].splitlines(),[
            "${{ runner.temp }}/plugin-ci-source/${{ env.PUBLIC_NAME }}-source.tar.gz",
            "${{ runner.temp }}/plugin-ci-source/SHA256SUMS",
            "${{ runner.temp }}/plugin-ci-source/PAYLOAD-SHA256SUMS"])
        with tempfile.TemporaryDirectory() as temp:
            p=Path(temp)/"plugin-ci-source";p.mkdir()
            archive=p/"framerot-source.tar.gz"
            archive.write_bytes(b"verified source archive")
            manifest=p/"SHA256SUMS"
            manifest.write_text(hashlib.sha256(archive.read_bytes()).hexdigest()+"  "+archive.name+"\n")
            payload=p/"PAYLOAD-SHA256SUMS"
            payload.write_text("a"*64+"  framerot/bin/64bit/framerot.so\n")
            env={**os.environ,"RUNNER_TEMP":temp,"PUBLIC_NAME":"framerot"}
            def gate():
                return subprocess.run(["bash","-c",step["run"]],env=env,capture_output=True).returncode
            self.assertEqual(gate(),0)
            digest = "a" * 64
            for invalid in [
                "\n", digest + "  ../outside\n", digest + "  /absolute\n",
                digest + "  C:/absolute\n", digest + "  folder/../outside\n",
                digest + "  folder//file\n",
                digest + "  folder/file\n" + digest + "  folder/file\n",
            ]:
                with self.subTest(invalid=invalid):
                    payload.write_text(invalid)
                    self.assertNotEqual(gate(),0)
            payload.write_text(digest+"  framerot/bin/64bit/framerot.so\n")
            archive.write_bytes(b"corrupted")
            self.assertNotEqual(gate(),0)
            archive.write_bytes(b"verified source archive")
            payload.unlink()
            self.assertNotEqual(gate(),0)

if __name__ == "__main__":
    unittest.main()
