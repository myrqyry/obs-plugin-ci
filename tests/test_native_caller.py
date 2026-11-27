import importlib.util
import os
import shutil
import subprocess
import platform
from pathlib import Path
import plistlib
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import native_caller as caller

class NativeCallerTests(unittest.TestCase):
    @unittest.skipUnless(platform.system() == "Linux", "Linux caller entrypoint")
    def test_linux_public_alias_preserves_module_archive(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            engine = root / "engine"
            script = engine / "scripts/run-linux-caller.sh"
            script.parent.mkdir(parents=True)
            shutil.copyfile(ROOT / "scripts/run-linux-caller.sh", script)
            binary = engine / "target/release/obs-ci"
            binary.parent.mkdir(parents=True)
            binary.write_text("#!/bin/sh\n")
            binary.chmod(0o755)
            caller_root = root / "caller"
            hook = caller_root / ".github/scripts/ci-project.sh"
            hook.parent.mkdir(parents=True)
            hook.write_text('ci_dependencies(){ printf "%s|%s|%s" "$MODULE" "$PUBLIC_NAME" "$ARCHIVE"; exit 42; }\n')
            env = {**os.environ, "TARGET": "linux-x86_64", "MODULE": "example",
                   "PUBLIC_NAME": "public-example", "ARCHIVE": "example-linux-x86_64-obs32.2.0-test.tar.gz",
                   "MINIMUM_TESTS": "1", "OBS_VERSION": "32.2.0", "RUNNER_TEMP": str(root)}
            env.pop("OBS_CI_LOCAL_SDK", None)
            result = subprocess.run(["bash", str(script), str(caller_root)], env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 42, result.stderr)
            self.assertEqual(result.stdout, "example|public-example|example-linux-x86_64-obs32.2.0-test.tar.gz")
            (root / "invalid").mkdir()
            result = subprocess.run(["bash", str(script), str(caller_root)],
                                    env={**env, "PUBLIC_NAME": "../bad", "RUNNER_TEMP": str(root / "invalid")}, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 42)

    def test_native_layouts(self):
        for version in ("32.2.0", "33.0.3"):
            for target in ("windows-x64", "macos-arm64", "macos-x86_64"):
                with self.subTest(version=version, target=target), tempfile.TemporaryDirectory() as tmp:
                    stage = Path(tmp)
                    binary, data, notice = caller.layout(stage, "example", target, version)
                    binary.parent.mkdir(parents=True)
                    binary.write_bytes(b"native fixture")
                    data.mkdir(parents=True)
                    (data / "locale.ini").write_bytes(b"locale")
                    notice.write_bytes(b"license")
                    if target.startswith("macos"):
                        info = stage / "example.plugin/Contents/Info.plist"
                        info.write_bytes(plistlib.dumps({"CFBundleExecutable": "example", "CFBundlePackageType": "BNDL"}))
                    expected = {"locale.ini": b"locale"}
                    self.assertEqual(caller.verify_payload(stage, "example", target, version, expected, b"license"), binary)
                    (data / "unexpected").write_bytes(b"extra")
                    with self.assertRaises(ValueError):
                        caller.verify_payload(stage, "example", target, version, expected, b"license")
    def test_reject_wrong_layout_and_plist(self):
        with tempfile.TemporaryDirectory() as tmp:
            stage = Path(tmp)
            binary, data, notice = caller.layout(stage, "example", "windows-x64", "32.2.0")
            self.assertEqual(binary.relative_to(stage).as_posix(), "example/bin/64bit/example.dll")
            self.assertEqual(caller.layout(stage, "example", "windows-x64", "33.0.3")[0].relative_to(stage).as_posix(), "example/example.dll")
            with self.assertRaises(ValueError):
                caller.layout(stage, "../escape", "windows-x64", "33.0.3")
            with self.assertRaises(ValueError):
                caller.layout(stage, "example", "linux-x86_64", "32.2.0")
            with self.assertRaises(ValueError):
                caller.layout(stage, "example", "windows-x64", "31.0.0")
            binary, data, notice = caller.layout(stage, "example", "macos-arm64", "33.0.3")
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"native")
            data.mkdir(parents=True)
            notice.write_bytes(b"license")
            (binary.parent.parent / "Info.plist").write_bytes(plistlib.dumps({"CFBundleExecutable": "other", "CFBundlePackageType": "BNDL"}))
            with self.assertRaises(ValueError):
                caller.verify_payload(stage, "example", "macos-arm64", "33.0.3", {}, b"license")

if __name__ == "__main__":
    unittest.main()
