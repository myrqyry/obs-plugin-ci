"""Build native caller modules with the shared pinned SDK and audited packaging.

Callers explicitly own configure options and the installed payload policy.
Linux runtime tests remain in their existing caller. Native legs compile and
inspect the production module; they do not claim runtime-test coverage.
"""
import importlib.util
import os
from pathlib import Path
import plistlib
import re
import sys

import native_sdk as sdk


def layout(stage, module, target, version):
    sdk.sdk_pin(version)
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", module):
        raise ValueError("Invalid module name")
    if target == "windows-x64":
        root = stage / module
        binary = root / "bin/64bit" / (module + ".dll") if version == "32.2.0" else root / (module + ".dll")
        return binary, root / "data", root / "LICENSE"
    if target in ("macos-arm64", "macos-x86_64"):
        contents = stage / (module + ".plugin") / "Contents"
        return contents / "MacOS" / module, contents / "Resources", contents / "Resources/LICENSE"
    raise ValueError("Unsupported native target")


def verify_payload(stage, module, target, version, assets, license_bytes, *, license_name="LICENSE"):
    binary, data, notice = layout(stage, module, target, version)
    notice = notice.with_name(license_name)
    if not binary.is_file() or binary.stat().st_size == 0:
        raise ValueError("Missing native module")
    if license_bytes is not None and (not notice.is_file() or notice.read_bytes() != license_bytes):
        raise ValueError("Missing or changed license")
    expected = {binary}
    if license_bytes is not None:
        expected.add(notice)
    for name, value in assets.items():
        path = data / name
        if not path.is_file() or path.read_bytes() != value:
            raise ValueError(f"Missing or changed asset: {name}")
        expected.add(path)
    if target.startswith("macos-"):
        info = binary.parent.parent / "Info.plist"
        values = plistlib.loads(info.read_bytes())
        if values.get("CFBundleExecutable") != module or values.get("CFBundlePackageType") != "BNDL":
            raise ValueError("Incorrect bundle metadata")
        expected.add(info)
    entries = list(stage.rglob("*"))
    if any(p.is_symlink() for p in entries):
        raise ValueError("Unexpected symlink")
    actual = {p for p in entries if p.is_file()}
    if actual != expected:
        raise ValueError(f"Unexpected native package files: {actual ^ expected}")
    return binary


def load_hook(root):
    path = root / ".github/scripts/ci-native.py"
    spec = importlib.util.spec_from_file_location("project_native", path)
    hook = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(hook)
    return hook


def configure_args(hook, root, work, target, version):
    """Validate the caller hook result before passing it to CMake."""
    value = hook.configure(root, work, target, version)
    if isinstance(value, (str, bytes, dict)):
        raise ValueError("Caller configure hook must return an iterable of argument strings")
    try:
        args = list(value)
    except TypeError as exc:
        raise ValueError("Caller configure hook must return an iterable of argument strings") from exc
    if any(not isinstance(arg, str) or not arg or chr(0) in arg for arg in args):
        raise ValueError("Caller configure hook returned an invalid argument string")
    return args


def main():
    root = Path(sys.argv[1]).resolve()
    module, target, version, archive = (os.environ[k] for k in ("MODULE", "TARGET", "OBS_VERSION", "ARCHIVE"))
    sdk.sdk_pin(version)
    host, generator = sdk.native_config()
    if host != target or archive != f"{module}-{target}-obs{version}-test.zip":
        raise ValueError("Host or archive does not match matrix")
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", module):
        raise ValueError("Invalid module")
    engine = Path(__file__).resolve().parents[1]
    cli = engine / "target/release" / ("obs-ci.exe" if os.name == "nt" else "obs-ci")
    if not cli.is_file():
        raise ValueError("Shared Rust engine was not built")
    hook = load_hook(root)
    if hook.MODULE != module:
        raise ValueError("Caller module mismatch")
    work = Path(os.environ["RUNNER_TEMP"]) / "native-caller"
    work.mkdir()
    obs = sdk.source_archive(work, version)
    sdk.patch_obs_sdk(obs, target, version)
    prefix = work / "sdk"
    obs_build = work / "obs-build"
    sdk.run("cmake", "-S", obs, "-B", obs_build, *generator,
        "-DOBS_CMAKE_VERSION:STRING=3.0.0", f"-DOBS_VERSION_OVERRIDE:STRING={version}",
        "-DENABLE_PLUGINS=OFF", "-DENABLE_FRONTEND=OFF",
        "-DENABLE_SCRIPTING=OFF", "-DENABLE_BROWSER=OFF")
    sdk.run("cmake", "--build", obs_build, "--config", "Release",
            "--target", "obs-frontend-api", "--parallel", "4")
    sdk.run("cmake", "--install", obs_build, "--config", "Release",
            "--component", "Development", "--prefix", prefix)
    deps = sdk.dependency_dirs(obs, target, version)
    build = work / "build"
    stage = work / "stage"
    installed = work / "installed"
    sdk.run("cmake", "-S", root, "-B", build, *generator,
            "-DCMAKE_BUILD_TYPE=RelWithDebInfo",
            "-DCMAKE_PREFIX_PATH=" + ";".join(p.as_posix() for p in [prefix, *deps]),
            "-Dlibobs_DIR=" + sdk.config_dir(prefix, "libobs").as_posix(),
            "-Dobs-frontend-api_DIR=" + sdk.config_dir(prefix, "obs-frontend-api").as_posix(),
            *configure_args(hook, root, work, target, version))
    sdk.run("cmake", "--build", build, "--config", "RelWithDebInfo", "--parallel", "4")
    sdk.run("cmake", "--install", build, "--config", "RelWithDebInfo", "--prefix", installed)
    binary = hook.stage(root, installed, stage, target, version)
    if binary != layout(stage, module, target, version)[0]:
        raise ValueError("Wrong native binary layout")
    if target.startswith("macos-"):
        sdk.run(*sdk.mac_arch_command(binary, target))
        sdk.run("otool", "-L", binary)
        sdk.run("codesign", "--force", "--sign", "-", stage / f"{module}.plugin")
        sdk.run("codesign", "--verify", "--strict", stage / f"{module}.plugin")
    output = Path(os.environ["RUNNER_TEMP"]) / "plugin-ci"
    sdk.run(cli, "pack", "--stage", stage, "--output", output, "--archive-name", archive)
    sdk.run(cli, "inspect-module", output / archive, "--module", module, "--target", target, "--obs-version", version)
    print(f"{target} OBS {version}: native compile and verified package; runtime tests are separate.")


if __name__ == "__main__":
    # Let caller hooks import this module without creating a second instance.
    sys.modules["native_caller"] = sys.modules[__name__]
    main()
