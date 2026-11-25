use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Path, process::Command};
use tempfile::tempdir;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_obs-ci"))
}

#[test]
fn full_matrix_and_linux_subset() {
    for (profile, count) in [("full", 8), ("linux-obs32", 1)] {
        let result = cli()
            .args([
                "plan",
                "--module",
                "example",
                "--minimum-tests",
                "9",
                "--profile",
                profile,
            ])
            .output()
            .unwrap();
        assert!(result.status.success(), "{:?}", result);
        let json: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(json["include"].as_array().unwrap().len(), count);
        assert_eq!(json["include"][0]["target"], "linux-x86_64");
        assert_eq!(json["include"][0]["obs"], "32.2.0");
    }
}

#[test]
fn plan_rejects_invalid_inputs() {
    for args in [
        vec!["--module", "../bad", "--minimum-tests", "1"],
        vec!["--module", "ok", "--minimum-tests", "0"],
        vec![
            "--module",
            "ok",
            "--minimum-tests",
            "1",
            "--profile",
            "unknown",
        ],
        vec!["--module", "ok", "--minimum-tests", "1", "--source-package"],
        vec![
            "--module",
            "ok",
            "--minimum-tests",
            "1",
            "--package-name",
            "bad/name",
        ],
    ] {
        assert!(
            !cli()
                .arg("plan")
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
}

fn write_manifest(path: &Path) {
    let digest = Sha256::digest(fs::read(path).unwrap());
    fs::write(
        path.with_file_name("SHA256SUMS"),
        format!(
            "{digest:x}  {}\n",
            path.file_name().unwrap().to_str().unwrap()
        ),
    )
    .unwrap();
}

fn zip_package(path: &Path, names: &[&str]) {
    let mut z = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for name in names {
        z.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(b"module").unwrap();
    }
    z.finish().unwrap();
    write_manifest(path);
}

#[test]
fn audit_is_read_only_and_rejects_invalid_archives() {
    let dir = tempdir().unwrap();
    let archive = dir.path().join("example-test.zip");
    zip_package(&archive, &["example/bin/plugin.dll"]);
    let original = fs::read(&archive).unwrap();
    let manifest = fs::read(archive.with_file_name("SHA256SUMS")).unwrap();
    let result = cli().arg("audit").arg(&archive).output().unwrap();
    assert!(result.status.success(), "{:?}", result);
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(json["regular_files"], 1);
    assert_eq!(fs::read(&archive).unwrap(), original);
    assert_eq!(
        fs::read(archive.with_file_name("SHA256SUMS")).unwrap(),
        manifest
    );
    fs::write(&archive, b"not an archive").unwrap();
    assert!(
        !cli()
            .arg("audit")
            .arg(&archive)
            .output()
            .unwrap()
            .status
            .success()
    );
    write_manifest(&archive);
    assert!(
        !cli()
            .arg("audit")
            .arg(&archive)
            .output()
            .unwrap()
            .status
            .success()
    );
    for names in [
        vec![],
        vec!["../outside"],
        vec!["example/.git/config"],
        vec!["example/File", "example/file"],
        vec!["example", "example/file"],
    ] {
        zip_package(&archive, &names);
        assert!(
            !cli()
                .arg("audit")
                .arg(&archive)
                .output()
                .unwrap()
                .status
                .success(),
            "{names:?}"
        );
    }
}

#[test]
fn matrix_filters_and_source_host_are_deterministic() {
    let result = cli()
        .args([
            "plan",
            "--module",
            "example",
            "--minimum-tests",
            "1",
            "--driver",
            "caller",
            "--source-package",
            "--targets",
            "windows-x64,linux-x86_64",
            "--obs-versions",
            "33.0.3",
        ])
        .output()
        .unwrap();
    assert!(result.status.success(), "{:?}", result);
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let rows = json["include"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["target"], "linux-x86_64");
    assert_eq!(rows[0]["obs"], "33.0.3");
    assert_eq!(rows[0]["emit_source"], true);
    assert_eq!(rows[1]["emit_source"], false);
    for args in [
        vec![
            "--targets",
            "windows-x64",
            "--source-package",
            "--driver",
            "caller",
        ],
        vec!["--targets", "bogus"],
        vec!["--targets", "linux-x86_64,linux-x86_64"],
        vec!["--obs-versions", "33.0.3", "--profile", "linux-obs32"],
    ] {
        assert!(
            !cli()
                .args(["plan", "--module", "example", "--minimum-tests", "1"])
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
}

fn tar_package(path: &Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut archive = tar::Builder::new(encoder);
    for (name, data) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        archive.append_data(&mut header, name, *data).unwrap();
    }
    archive.into_inner().unwrap().finish().unwrap();
    write_manifest(path);
}

#[test]
fn source_audit_checks_manifest_and_binary_payload() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("source");
    let binary_dir = dir.path().join("binary");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&binary_dir).unwrap();
    let source = source_dir.join("example-source.tar.gz");
    let binary = binary_dir.join("example-test.tar.gz");
    tar_package(
        &source,
        &[("example/COPYING", b"license"), ("example/main.c", b"code")],
    );
    tar_package(&binary, &[("example/bin/plugin.so", b"module")]);
    let digest = Sha256::digest(b"module");
    let payload = source_dir.join("PAYLOAD-SHA256SUMS");
    let valid = format!("{digest:x}  example/bin/plugin.so\n");
    fs::write(&payload, &valid).unwrap();
    let run = || {
        cli()
            .arg("audit-source")
            .arg(&source)
            .arg("--binary")
            .arg(&binary)
            .output()
            .unwrap()
    };
    let result = run();
    assert!(result.status.success(), "{:?}", result);
    let receipt: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(receipt["payload_entries"], 1);
    assert_eq!(receipt["payload_verified"], true);
    for bad in [
        String::new(),
        format!("{digest:x}  ../escape\n"),
        format!("{valid}{valid}"),
        format!("{:064}  example/bin/plugin.so\n", 0),
        format!("{digest:x}  example/other.so\n"),
        format!("{valid}{digest:x}  example/extra.so\n"),
    ] {
        fs::write(&payload, bad).unwrap();
        assert!(!run().status.success());
    }
    fs::write(&payload, &valid).unwrap();
    tar_package(
        &source,
        &[
            ("example/COPYING", b"license"),
            ("example/module.so.1", b"binary"),
        ],
    );
    assert!(!run().status.success());
    tar_package(&source, &[("example/main.c", b"code")]);
    assert!(!run().status.success());
    tar_package(
        &source,
        &[("example/COPYING", b"license"), ("other/main.c", b"code")],
    );
    assert!(!run().status.success());
}
