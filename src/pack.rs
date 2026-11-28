use crate::{
    audit::{Receipt, archive_hashes, digest},
    handoff::valid_name,
};
use flate2::{Compression, write::GzEncoder};
use std::{
    collections::BTreeMap,
    error::Error,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn inventory(root: &Path, dir: &Path, files: &mut BTreeMap<String, PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() {
            inventory(root, &path, files)?;
        } else if kind.is_file() {
            let name = path
                .strip_prefix(root)?
                .components()
                .map(|part| part.as_os_str().to_str().ok_or("Non-UTF8 payload path"))
                .collect::<std::result::Result<Vec<_>, _>>()?
                .join("/");
            if name.contains('\\') || files.insert(name, path).is_some() {
                return Err("Invalid or duplicate payload path".into());
            }
        } else {
            return Err(format!("Nonregular payload entry: {}", path.display()).into());
        }
    }
    Ok(())
}

fn mode(path: &Path) -> Result<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(fs::metadata(path)?.permissions().mode() & 0o777)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(0o644)
    }
}

fn write_archive(path: &Path, name: &str, files: &BTreeMap<String, PathBuf>) -> Result<()> {
    if name.ends_with(".zip") {
        let mut zip = zip::ZipWriter::new(File::create(path)?);
        for (member, source) in files {
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(mode(source)?);
            zip.start_file(member, options)?;
            io::copy(&mut File::open(source)?, &mut zip)?;
        }
        zip.finish()?.sync_all()?;
    } else {
        let encoder = GzEncoder::new(File::create(path)?, Compression::default());
        let mut tar = tar::Builder::new(encoder);
        for (member, source) in files {
            let file = File::open(source)?;
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Regular);
            header.set_size(file.metadata()?.len());
            header.set_mode(mode(source)?);
            header.set_cksum();
            tar.append_data(&mut header, member, file)?;
        }
        tar.into_inner()?.finish()?.sync_all()?;
    }
    Ok(())
}

/// Caller owns staging policy. This command owns archive bytes, checksums and
/// readback. Use an isolated, quiescent staging tree and a fresh output directory.
pub fn pack(stage: &Path, output: &Path, name: &str) -> Result<Receipt> {
    if !valid_name(name) || name.ends_with("-source.tar.gz") {
        return Err("Expected a valid binary archive filename".into());
    }
    if !fs::symlink_metadata(stage)?.is_dir() {
        return Err("Stage must be a real directory".into());
    }
    let stage = stage.canonicalize()?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()?;
    let output = parent.join(output.file_name().ok_or("Missing output directory name")?);
    if parent.starts_with(&stage) || fs::symlink_metadata(&output).is_ok() {
        return Err("Output must be a fresh directory outside the staging tree".into());
    }
    let mut files = BTreeMap::new();
    inventory(&stage, &stage, &mut files)?;
    let expected = files
        .iter()
        .map(|(name, path)| Ok((name.clone(), digest(&mut File::open(path)?)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let temp = tempfile::tempdir_in(&parent)?;
    let archive = temp.path().join(name);
    write_archive(&archive, name, &files)?;
    let checksum = temp.path().join("SHA256SUMS");
    fs::write(
        &checksum,
        format!("{}  {name}\n", digest(&mut File::open(&archive)?)?),
    )?;
    let (receipt, actual) = archive_hashes(&archive)?;
    if actual != expected {
        return Err("Archive contents differ from staged file hashes".into());
    }
    // Recheck the inventory after writing to detect incidental staging changes.
    let mut after = BTreeMap::new();
    inventory(&stage, &stage, &mut after)?;
    if files != after
        || after.iter().any(|(name, path)| {
            File::open(path)
                .ok()
                .and_then(|mut f| digest(&mut f).ok())
                .as_ref()
                != expected.get(name)
        })
    {
        return Err("Staged payload changed during packaging".into());
    }
    // Publish into an exclusively created directory. Never replace an existing
    // archive or checksum; remove only files created by this invocation on failure.
    fs::create_dir(&output)?;
    let dest = output.join(name);
    if let Err(err) = fs::hard_link(&archive, &dest) {
        let _ = fs::remove_dir(&output);
        return Err(err.into());
    }
    if let Err(err) = fs::hard_link(&checksum, output.join("SHA256SUMS")) {
        let _ = fs::remove_file(&dest);
        let _ = fs::remove_dir(&output);
        return Err(err.into());
    }
    Ok(receipt)
}
