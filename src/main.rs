mod audit;
mod handoff;
mod native;
mod plan;

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
