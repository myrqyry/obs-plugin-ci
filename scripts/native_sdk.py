"""Native SDK bootstrap from hosted-qualified EmuLens b72651e (Windows/macOS)."""
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import urllib.request
import urllib.error
import http.client
import tempfile
import time

DEFAULT_VERSION = "32.2.0"
PINS = json.loads(Path(__file__).with_name("obs-sdk-pins.json").read_text(encoding="utf-8"))


def sdk_pin(version):
    if version not in PINS:
        raise ValueError(f"Unsupported OBS SDK version: {version}")
    return PINS[version]


def run(*args, env=None, cwd=None):
    print("+", *args, flush=True)
    subprocess.run([str(a) for a in args], check=True, env=env, cwd=cwd)


def digest(path):
    with path.open("rb") as f:
        return hashlib.file_digest(f, "sha256").hexdigest()


def download(url, path, expected, *, timeout=120):
    request = urllib.request.Request(url, headers={"User-Agent": "obs-plugin-ci"})
    for attempt in range(1, 4):
        temporary = None
        try:
            with tempfile.NamedTemporaryFile(dir=path.parent, prefix=path.name + ".", delete=False) as dst:
                temporary = Path(dst.name)
                with urllib.request.urlopen(request, timeout=timeout) as src:
                    shutil.copyfileobj(src, dst)
            if digest(temporary) != expected:
                raise RuntimeError(f"SHA-256 mismatch: {path.name}")
            temporary.replace(path)
            return
        except urllib.error.HTTPError as exc:
            if attempt == 3 or exc.code not in (408, 429, 500, 502, 503, 504):
                raise
        except (urllib.error.URLError, TimeoutError, ConnectionError, http.client.IncompleteRead):
            if attempt == 3:
                raise
        finally:
            if temporary is not None:
                temporary.unlink(missing_ok=True)
        print(f"Transient download failure for {path.name}; retry {attempt + 1}/3", flush=True)
        time.sleep(2 ** attempt)


def source_archive(work, version):
    pin = sdk_pin(version)
    archive = work / "obs.tar.gz"
    download(f"https://github.com/obsproject/obs-studio/archive/refs/tags/{version}.tar.gz",
             archive, pin["source_sha256"])
    with tarfile.open(archive) as tar:
        # The source archive contains symlinks to unused upstream shell
        # scripts. Windows need not have symlink privileges to extract them.
        members = [m for m in tar.getmembers() if not m.issym() and not m.islnk()]
        tar.extractall(work, members=members, filter="data")
    return work / f"obs-studio-{version}"


def patch_once(path, old, new):
    """Apply only an exact, reviewed change to the checksum-pinned SDK."""
    data = path.read_bytes()
    old, new = old.encode(), new.encode()
    if data.count(old) != 1:
        raise RuntimeError(f"Unexpected OBS source: {path}; expected exactly one occurrence of {old!r}")
    path.write_bytes(data.replace(old, new, 1))


def patch_obs_sdk(obs, target, version=DEFAULT_VERSION):
    sdk_pin(version)
    if target == "windows-x64":
        # OBS 32.2.0's boolean list getter reads the double union member,
        # although add_item/insert_item store the boolean member.
        path = obs / "libobs/obs-properties.c"
        if version == DEFAULT_VERSION:
            patch_once(path, "data->items.array[idx].d : false;",
                       "data->items.array[idx].b : false;")
        elif path.read_bytes().count(b"data->items.array[idx].b : false;") != 1:
            raise RuntimeError(f"Unexpected OBS {version} source: {path}")
    elif target.startswith("macos-"):
        # The installed framework's Resources symlink causes CMake's
        # generated _IMPORT_PREFIX to resolve inside libobs.framework.
        # Export package metadata outside the framework, leaving its
        # actual binary and public headers in their standard locations.
        patch_once(obs / "cmake/common/helpers_common.cmake",
                   'set(package_destination "Frameworks/${target}.framework/Resources/cmake")',
                   'set(package_destination "${OBS_CMAKE_DESTINATION}/${target}")')


def config_dir(root, package):
    matches = list(root.rglob(f"{package}Config.cmake"))
    if len(matches) != 1:
        raise RuntimeError(f"Expected one {package} config in {root}, found {matches}")
    return matches[0].parent


def dependency_dirs(obs, target, version=DEFAULT_VERSION):
    release = sdk_pin(version)["dependencies"]
    # OBS also builds/downloads x86 helper dependencies on Windows.
    # Only the native x64 (or universal macOS) pair belongs on the
    # plugin's CMake prefix and runtime library search paths.
    if target == "windows-x64":
        arch = "x64"
    elif target in ("macos-arm64", "macos-x86_64"):
        # Both architectures use the pinned universal archive.
        arch = "universal"
    else:
        raise RuntimeError(f"Unsupported dependency target: {target}")
    base = obs / ".deps"
    deps = (base / f"obs-deps-{release}-{arch}",
            base / f"obs-deps-qt6-{release}-{arch}")
    for path, label in zip(deps, ("OBS", "Qt")):
        if not path.is_dir():
            raise RuntimeError(f"Missing pinned {label} dependency directory: {path}")
    return deps


def qt_test_platform(qt, target):
    if target == "windows-x64":
        # The pinned Windows Qt archive includes minimal/windows/direct2d,
        # not offscreen. Fail before launching a GUI executable if absent.
        plugin = qt / "plugins" / "platforms" / "qminimal.dll"
        if not plugin.is_file():
            raise RuntimeError(f"Missing headless Qt platform plugin: {plugin}")
        return "minimal"
    return "offscreen"


def native_config():
    host = platform.system()
    arch = platform.machine().lower()
    if host == "Windows" and arch in ("amd64", "x86_64"):
        return "windows-x64", ["-G", "Visual Studio 18 2026", "-A", "x64,version=10.0.26100.0"]
    if host == "Darwin" and arch in ("arm64", "x86_64"):
        return f"macos-{arch}", ["-G", "Xcode", f"-DCMAKE_OSX_ARCHITECTURES={arch}",
                                 "-DCMAKE_OSX_DEPLOYMENT_TARGET=13.0",
                                 "-DCMAKE_XCODE_ATTRIBUTE_CODE_SIGNING_ALLOWED=NO"]
    raise RuntimeError(f"Unsupported native host: {host}/{arch}")


def mac_arch_command(binary, target):
    arch = target.removeprefix("macos-")
    return ("lipo", binary, "-verify_arch", arch)
