use flate2::read::MultiGzDecoder;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs::{self, File},
    io::{self, Read},
    path::Path,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Serialize)]
pub struct Receipt {
    archive: String,
    sha256: String,
    bytes: u64,
    regular_files: usize,
    directories: usize,
}

#[derive(Default)]
struct Members {
    entries: BTreeMap<String, bool>,
    spellings: BTreeMap<String, String>,
    nonempty_files: usize,
}

impl Members {
    fn add(&mut self, name: &str, directory: bool, size: u64) -> Result<()> {
        let name = if directory {
            name.strip_suffix('/').unwrap_or(name)
        } else {
            name
        };
        let parts: Vec<_> = name.split('/').collect();
        if parts
            .iter()
            .any(|p| p.is_empty() || *p == "." || *p == ".." || p.ends_with([' ', '.']))
            || name
                .chars()
                .any(|c| c.is_control() || matches!(c, '\\' | ':'))
        {
            return Err(format!("Unsafe archive path: {name:?}").into());
        }
        if parts.iter().any(|p| {
            let lower = caseless::default_case_fold_str(p);
            matches!(lower.as_str(), ".git" | ".env") || lower.ends_with(".pdb")
        }) {
            return Err(format!("Forbidden archive payload: {name:?}").into());
        }
        for end in 1..=parts.len() {
            let prefix = parts[..end].join("/");
            let previous = self
                .spellings
                .entry(caseless::default_case_fold_str(&prefix))
                .or_insert_with(|| prefix.clone());
            if *previous != prefix {
                return Err(
                    format!("Case-colliding archive paths: {previous:?}, {prefix:?}").into(),
                );
            }
        }
        if self.entries.insert(name.to_owned(), directory).is_some() {
            return Err(format!("Duplicate archive path: {name:?}").into());
        }
        if !directory && size > 0 {
            self.nonempty_files += 1;
        }
        Ok(())
    }

    fn finish(&self) -> Result<()> {
        if self.nonempty_files == 0 {
            return Err("Archive has no nonempty regular files".into());
        }
        for name in self.entries.keys() {
            for (i, _) in name.match_indices('/') {
                if self.entries.get(&name[..i]) == Some(&false) {
                    return Err(format!("Archive file used as a directory: {name:?}").into());
                }
            }
        }
        Ok(())
    }
}

fn regular_file(path: &Path) -> Result<File> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(format!("Missing or invalid verified file: {}", path.display()).into());
    }
    Ok(File::open(path)?)
}

fn digest(file: &mut File) -> Result<String> {
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub fn audit(path: &Path) -> Result<Receipt> {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Invalid archive filename")?;
    let mut file = regular_file(path)?;
    let bytes = file.metadata()?.len();
    let sha256 = digest(&mut file)?;
    let mut manifest = String::new();
    regular_file(&path.with_file_name("SHA256SUMS"))?.read_to_string(&mut manifest)?;
    if manifest.replace("\r\n", "\n") != format!("{sha256}  {name}\n") {
        return Err("Archive name or SHA-256 mismatch".into());
    }
    let mut members = Members::default();
    if name.ends_with(".zip") {
        let mut package = zip::ZipArchive::new(regular_file(path)?)?;
        if package.has_duplicate_names() {
            return Err("Duplicate ZIP member names".into());
        }
        package.validate_header_consistency()?;
        for i in 0..package.len() {
            let mut entry = package.by_index(i)?;
            let member_name = std::str::from_utf8(entry.name_raw())?.to_owned();
            let directory = entry.is_dir();
            let kind = entry.unix_mode().unwrap_or(0) & 0o170000;
            if kind != 0 && kind != if directory { 0o040000 } else { 0o100000 } {
                return Err(format!("Unsupported ZIP member type: {member_name:?}").into());
            }
            members.add(&member_name, directory, entry.size())?;
            // Reading every member verifies its decompression and CRC.
            io::copy(&mut entry, &mut io::sink())?;
        }
    } else if name.ends_with(".tar.gz") {
        let decoder = MultiGzDecoder::new(regular_file(path)?);
        let mut package = tar::Archive::new(decoder);
        for entry in package.entries()? {
            let mut entry = entry?;
            let kind = entry.header().entry_type();
            let member_name = std::str::from_utf8(&entry.path_bytes())?.to_owned();
            if !(kind.is_file() || kind.is_dir()) {
                return Err(format!("Unsupported tar member type: {member_name:?}").into());
            }
            members.add(&member_name, kind.is_dir(), entry.size())?;
            io::copy(&mut entry, &mut io::sink())?;
        }
        // Drain the decoder so a bad/truncated gzip footer cannot pass.
        io::copy(&mut package.into_inner(), &mut io::sink())?;
    } else {
        return Err("Unsupported package extension".into());
    }
    members.finish()?;
    let directories = members
        .entries
        .values()
        .filter(|&&directory| directory)
        .count();
    Ok(Receipt {
        archive: name.into(),
        sha256,
        bytes,
        regular_files: members.entries.len() - directories,
        directories,
    })
}
