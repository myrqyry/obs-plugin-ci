"""Drive the Rust qualification fixture; no OBS SDK or publication involved."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
MODULE = "fixture"
PUBLIC = "rust-fixture"
SOURCE_NAME = f"{PUBLIC}-source"
TARGETS = ("linux-x86_64", "windows-x64", "macos-arm64", "macos-x86_64")
VERSIONS = ("32.2.0", "33.0.3")
ENGINE = ROOT / "target/release" / ("obs-ci.exe" if os.name == "nt" else "obs-ci")


def engine_call(engine, *args):
    result = subprocess.run([str(engine), *map(str, args)], check=True,
                            stdout=subprocess.PIPE, text=True, encoding="utf-8")
    return json.loads(result.stdout)


def plan(engine, targets="", versions=""):
    args = ["plan", "--module", MODULE, "--package-name", PUBLIC,
            "--minimum-tests", "1", "--driver", "caller"]
    if targets:
        args += ["--targets", targets]
    if versions:
        args += ["--obs-versions", versions]
    if not targets or "linux-x86_64" in targets.split(","):
        args.append("--source-package")
    return engine_call(engine, *args)


def archive_name(name, target, obs):
    extension = "tar.gz" if target == "linux-x86_64" else "zip"
    return f"{name}-{target}-obs{obs}-test.{extension}"


def artifact_name(target, obs):
    return f"{PUBLIC}-{target}-obs{obs}-test"


def module_path(target, obs):
    if target.startswith("macos-"):
        return f"{MODULE}.plugin/Contents/MacOS/{MODULE}"
    suffix = "so" if target == "linux-x86_64" else "dll"
    middle = "bin/64bit/" if obs == "32.2.0" else ""
    return f"{MODULE}/{middle}{MODULE}.{suffix}"


def write_json(path, data):
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n",
                    encoding="utf-8", newline="\n")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def manifest(archive):
    archive.with_name("SHA256SUMS").write_text(
        f"{digest(archive.read_bytes())}  {archive.name}\n", encoding="utf-8", newline="\n")


def package(path, entries):
    if path.suffix == ".zip":
        with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as output:
            for name, content in entries.items():
                member = zipfile.ZipInfo(name)
                member.create_system = 3
                member.external_attr = (0o100755 if "/MacOS/" in name else 0o100644) << 16
                output.writestr(member, content, compress_type=zipfile.ZIP_DEFLATED)
    else:
        with tarfile.open(path, "w:gz") as output:
            for name, content in entries.items():
                member = tarfile.TarInfo(name)
                member.size = len(content)
                member.mode = 0o644
                output.addfile(member, io.BytesIO(content))
    manifest(path)


def inspect(engine, archive, target, obs):
    return engine_call(engine, "inspect-module", archive, "--module", MODULE,
                       "--target", target, "--obs-version", obs)


def build(engine, work, target, obs, emit_source):
    if emit_source and target != "linux-x86_64":
        raise ValueError("Source fixture must be hosted on Linux")
    work.mkdir(parents=True, exist_ok=True)
    directory = work / artifact_name(target, obs)
    directory.mkdir()
    source_dir = work / SOURCE_NAME
    if emit_source:
        source_dir.mkdir()

    # Compile a real native shared library on this runner. No libobs linkage.
    with tempfile.TemporaryDirectory() as temp:
        suffix = {"linux-x86_64": ".so", "windows-x64": ".dll",
                  "macos-arm64": ".dylib", "macos-x86_64": ".dylib"}[target]
        binary = Path(temp) / (MODULE + suffix)
        subprocess.run([os.environ.get("RUSTC", "rustc"), "--crate-type", "cdylib",
                        "--crate-name", MODULE, "--edition", "2024",
                        str(ROOT / "fixtures/module.rs"), "-o", str(binary)], check=True)
        entries = {module_path(target, obs): binary.read_bytes(),
                   f"{MODULE}/COPYING" if not target.startswith("macos-")
                   else f"{MODULE}.plugin/Contents/Resources/COPYING": (ROOT / "LICENSE").read_bytes()}
        original = directory / archive_name(MODULE, target, obs)
        public = directory / archive_name(PUBLIC, target, obs)
        package(original, entries)
        engine_call(engine, "handoff", original, "--public-name", public.name)
        write_json(directory / "MODULE-RECEIPT.json", inspect(engine, public, target, obs))
        if emit_source:
            source = source_dir / f"{SOURCE_NAME}.tar.gz"
            package(source, {f"{MODULE}/COPYING": (ROOT / "LICENSE").read_bytes(),
                             f"{MODULE}/module.rs": (ROOT / "fixtures/module.rs").read_bytes()})
            (source_dir / "PAYLOAD-SHA256SUMS").write_text(
                "".join(f"{digest(data)}  {name}\n" for name, data in sorted(entries.items())),
                encoding="utf-8", newline="\n")
            write_json(source_dir / "SOURCE-RECEIPT.json",
                       engine_call(engine, "audit-source", source, "--binary", public))


def exact_files(directory, expected):
    entries = list(directory.iterdir())
    if ({entry.name for entry in entries} != set(expected)
            or any(entry.is_symlink() or not entry.is_file() for entry in entries)):
        raise ValueError(f"Unexpected artifact contents: {directory}")


def verify_receipt(path, actual):
    if json.loads(path.read_text(encoding="utf-8")) != actual:
        raise ValueError(f"Receipt mismatch: {path}")


def collect(engine, downloads, matrix, output):
    # An unsuccessful qualification must not leave a previous success report.
    if output.exists() or output.is_symlink():
        output.unlink()
    rows = matrix["include"]
    if not rows:
        raise ValueError("Empty qualification matrix")
    targets = ",".join(sorted({row["target"] for row in rows}))
    versions = ",".join(sorted({row["obs"] for row in rows}))
    if matrix != plan(engine, targets, versions):
        raise ValueError("Matrix does not match the Rust planner")
    expected = {artifact_name(row["target"], row["obs"]) for row in rows}
    host = next((row for row in rows if row["emit_source"]), None)
    if host:
        expected.add(SOURCE_NAME)
    dirs = list(downloads.iterdir())
    if ({entry.name for entry in dirs} != expected
            or any(entry.is_symlink() or not entry.is_dir() for entry in dirs)):
        raise ValueError("Downloaded artifact set does not match the planned matrix")
    modules = []
    for row in rows:
        target, obs = row["target"], row["obs"]
        directory = downloads / artifact_name(target, obs)
        archive = directory / archive_name(PUBLIC, target, obs)
        exact_files(directory, [archive.name, "SHA256SUMS", "MODULE-RECEIPT.json"])
        actual = inspect(engine, archive, target, obs)
        verify_receipt(directory / "MODULE-RECEIPT.json", actual)
        modules.append({"target": target, "obs_layout": obs, "receipt": actual})
    source = None
    if host:
        directory = downloads / SOURCE_NAME
        archive = directory / f"{SOURCE_NAME}.tar.gz"
        exact_files(directory, [archive.name, "SHA256SUMS", "PAYLOAD-SHA256SUMS", "SOURCE-RECEIPT.json"])
        target, obs = host["target"], host["obs"]
        binary = downloads / artifact_name(target, obs) / archive_name(PUBLIC, target, obs)
        source = engine_call(engine, "audit-source", archive, "--binary", binary)
        verify_receipt(directory / "SOURCE-RECEIPT.json", source)
        if not source["payload_verified"]:
            raise ValueError("Source payload not cross-verified")
    output.parent.mkdir(parents=True, exist_ok=True)
    write_json(output, {"scope": "synthetic-fixture-only", "modules": modules, "source": source})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    planner = commands.add_parser("plan")
    planner.add_argument("--targets", default="")
    planner.add_argument("--obs-versions", default="")
    builder = commands.add_parser("build")
    builder.add_argument("--work", type=Path, required=True)
    builder.add_argument("--target", choices=TARGETS, required=True)
    builder.add_argument("--obs-version", choices=VERSIONS, required=True)
    builder.add_argument("--emit-source", action="store_true")
    collector = commands.add_parser("collect")
    collector.add_argument("--downloads", type=Path, required=True)
    collector.add_argument("--matrix-json", required=True)
    collector.add_argument("--output", type=Path, required=True)
    for command in (planner, builder, collector):
        command.add_argument("--engine", type=Path, default=ENGINE)
    args = parser.parse_args()
    engine = args.engine.resolve()
    if args.command == "plan":
        print(json.dumps(plan(engine, args.targets, args.obs_versions), separators=(",", ":")))
    elif args.command == "build":
        build(engine, args.work, args.target, args.obs_version, args.emit_source)
    else:
        collect(engine, args.downloads, json.loads(args.matrix_json), args.output)


if __name__ == "__main__":
    main()
