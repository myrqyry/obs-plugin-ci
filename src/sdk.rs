use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fingerprint {
    schema: u32,
    recipe_sha256: String,
    work: PathBuf,
    os_sha256: String,
    package_db_sha256: String,
    tools: BTreeMap<String, String>,
    cache_key: String,
}

fn hash_file(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    io::copy(&mut File::open(path)?, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

// Canonicalize the parent even before creating the workspace; never silently
// accept an existing symlink as an SDK workspace.
fn work_path(path: &Path) -> Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or("SDK work must be a named directory")?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let work = parent.canonicalize()?.join(name);
    match fs::symlink_metadata(&work) {
        Ok(m) if !m.is_dir() => return Err("SDK work must be a real directory".into()),
        Ok(_) => (),
        Err(e) if e.kind() == io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    }
    Ok(work)
}

pub fn fingerprint(plan: &Plan, work: &Path) -> Result<Fingerprint> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err(
            "Compiled SDK preparation currently requires Debian/Ubuntu-compatible Linux x86_64"
                .into(),
        );
    }
    let work = work_path(work)?;
    let mut tools = BTreeMap::new();
    for name in [
        "cmake",
        "ninja",
        "gcc",
        "g++",
        "ld",
        "ar",
        "ranlib",
        "pkg-config",
        "as",
        "strip",
    ] {
        let path = Path::new("/usr/bin").join(name).canonicalize()?;
        tools.insert(
            name.into(),
            format!("{}:{}", path.display(), hash_file(&path)?),
        );
    }
    let mut result = Fingerprint {
        schema: 1,
        recipe_sha256: plan.recipe_sha256.clone(),
        work,
        os_sha256: hash_file(Path::new("/etc/os-release"))?,
        package_db_sha256: hash_file(Path::new("/var/lib/dpkg/status"))?,
        tools,
        cache_key: String::new(),
    };
    result.cache_key = format!("{:x}", Sha256::digest(serde_json::to_vec(&result)?));
    Ok(result)
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct InstalledEntry {
    kind: String,
    value: String,
    mode: u32,
}

fn tree_entries(root: &Path) -> Result<BTreeMap<String, InstalledEntry>> {
    if !fs::symlink_metadata(root)?.is_dir() {
        return Err("SDK install root is not a real directory".into());
    }
    let mut entries = BTreeMap::new();
    fn walk(root: &Path, dir: &Path, entries: &mut BTreeMap<String, InstalledEntry>) -> Result<()> {
        for item in fs::read_dir(dir)? {
            let item = item?;
            let path = item.path();
            let meta = fs::symlink_metadata(&path)?;
            let name = path
                .strip_prefix(root)?
                .to_str()
                .ok_or("Non-UTF8 SDK path")?
                .to_owned();
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                meta.permissions().mode() & 0o7777
            };
            #[cfg(not(unix))]
            let mode = 0;
            let (kind, value) = if meta.is_dir() {
                walk(root, &path, entries)?;
                ("directory", String::new())
            } else if meta.is_file() {
                ("file", hash_file(&path)?)
            } else if meta.file_type().is_symlink() {
                let target = fs::read_link(&path)?;
                if target.is_absolute() || !path.canonicalize()?.starts_with(root) {
                    return Err(format!("SDK link escapes install root: {name}").into());
                }
                (
                    "symlink",
                    target.to_str().ok_or("Non-UTF8 SDK link")?.to_owned(),
                )
            } else {
                return Err(format!("Unsupported SDK entry: {name}").into());
            };
            entries.insert(
                name,
                InstalledEntry {
                    kind: kind.into(),
                    value,
                    mode,
                },
            );
        }
        Ok(())
    }
    walk(root, root, &mut entries)?;
    Ok(entries)
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstalledReceipt {
    fingerprint: Fingerprint,
    entries: BTreeMap<String, InstalledEntry>,
}

fn installed_receipt(plan: &Plan, fp: Fingerprint) -> Result<InstalledReceipt> {
    let sdk = fp.work.join("sdk");
    let entries = tree_entries(&sdk)?;
    for file in plan.recipe.required_sdk_files {
        let path = sdk.join(file);
        if !fs::symlink_metadata(&path)?.is_file() || path.metadata()?.len() == 0 {
            return Err(format!("Missing SDK config: {file}").into());
        }
    }
    let marker = sdk.join("OBS_SDK_VERSION");
    if !fs::symlink_metadata(&marker)?.is_file()
        || fs::read_to_string(marker)? != format!("{}\n", plan.recipe.obs_version)
    {
        return Err("SDK version marker mismatch".into());
    }
    Ok(InstalledReceipt {
        fingerprint: fp,
        entries,
    })
}

