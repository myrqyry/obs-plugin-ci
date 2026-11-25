use crate::audit::{Receipt, audit};
use std::{error::Error, fs, io::Write, path::Path};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn valid_name(name: &str) -> bool {
    let stem = name
        .strip_suffix(".tar.gz")
        .or_else(|| name.strip_suffix(".zip"));
    let Some(stem) = stem else {
        return false;
    };
    let base = stem
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && matches!(base.as_bytes()[3], b'1'..=b'9'));
    !reserved
        && stem
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && !stem.ends_with('.')
        && stem
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

pub fn handoff(path: &Path, public_name: &str) -> Result<Receipt> {
    handoff_with(path, public_name, |replacement, manifest| {
        replacement
            .persist(manifest)
            .map(|_| ())
            .map_err(|err| err.error)
    })
}

fn handoff_with(
    path: &Path,
    public_name: &str,
    replace_manifest: impl FnOnce(tempfile::NamedTempFile, &Path) -> std::io::Result<()>,
) -> Result<Receipt> {
    let mut receipt = audit(path)?;
    if !valid_name(public_name)
        || receipt.archive.ends_with(".zip") != public_name.ends_with(".zip")
    {
        return Err("Invalid public archive filename or mismatched extension".into());
    }
    let work = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let exact_source = fs::read_dir(work)?.collect::<std::io::Result<Vec<_>>>()?;
    if !exact_source
        .iter()
        .any(|entry| entry.file_name() == receipt.archive.as_str())
    {
        return Err("Archive path must match exact on-disk filename".into());
    }
    if receipt.archive == public_name {
        return Ok(receipt);
    }
    let public = work.join(public_name);
    // Reject a distinct directory entry, including a hard link to the source.
    // A case-only alias of the original entry is permitted on insensitive filesystems.
    if exact_source
        .iter()
        .any(|entry| entry.file_name() == public_name)
        || (fs::symlink_metadata(&public).is_ok()
            && !receipt.archive.eq_ignore_ascii_case(public_name))
    {
        return Err("Public archive already exists".into());
    }
    let manifest = work.join("SHA256SUMS");
    let mut replacement = tempfile::NamedTempFile::new_in(work)?;
    writeln!(replacement, "{}  {}", receipt.sha256, public_name)?;
    replacement.as_file().sync_all()?;

    // Keep recovery data outside automatic cleanup until the archive and
    // manifest are both committed. Hard-link creation never clobbers a destination.
    let staging = tempfile::Builder::new()
        .prefix(".obs-ci-handoff-")
        .tempdir_in(work)?
        .keep();
    let backup = staging.join("original");
    if let Err(err) = fs::rename(path, &backup) {
        let _ = fs::remove_dir(&staging);
        return Err(err.into());
    }
    if let Err(err) = fs::hard_link(&backup, &public) {
        if let Err(restore) = fs::hard_link(&backup, path) {
            return Err(format!(
                "Handoff failed: {err}; restore failed: {restore}; original preserved at {}",
                backup.display()
            )
            .into());
        }
        fs::remove_file(&backup)?;
        fs::remove_dir(&staging)?;
        return Err(err.into());
    }
    if let Err(err) = replace_manifest(replacement, &manifest) {
        let restore = fs::remove_file(&public).and_then(|_| fs::hard_link(&backup, path));
        if let Err(restore) = restore {
            return Err(format!("Manifest replacement failed: {err}; restore failed: {restore}; original preserved at {}", backup.display()).into());
        }
        fs::remove_file(&backup)?;
        fs::remove_dir(&staging)?;
        return Err(err.into());
    }
    fs::remove_file(&backup)?;
    fs::remove_dir(&staging)?;
    receipt.archive = public_name.to_owned();
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn manifest_failure_restores_original_bytes_and_name() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("Example-test.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&original).unwrap());
        zip.start_file(
            "example/module.dll",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(b"module").unwrap();
        zip.finish().unwrap();
        let bytes = fs::read(&original).unwrap();
        let manifest = dir.path().join("SHA256SUMS");
        let before = format!("{:x}  Example-test.zip\n", Sha256::digest(&bytes));
        fs::write(&manifest, &before).unwrap();
        for public in ["public-test.zip", "example-test.zip"] {
            let result = handoff_with(&original, public, |_, _| {
                Err(std::io::Error::other(
                    "simulated manifest replacement failure",
                ))
            });
            assert!(result.is_err());
            assert_eq!(fs::read(&original).unwrap(), bytes);
            assert_eq!(fs::read_to_string(&manifest).unwrap(), before);
            let names: std::collections::BTreeSet<_> = fs::read_dir(dir.path())
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            assert_eq!(
                names,
                ["Example-test.zip", "SHA256SUMS"]
                    .into_iter()
                    .map(std::ffi::OsString::from)
                    .collect()
            );
        }
    }
}
