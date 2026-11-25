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
