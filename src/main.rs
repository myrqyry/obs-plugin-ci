mod audit;
mod handoff;
mod native;
mod pack;
mod plan;
mod sdk;

use clap::{Parser, Subcommand};
use std::{error::Error, path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(version, about = "Local OBS plugin CI contract tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Preview the existing shared workflow matrix as JSON.
    Plan {
        #[arg(long)]
        module: String,
        #[arg(long, default_value = "")]
        package_name: String,
        #[arg(long)]
        minimum_tests: String,
        #[arg(long, default_value = "full")]
        profile: String,
        #[arg(long, default_value = "native")]
        driver: String,
        #[arg(long)]
        source_package: bool,
        /// Restrict to comma-separated supported platform targets.
        #[arg(long, value_delimiter = ',')]
        targets: Vec<String>,
        /// Restrict to comma-separated pinned OBS versions.
        #[arg(long, value_delimiter = ',')]
        obs_versions: Vec<String>,
    },
    /// Print a pinned Linux SDK source and CMake recipe.
    SdkPlan {
        #[arg(long)]
        obs_version: String,
        #[arg(long)]
        recipe: String,
    },
    /// Verify an existing pinned OBS source archive.
    SdkVerifySource {
        archive: PathBuf,
        #[arg(long)]
        obs_version: String,
        #[arg(long)]
        recipe: String,
    },
    /// Fetch or reverify a content-addressed OBS source archive.
    SdkFetch {
        #[arg(long)]
        cache: PathBuf,
        #[arg(long)]
        obs_version: String,
        #[arg(long)]
        recipe: String,
    },
    /// Fingerprint a Linux SDK environment and exact install workspace.
    SdkFingerprint {
        #[arg(long)]
        obs_version: String,
        #[arg(long)]
        recipe: String,
        #[arg(long)]
        work: PathBuf,
    },
    /// Prepare a new pinned SDK, or verify an existing completed workspace.
    SdkPrepare {
        #[arg(long)]
        obs_version: String,
        #[arg(long)]
        recipe: String,
        #[arg(long)]
        cache: PathBuf,
        #[arg(long)]
        work: PathBuf,
        #[arg(long, default_value_t = 4)]
        jobs: u16,
    },
    /// Verify the installed SDK against its environment and tree receipt.
    SdkVerifyInstalled {
        #[arg(long)]
        obs_version: String,
        #[arg(long)]
        recipe: String,
        #[arg(long)]
        work: PathBuf,
    },
    /// Package a staged binary tree into a new verified artifact directory.
    Pack {
        #[arg(long)]
        stage: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        archive_name: String,
    },
    /// Emit sorted per-file SHA-256 entries from a verified binary package.
    PayloadManifest { archive: PathBuf },
    /// Read-only binary archive and adjacent SHA256SUMS verification.
    Audit { archive: PathBuf },
    /// Verify and rename a package with no destination overwrite.
    Handoff {
        archive: PathBuf,
        #[arg(long)]
        public_name: String,
    },
    /// Verify the expected native module, architecture and OBS entry-point exports.
    InspectModule {
        archive: PathBuf,
        #[arg(long)]
        module: String,
        #[arg(long)]
        target: String,
        #[arg(long)]
        obs_version: String,
    },
    /// Verify source archive and payload manifest; optionally compare an actual binary package.
    AuditSource {
        archive: PathBuf,
        #[arg(long)]
        binary: Option<PathBuf>,
    },
}

fn run(command: Command) -> Result<(), Box<dyn Error>> {
    match command {
        Command::Plan {
            module,
            package_name,
            minimum_tests,
            profile,
            driver,
            source_package,
            targets,
            obs_versions,
        } => {
            println!(
                "{}",
                serde_json::to_string(&plan::plan(
                    &module,
                    &package_name,
                    &minimum_tests,
                    &profile,
                    &driver,
                    source_package,
                    (&targets, &obs_versions),
                )?)?
            );
        }
        Command::SdkPlan {
            obs_version,
            recipe,
        } => {
            println!(
                "{}",
                serde_json::to_string(&sdk::plan(&obs_version, &recipe)?)?
            );
        }
        Command::SdkVerifySource {
            archive,
            obs_version,
            recipe,
        } => {
            println!(
                "{}",
                serde_json::to_string(&sdk::verify_source(
                    &sdk::plan(&obs_version, &recipe)?,
                    &archive
                )?)?
            );
        }
        Command::SdkFetch {
            cache,
            obs_version,
            recipe,
        } => {
            println!(
                "{}",
                serde_json::to_string(&sdk::fetch_source(
                    &sdk::plan(&obs_version, &recipe)?,
                    &cache
                )?)?
            );
        }
        Command::SdkFingerprint {
            obs_version,
            recipe,
            work,
        } => {
            println!(
                "{}",
                serde_json::to_string(&sdk::fingerprint(
                    &sdk::plan(&obs_version, &recipe)?,
                    &work
                )?)?
            );
        }
        Command::SdkPrepare {
            obs_version,
            recipe,
            cache,
            work,
            jobs,
        } => {
            println!(
                "{}",
                serde_json::to_string(&sdk::prepare(
                    &sdk::plan(&obs_version, &recipe)?,
                    &cache,
                    &work,
                    jobs
                )?)?
            );
        }
        Command::SdkVerifyInstalled {
            obs_version,
            recipe,
            work,
        } => {
            println!(
                "{}",
                serde_json::to_string(&sdk::verify_installed(
                    &sdk::plan(&obs_version, &recipe)?,
                    &work
                )?)?
            );
        }
        Command::Pack {
            stage,
            output,
            archive_name,
        } => {
            println!(
                "{}",
                serde_json::to_string(&pack::pack(&stage, &output, &archive_name)?)?
            );
        }
        Command::PayloadManifest { archive } => {
            print!("{}", audit::payload_checksums(&archive)?);
        }
        Command::Handoff {
            archive,
            public_name,
        } => {
            println!(
                "{}",
                serde_json::to_string(&handoff::handoff(&archive, &public_name)?)?
            );
        }
        Command::InspectModule {
            archive,
            module,
            target,
            obs_version,
        } => {
            println!(
                "{}",
                serde_json::to_string(&native::inspect_module(
                    &archive,
                    &module,
                    &target,
                    &obs_version
                )?)?
            );
        }
        Command::AuditSource { archive, binary } => {
            println!(
                "{}",
                serde_json::to_string(&audit::audit_source(&archive, binary.as_deref())?)?
            );
        }
        Command::Audit { archive } => {
            println!("{}", serde_json::to_string(&audit::audit(&archive)?)?);
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("obs-ci: {err}");
            ExitCode::FAILURE
        }
    }
}
