use flate2::read::MultiGzDecoder;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Serialize)]
pub struct Receipt {
    pub(crate) archive: String,
    pub(crate) sha256: String,
    bytes: u64,
    regular_files: usize,
    directories: usize,
}

#[derive(Default)]
struct Members {
    entries: BTreeMap<String, bool>,
    spellings: BTreeMap<String, String>,
    nonempty_files: usize,
    hashes: BTreeMap<String, String>,
    nonempty_paths: BTreeSet<String>,
    captured: Option<Vec<u8>>,
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
            self.nonempty_paths.insert(name.to_owned());
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

pub(crate) fn digest(file: &mut impl Read) -> Result<String> {
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

fn inspect(
    path: &Path,
    source: bool,
    hash_members: bool,
    capture: Option<&str>,
) -> Result<(Receipt, Members)> {
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
    file.seek(SeekFrom::Start(0))?;
    let mut members = Members::default();
    if source && !name.ends_with("-source.tar.gz") {
        return Err("Expected a -source.tar.gz package".into());
    }
    if name.ends_with(".zip") {
        let mut package = zip::ZipArchive::new(file)?;
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
            if capture == Some(member_name.as_str()) && !directory {
                members.captured = Some(read_module(&mut entry)?);
            } else if hash_members && !directory {
                members.hashes.insert(member_name, digest(&mut entry)?);
            } else {
                io::copy(&mut entry, &mut io::sink())?;
            }
        }
    } else if name.ends_with(".tar.gz") {
        let decoder = MultiGzDecoder::new(file);
        let mut package = tar::Archive::new(decoder);
        let mut global_header_seen = false;
        for entry in package.entries()? {
            let mut entry = entry?;
            let kind = entry.header().entry_type();
            if kind.is_pax_global_extensions() {
                // Git archive emits a leading global comment with its commit ID.
                // Do not permit metadata that changes member paths, sizes or types.
                if global_header_seen || !members.entries.is_empty() || entry.size() > 4096 {
                    return Err("Unexpected global PAX header".into());
                }
                global_header_seen = true;
                let extensions = entry.pax_extensions()?.ok_or("Missing PAX metadata")?;
                let mut comment_seen = false;
                for extension in extensions {
                    let extension = extension?;
                    if extension.key_bytes() != b"comment" || comment_seen {
                        return Err("Unsupported global PAX metadata".into());
                    }
                    comment_seen = true;
                }
                if !comment_seen {
                    return Err("Empty global PAX metadata".into());
                }
                continue;
            }

            let member_name = std::str::from_utf8(&entry.path_bytes())?.to_owned();
            if !(kind.is_file() || kind.is_dir()) {
                return Err(format!("Unsupported tar member type: {member_name:?}").into());
            }
            members.add(&member_name, kind.is_dir(), entry.size())?;
            if capture == Some(member_name.as_str()) && kind.is_file() {
                members.captured = Some(read_module(&mut entry)?);
            } else if hash_members && kind.is_file() {
                members.hashes.insert(member_name, digest(&mut entry)?);
            } else {
                io::copy(&mut entry, &mut io::sink())?;
            }
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
    Ok((
        Receipt {
            archive: name.into(),
            sha256,
            bytes,
            regular_files: members.entries.len() - directories,
            directories,
        },
        members,
    ))
}

pub(crate) fn archive_hashes(path: &Path) -> Result<(Receipt, BTreeMap<String, String>)> {
    let (receipt, members) = inspect(path, false, true, None)?;
    Ok((receipt, members.hashes))
}

pub(crate) fn payload_checksums(path: &Path) -> Result<String> {
    let (_, hashes) = archive_hashes(path)?;
    Ok(hashes
        .iter()
        .map(|(name, hash)| format!("{hash}  {name}\n"))
        .collect())
}

pub fn audit(path: &Path) -> Result<Receipt> {
    Ok(inspect(path, false, false, None)?.0)
}

#[derive(Serialize)]
pub struct SourceReceipt {
    #[serde(flatten)]
    archive: Receipt,
    payload_entries: usize,
    payload_verified: bool,
}

fn compiled_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if [
        ".so", ".dll", ".dylib", ".exe", ".pdb", ".o", ".obj", ".a", ".lib", ".pyc",
    ]
    .iter()
    .any(|suffix| lower.ends_with(suffix))
    {
        return true;
    }
    lower.rsplit_once(".so.").is_some_and(|(_, version)| {
        version
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
    })
}

fn license_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ["license", "copying"].iter().any(|prefix| {
        lower == *prefix
            || lower
                .strip_prefix(prefix)
                .is_some_and(|suffix| suffix.len() > 1 && suffix.starts_with(['.', '_', '-']))
    })
}

fn payload_manifest(path: &Path) -> Result<BTreeMap<String, String>> {
    let mut content = String::new();
    regular_file(path)?.read_to_string(&mut content)?;
    let mut entries = BTreeMap::new();
    for line in content.lines() {
        let (hash, name) = line
            .split_once("  ")
            .ok_or("Invalid payload checksum manifest")?;
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("Invalid payload checksum digest".into());
        }
        if name.trim().is_empty()
            || name.split('/').any(|p| matches!(p, "" | "." | ".."))
            || name
                .chars()
                .any(|c| c.is_control() || matches!(c, '\\' | ':'))
        {
            return Err(format!("Unsafe payload checksum path: {name:?}").into());
        }
        if entries.insert(name.to_owned(), hash.to_owned()).is_some() {
            return Err(format!("Duplicate payload checksum path: {name:?}").into());
        }
    }
    if entries.is_empty() {
        return Err("Empty payload checksum manifest".into());
    }
    Ok(entries)
}