pub fn verify_installed(plan: &Plan, work: &Path) -> Result<InstalledReceipt> {
    let fp = fingerprint(plan, work)?;
    let path = fp.work.join("SDK-RECEIPT.json");
    if !fs::symlink_metadata(&path)?.is_file() {
        return Err("SDK receipt must be a regular file".into());
    }
    let expected: InstalledReceipt = serde_json::from_reader(File::open(path)?)?;
    let actual = installed_receipt(plan, fp)?;
    if expected != actual {
        return Err(
            "SDK receipt mismatch: environment, install prefix or installed tree changed".into(),
        );
    }
    Ok(actual)
}

fn cmake(work: &Path) -> Command {
    let mut cmd = Command::new("/usr/bin/cmake");
    cmd.env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", work.join("home"))
        .env("TMPDIR", work.join("tmp"))
        .env("LC_ALL", "C")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    cmd
}

fn run_cmake(mut cmd: Command, work: &Path) -> Result<()> {
    // Preserve compiler/configure diagnostics without mixing them into JSON.
    let log_path = work.join("sdk-build.log");
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    cmd.stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log));
    let status = cmd.status()?;
    if !status.success() {
        return Err(format!(
            "CMake failed ({status}): {cmd:?}; see {}",
            log_path.display()
        )
        .into());
    }
    Ok(())
}

fn build_sdk(plan: &Plan, work: &Path, source: &Path, jobs: u16) -> Result<()> {
    let build = work.join("obs-build");
    let sdk = work.join("sdk");
    fs::create_dir(work.join("home"))?;
    fs::create_dir(work.join("tmp"))?;
    let mut configure = cmake(work);
    configure
        .arg("-S")
        .arg(source)
        .arg("-B")
        .arg(&build)
        .args(["-G", plan.recipe.cmake_generator])
        .arg(format!("-DCMAKE_INSTALL_PREFIX={}", sdk.display()))
        .args([
            "-DCMAKE_C_COMPILER=/usr/bin/gcc",
            "-DCMAKE_CXX_COMPILER=/usr/bin/g++",
            "-DCMAKE_MAKE_PROGRAM=/usr/bin/ninja",
            "-DCMAKE_FIND_USE_PACKAGE_REGISTRY=OFF",
            "-DCMAKE_FIND_USE_SYSTEM_PACKAGE_REGISTRY=OFF",
        ])
        .args(&plan.recipe.cmake_options);
    run_cmake(configure, work)?;
    let mut compile = cmake(work);
    compile
        .arg("--build")
        .arg(&build)
        .arg("--target")
        .args(plan.recipe.build_targets)
        .arg("--parallel")
        .arg(jobs.to_string());
    run_cmake(compile, work)?;
    for component in plan.recipe.install_components {
        let mut install = cmake(work);
        install
            .arg("--install")
            .arg(&build)
            .args(["--component", component, "--config", "Release", "--prefix"])
            .arg(&sdk);
        run_cmake(install, work)?;
    }
    fs::write(
        sdk.join("OBS_SDK_VERSION"),
        format!("{}\n", plan.recipe.obs_version),
    )?;
    Ok(())
}

fn require_disk(parent: &Path) -> Result<()> {
    let result = Command::new("/usr/bin/df")
        .env("LC_ALL", "C")
        .args(["-Pk"])
        .arg(parent)
        .output()?;
    if !result.status.success() {
        return Err("Unable to check free disk space".into());
    }
    let text = String::from_utf8(result.stdout)?;
    let available: u64 = text
        .lines()
        .last()
        .ok_or("Missing disk usage")?
        .split_whitespace()
        .nth(3)
        .ok_or("Missing disk availability")?
        .parse()?;
    if available < 8 * 1024 * 1024 {
        return Err("SDK build requires at least 8 GiB available; no build started".into());
    }
    Ok(())
}

