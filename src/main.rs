mod audit;
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
    },
    /// Read-only binary archive and adjacent SHA256SUMS verification.
    Audit { archive: PathBuf },
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
                )?)?
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