pub fn audit_source(path: &Path, binary: Option<&Path>) -> Result<SourceReceipt> {
    let (receipt, members) = inspect(path, true, false, None)?;
    let roots: BTreeSet<_> = members
        .entries
        .keys()
        .map(|name| name.split('/').next().unwrap())
        .collect();
    if roots.len() != 1 {
        return Err("Source archive must have one root".into());
    }
    for (name, directory) in &members.entries {
        if !directory && (!name.contains('/') || compiled_file(name)) {
            return Err(format!("Unexpected source file: {name:?}").into());
        }
    }
    if !members.nonempty_paths.iter().any(|name| {
        name.split_once('/')
            .is_some_and(|(_, relative)| !relative.contains('/') && license_file(relative))
    }) {
        return Err("Source archive missing root LICENSE or COPYING".into());
    }
    let expected = payload_manifest(&path.with_file_name("PAYLOAD-SHA256SUMS"))?;
    if let Some(binary) = binary {
        let (_, installed) = inspect(binary, false, true, None)?;
        if expected != installed.hashes {
            let missing: Vec<_> = expected
                .keys()
                .filter(|k| !installed.hashes.contains_key(*k))
                .collect();
            let extra: Vec<_> = installed
                .hashes
                .keys()
                .filter(|k| !expected.contains_key(*k))
                .collect();
            let changed: Vec<_> = expected
                .iter()
                .filter(|(k, v)| installed.hashes.get(*k).is_some_and(|actual| actual != *v))
                .map(|(k, _)| k)
                .collect();
            return Err(format!("Payload manifest mismatch: missing={missing:?}, extra={extra:?}, changed={changed:?}").into());
        }
    }
    Ok(SourceReceipt {
        archive: receipt,
        payload_entries: expected.len(),
        payload_verified: binary.is_some(),
    })
}

fn read_module(reader: &mut impl Read) -> Result<Vec<u8>> {
    const MAX: u64 = 128 * 1024 * 1024;
    let mut bytes = Vec::new();
    reader.take(MAX + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX {
        return Err("Module exceeds 128 MiB inspection limit".into());
    }
    Ok(bytes)
}

pub(crate) fn module_bytes(path: &Path, member: &str) -> Result<(Receipt, Vec<u8>)> {
    let (receipt, members) = inspect(path, false, false, Some(member))?;
    Ok((
        receipt,
        members
            .captured
            .ok_or_else(|| format!("Missing module member: {member}"))?,
    ))
}
