import importlib.util
from pathlib import Path
import plistlib
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import native_caller as caller

class NativeCallerTests(unittest.TestCase):
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