pub fn prepare(plan: &Plan, cache: &Path, work: &Path, jobs: u16) -> Result<InstalledReceipt> {
    if !(1..=64).contains(&jobs) {
        return Err("SDK jobs must be between 1 and 64".into());
    }
    let fp = fingerprint(plan, work)?;
    // Existing directories are never repaired or overwritten. Only reuse a
    // complete receipt that matches this exact environment and prefix.
    if fp.work.exists() {
        return verify_installed(plan, &fp.work);
    }
    require_disk(fp.work.parent().ok_or("Missing SDK parent")?)?;
    let source = fetch_source(plan, cache)?;
    fs::create_dir(&fp.work)?;
    let mut file = File::open(source.archive)?;
    verify_reader(&mut file, plan.recipe.source_sha256)?;
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0))?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    archive.unpack(&fp.work)?;
    let source = fp
        .work
        .join(format!("obs-studio-{}", plan.recipe.obs_version));
    build_sdk(plan, &fp.work, &source, jobs)?;
    // A package-manager change during a build must invalidate the result.
    if fingerprint(plan, &fp.work)? != fp {
        return Err("SDK environment changed during compilation".into());
    }
    seal_installed(plan, fp)
}

fn seal_installed(plan: &Plan, fp: Fingerprint) -> Result<InstalledReceipt> {
    let receipt = installed_receipt(plan, fp)?;
    let mut tmp = tempfile::NamedTempFile::new_in(&receipt.fingerprint.work)?;
    serde_json::to_writer_pretty(&mut tmp, &receipt)?;
    tmp.as_file().sync_all()?;
    tmp.persist_noclobber(receipt.fingerprint.work.join("SDK-RECEIPT.json"))?;
    Ok(receipt)
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod build_tests {
    use super::*;

    #[test]
    fn actual_cmake_install_receipt_rejects_mutation_and_wrong_recipe() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        fs::create_dir(&work).unwrap();
        let source = work.join("source");
        fs::create_dir(&source).unwrap();
        fs::write(
            source.join("core.c"),
            "int sdk_fixture(void) { return 42; }\n",
        )
        .unwrap();
        fs::write(source.join("CMakeLists.txt"), r##"
cmake_minimum_required(VERSION 3.20)
project(Fixture LANGUAGES C CXX)
add_library(obs-frontend-api SHARED core.c)
add_library(libobs-opengl SHARED core.c)
set_target_properties(obs-frontend-api PROPERTIES VERSION 1.0 SOVERSION 1)
install(TARGETS obs-frontend-api libobs-opengl LIBRARY DESTINATION lib COMPONENT Runtime)
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/libobsConfig.cmake" "# fixture config\n")
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/obs-frontend-apiConfig.cmake" "# fixture config\n")
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/libobsConfig.cmake" DESTINATION lib/cmake/libobs COMPONENT Development)
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/obs-frontend-apiConfig.cmake" DESTINATION lib/cmake/obs-frontend-api COMPONENT Development)
"##).unwrap();
        let plan = plan("32.2.0", "legacy-core").unwrap();
        let fp = fingerprint(&plan, &work).unwrap();
        build_sdk(&plan, &work, &source, 2).unwrap();
        let receipt = seal_installed(&plan, fp).unwrap();
        assert_eq!(verify_installed(&plan, &work).unwrap(), receipt);
        assert!(work.join("sdk-build.log").metadata().unwrap().len() > 0);
        let unused_cache = dir.path().join("unused-cache");
        assert_eq!(prepare(&plan, &unused_cache, &work, 4).unwrap(), receipt);
        assert!(!unused_cache.exists());
        let different = super::plan("32.2.0", "streaming-minimal").unwrap();
        assert!(verify_installed(&different, &work).is_err());
        let sdk = work.join("sdk");
        let added = sdk.join("unexpected");
        fs::write(&added, "unexpected").unwrap();
        assert!(verify_installed(&plan, &work).is_err());
        fs::remove_file(&added).unwrap();
        let link = sdk.join("lib/libobs-frontend-api.so");
        assert!(link.is_symlink());
        let target = fs::read_link(&link).unwrap();
        fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", &link).unwrap();
        assert!(verify_installed(&plan, &work).is_err());
        fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(target, &link).unwrap();
        assert_eq!(verify_installed(&plan, &work).unwrap(), receipt);
        fs::write(sdk.join("lib/libobs-frontend-api.so.1.0"), b"changed").unwrap();
        assert!(verify_installed(&plan, &work).is_err());
        assert!(prepare(&plan, &unused_cache, &work, 4).is_err());
        assert!(!unused_cache.exists());
    }
}
