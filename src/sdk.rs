use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Serialize)]
pub struct Recipe {
    schema: u32,
    obs_version: String,
    recipe: String,
    source_url: String,
    source_sha256: &'static str,
    cmake_generator: &'static str,
    cmake_options: Vec<String>,
    build_targets: [&'static str; 2],
    install_components: [&'static str; 2],
    required_sdk_files: [&'static str; 2],
}

#[derive(Serialize)]
pub struct Plan {
    #[serde(flatten)]
    recipe: Recipe,
    recipe_sha256: String,
}

pub fn plan(version: &str, recipe: &str) -> Result<Plan> {
    let source_sha256 = match (version, recipe) {
        ("32.2.0", "streaming-minimal" | "legacy-core") => {
            "c333e4a7d5c4a94c7bb4833f046368e0ac5e981fb3964ca4981b23be1dfbda4a"
        }
        ("33.0.3", "streaming-minimal") => {
            "b0eff217a972e1dd9951ce24d66595c723dde847461b16de07eb33d75ec34659"
        }
        ("33.0.1", "legacy-core") => {
            "9174ee0e3677c92d0d060bdce4c5d44dfee9fd3afddfb6333f098fd029b8de2b"
        }
        _ => return Err("Unsupported OBS SDK version/recipe pair".into()),
    };
    let mut cmake_options = vec![
        "-DCMAKE_BUILD_TYPE=Release".into(),
        "-DOBS_CMAKE_VERSION:STRING=3.0.0".into(),
        format!("-DOBS_VERSION_OVERRIDE:STRING={version}"),
        "-DENABLE_PLUGINS=OFF".into(),
        "-DENABLE_FRONTEND=OFF".into(),
    ];
    if recipe == "streaming-minimal" {
        cmake_options.extend([
            "-DENABLE_SCRIPTING=OFF".into(),
            "-DENABLE_BROWSER=OFF".into(),
        ]);
    }
    let recipe = Recipe {
        schema: 1,
        obs_version: version.into(),
        recipe: recipe.into(),
        source_url: format!(
            "https://github.com/obsproject/obs-studio/archive/refs/tags/{version}.tar.gz"
        ),
        source_sha256,
        cmake_generator: "Ninja",
        cmake_options,
        build_targets: ["obs-frontend-api", "libobs-opengl"],
        install_components: ["Development", "Runtime"],
        required_sdk_files: [
            "lib/cmake/libobs/libobsConfig.cmake",
            "lib/cmake/obs-frontend-api/obs-frontend-apiConfig.cmake",
        ],
    };
    let recipe_sha256 = format!("{:x}", Sha256::digest(serde_json::to_vec(&recipe)?));
    Ok(Plan {
        recipe,
        recipe_sha256,
    })
}

fn verify_reader(mut reader: impl Read, expected: &str) -> Result<u64> {
    let mut hash = Sha256::new();
    let bytes = io::copy(&mut reader, &mut hash)?;
    if format!("{:x}", hash.finalize()) != expected {
        return Err("Pinned OBS source SHA-256 mismatch".into());
    }
    Ok(bytes)
}

#[derive(Serialize)]
pub struct SourceReceipt {
    obs_version: String,
    recipe_sha256: String,
    source_sha256: String,
    bytes: u64,
    archive: PathBuf,
}

pub fn verify_source(plan: &Plan, archive: &Path) -> Result<SourceReceipt> {
    if !fs::symlink_metadata(archive)?.is_file() {
        return Err("OBS source archive must be a regular file".into());
    }
    let bytes = verify_reader(File::open(archive)?, plan.recipe.source_sha256)?;
    Ok(SourceReceipt {
        obs_version: plan.recipe.obs_version.clone(),
        recipe_sha256: plan.recipe_sha256.clone(),
        source_sha256: plan.recipe.source_sha256.into(),
        bytes,
        archive: archive.to_owned(),
    })
}

fn checked(command: &mut Command) -> Result<()> {
    let status = command.status()?;
    if !status.success() {
        return Err(format!("Command failed ({status}): {command:?}").into());
    }
    Ok(())
}

// Only source archives are shared here. Built SDKs are host/toolchain-specific.
pub fn fetch_source(plan: &Plan, cache: &Path) -> Result<SourceReceipt> {
    let folder = cache.join(plan.recipe.source_sha256);
    fs::create_dir_all(cache)?;
    match fs::create_dir(&folder) {
        Ok(()) => (),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => (),
        Err(err) => return Err(err.into()),
    }
    if !fs::symlink_metadata(&folder)?.is_dir() {
        return Err("Invalid SDK cache directory (expected a real directory)".into());
    }
    let archive = folder.join(format!("obs-studio-{}.tar.gz", plan.recipe.obs_version));
    match fs::symlink_metadata(&archive) {
        Ok(_) => return verify_source(plan, &archive),
        Err(err) if err.kind() == io::ErrorKind::NotFound => (),
        Err(err) => return Err(err.into()),
    }
    let tmp = tempfile::NamedTempFile::new_in(&folder)?;
    checked(
        Command::new("curl")
            .args([
                "--fail",
                "--silent",
                "--show-error",
                "--location",
                "--retry",
                "5",
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
                "--output",
            ])
            .arg(tmp.path())
            .arg(&plan.recipe.source_url),
    )?;
    verify_reader(File::open(tmp.path())?, plan.recipe.source_sha256)?;
    match tmp.persist_noclobber(&archive) {
        Ok(_) => (),
        Err(err) if err.error.kind() == io::ErrorKind::AlreadyExists => (),
        Err(err) => return Err(err.error.into()),
    }
    verify_source(plan, &archive)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifier_accepts_exact_bytes_only() {
        let bytes = b"source";
        let digest = format!("{:x}", Sha256::digest(bytes));
        assert_eq!(verify_reader(&bytes[..], &digest).unwrap(), 6);
        assert!(verify_reader(&b"modified"[..], &digest).is_err());
    }
}
