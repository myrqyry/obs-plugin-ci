import importlib.util
import json
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

    @unittest.skipUnless(platform.system() == "Linux", "Linux SDK fingerprint")
    def test_sdk_cache_key_uses_caller_dependency_environment(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            engine = root / "engine"
            script = engine / "scripts/run-linux-caller.sh"
            script.parent.mkdir(parents=True)
            shutil.copyfile(ROOT / "scripts/run-linux-caller.sh", script)
            binary = engine / "target/release/obs-ci"
            binary.parent.mkdir(parents=True)
            binary.symlink_to(ROOT / "target/debug/obs-ci")
            caller_root = root / "caller"
            hook = caller_root / ".github/scripts/ci-project.sh"
            hook.parent.mkdir(parents=True)
            hook.write_text('ci_dependencies(){ touch "$CI_WORK/dependencies-ran"; }\n'
                            'ci_build(){ touch "$CI_WORK/unexpected-build"; }\n')
            env = {**os.environ, "TARGET": "linux-x86_64", "MODULE": "example",
                   "PUBLIC_NAME": "example", "ARCHIVE": "example-linux-x86_64-obs32.2.0-test.tar.gz",
                   "MINIMUM_TESTS": "1", "OBS_VERSION": "32.2.0", "RUNNER_TEMP": str(root),
                   "OBS_CI_LINUX_SDK_CACHE": "true", "ENGINE_REVISION": "a" * 40,
                   "GITHUB_OUTPUT": str(root / "output")}
            for key in ("OBS_CI_LOCAL_SDK", "OBS_CI_LOCAL_BUILD"):
                env.pop(key, None)
            cmd = ["bash", str(script), str(caller_root)]
            result = subprocess.run([*cmd, "--sdk-cache-key"], env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            work = root / "caller-work"
            self.assertTrue((work / "dependencies-ran").is_file())
            fingerprint = subprocess.check_output([str(binary), "sdk-fingerprint", "--obs-version", "32.2.0",
                          "--recipe", "legacy-core", "--work", str(work / "obs")], text=True)
            key = json.loads(fingerprint)["cache_key"]
            self.assertEqual((root / "output").read_text().strip(), f"key=obs-ci-sdk-v1-{'a' * 40}-{key}")
            # A partial restored cache must fail closed before compiling a plugin.
            (work / "obs/sdk").mkdir(parents=True)
            (work / "obs/SDK-RECEIPT.json").write_text("{}")
            result = subprocess.run(cmd, env=env, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((work / "unexpected-build").exists())
            self.assertEqual((work / "obs/SDK-RECEIPT.json").read_text(), "{}")

    @unittest.skipUnless(platform.system() == "Linux", "Linux SDK-only adapter")
    def test_sdk_only_preserves_specialized_verifier_boundary(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            engine = root / "engine"
            script = engine / "scripts/run-linux-caller.sh"
            script.parent.mkdir(parents=True)
            shutil.copyfile(ROOT / "scripts/run-linux-caller.sh", script)
            binary = engine / "target/release/obs-ci"
            binary.parent.mkdir(parents=True)
            # Stub only the SDK acquisition boundary; the shell driver and
            # caller hooks are real. Actual SDK builds are separate pilots.
            binary.write_text("""#!/usr/bin/env bash
set -eu
if [[ "$1" == sdk-fingerprint ]]; then
  echo '{"cache_key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}'
elif [[ "$1" == sdk-prepare ]]; then
  if [[ -e "$RUNNER_TEMP/caller-work/obs/rejected" ]]; then
    echo "fixture invalid restored SDK" >&2; exit 82
  fi
  sdk="$RUNNER_TEMP/caller-work/obs/sdk"
  mkdir -p "$sdk/lib/cmake/libobs" "$sdk/include/obs"
  touch "$sdk/lib/cmake/libobs/libobsConfig.cmake"
  printf '32.2.0\n' > "$sdk/OBS_SDK_VERSION"
  printf '#define LIBOBS_API_MAJOR_VER 32\n#define LIBOBS_API_MINOR_VER 2\n' > "$sdk/include/obs/obs-config.h"
  echo '{}'
else
  exit 80
fi
""")
            binary.chmod(0o755)
            caller_root = root / "caller"
            hook = caller_root / ".github/scripts/ci-project.sh"
            hook.parent.mkdir(parents=True)
            hook.write_text('ci_dependencies(){ touch "$CI_WORK/dependencies-ran"; }\n'
                            'ci_build(){ touch "$CI_WORK/unexpected-build"; exit 81; }\n')
            env = {**os.environ, "TARGET": "linux-x86_64", "MODULE": "example",
                   "PUBLIC_NAME": "example", "ARCHIVE": "example-linux-x86_64-obs32.2.0-test.tar.gz",
                   "MINIMUM_TESTS": "1", "OBS_VERSION": "32.2.0", "RUNNER_TEMP": str(root),
                   "OBS_CI_LINUX_SDK_CACHE": "true", "ENGINE_REVISION": "a" * 40,
                   "GITHUB_OUTPUT": str(root / "output")}
            for key in ("OBS_CI_LOCAL_SDK", "OBS_CI_LOCAL_BUILD"):
                env.pop(key, None)
            cmd = ["bash", str(script), str(caller_root)]
            subprocess.run([*cmd, "--sdk-cache-key"], env=env, check=True, capture_output=True)
            result = subprocess.run([*cmd, "--sdk-only"], env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            work = root / "caller-work"
            self.assertTrue((work / "obs/sdk/OBS_SDK_VERSION").is_file())
            self.assertFalse((work / "unexpected-build").exists())
            self.assertFalse((root / "plugin-ci").exists())
            # A rejected restore is quarantined; a fresh build replaces it
            # only in the explicit cache preparation phase.
            (work / "obs/rejected").write_text("bad restored bytes")
            result = subprocess.run([*cmd, "--sdk-cache-restore"], env=env,
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            quarantined = list(work.glob("rejected-sdk.*/rejected"))
            self.assertEqual(len(quarantined), 1)
            self.assertEqual(quarantined[0].read_text(), "bad restored bytes")
            self.assertFalse((work / "obs/rejected").exists())
            self.assertFalse((work / "unexpected-build").exists())
            # A modified preflight fingerprint must block SDK use.
            (work / "sdk-fingerprint.json").write_text("{}")
            self.assertNotEqual(subprocess.run([*cmd, "--sdk-only"], env=env,
                                              capture_output=True).returncode, 0)
            self.assertFalse((work / "unexpected-build").exists())

    def test_configure_hook_contract(self):
        from types import SimpleNamespace
        args = (Path("caller"), Path("work"), "windows-x64", "32.2.0")
        for value in (None, "-DFOO=ON", b"-DFOO=ON", ["-DFOO=ON", 42], [""], ["bad" + chr(0)], {"-DFOO=ON": True}):
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, "configure"):
                caller.configure_args(SimpleNamespace(configure=lambda *a: value), *args)
        hook = SimpleNamespace(configure=lambda *a: iter(["-DFOO=ON", "-DBAR=OFF"]))
        self.assertEqual(caller.configure_args(hook, *args), ["-DFOO=ON", "-DBAR=OFF"])

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
