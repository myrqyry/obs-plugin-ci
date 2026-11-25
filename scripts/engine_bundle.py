"""Same-run executable transport for the synthetic qualification workflow."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import subprocess

ROOT = Path(__file__).resolve().parents[1]
RUST_VERSION = "1.93.1"
HOSTS = {
    ("Linux", "x86_64"): "linux-x86_64",
    ("Windows", "amd64"): "windows-x64",
    ("Windows", "x86_64"): "windows-x64",
    ("Darwin", "arm64"): "macos-arm64",
    ("Darwin", "x86_64"): "macos-x86_64",
}
RUNNERS = {
    "windows-x64": "windows-2025-vs2026",
    "macos-arm64": "macos-26",
    "macos-x86_64": "macos-26-intel",
}


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def expected_metadata(target, revision):
    if HOSTS.get((platform.system(), platform.machine().lower())) != target:
        raise ValueError(f"Engine target does not match current host: {target}")
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT,
                                   text=True).strip()
    if not re.fullmatch("[0-9a-f]{40}", revision) or revision != head:
        raise ValueError("Engine revision must match checkout HEAD")
    return {"schema": 1, "target": target, "revision": revision,
            "lock_sha256": sha256(ROOT / "Cargo.lock"), "rustc": RUST_VERSION}


def binary_name(target):
    return "obs-ci.exe" if target == "windows-x64" else "obs-ci"


def pack(target, revision, binary, directory):
    metadata = expected_metadata(target, revision)
    rustc = subprocess.check_output(["rustc", "--version"], text=True).split()
    if rustc[:2] != ["rustc", RUST_VERSION]:
        raise ValueError("Unexpected Rust toolchain")
    if binary.is_symlink() or not binary.is_file() or not binary.stat().st_size:
        raise ValueError("Missing regular engine executable")
    directory.mkdir(parents=True, exist_ok=False)
    output = directory / binary_name(target)
    shutil.copyfile(binary, output)
    metadata.update(sha256=sha256(output), bytes=output.stat().st_size)
    (directory / "ENGINE.json").write_text(
        json.dumps(metadata, indent=2, sort_keys=True) + "\n",
        encoding="utf-8", newline="\n")


def verify(target, revision, directory):
    expected = expected_metadata(target, revision)
    if directory.is_symlink() or not directory.is_dir():
        raise ValueError("Invalid engine bundle directory")
    entries = list(directory.iterdir())
    name = binary_name(target)
    if ({p.name for p in entries} != {name, "ENGINE.json"}
            or any(p.is_symlink() or not p.is_file() for p in entries)):
        raise ValueError("Engine bundle must contain exactly executable and receipt")
    binary = directory / name
    if not binary.stat().st_size:
        raise ValueError("Empty engine executable")
    expected.update(sha256=sha256(binary), bytes=binary.stat().st_size)
    receipt = json.loads((directory / "ENGINE.json").read_text(encoding="utf-8"))
    if receipt != expected:
        raise ValueError("Engine receipt mismatch (revision, platform, lockfile, toolchain, size or hash)")
    # Artifact downloads do not retain executable mode. Restore it only after validation.
    if os.name != "nt":
        binary.chmod(binary.stat().st_mode | stat.S_IXUSR)
    return binary.resolve()


def build_matrix(matrix):
    targets = {row["target"] for row in matrix["include"]}
    if not targets or not targets <= set(RUNNERS) | {"linux-x86_64"}:
        raise ValueError("Unexpected engine platform selection")
    return {"include": [{"target": target, "runner": runner}
                        for target, runner in RUNNERS.items() if target in targets]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("pack", "verify"):
        command = commands.add_parser(name)
        command.add_argument("--target", required=True)
        command.add_argument("--revision", required=True)
        command.add_argument("--directory", type=Path, required=True)
        if name == "pack":
            command.add_argument("--binary", type=Path, required=True)
    matrix = commands.add_parser("matrix")
    matrix.add_argument("--matrix-json", required=True)
    args = parser.parse_args()
    if args.command == "pack":
        pack(args.target, args.revision, args.binary, args.directory)
    elif args.command == "verify":
        print(verify(args.target, args.revision, args.directory), end="")
    else:
        print(json.dumps(build_matrix(json.loads(args.matrix_json)), separators=(",", ":")))


if __name__ == "__main__":
    main()
