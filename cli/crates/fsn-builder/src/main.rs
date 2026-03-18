//! FreeSynergy Resource Builder CLI.
//!
//! # Commands
//!
//! ```text
//! fsn-builder analyze  <compose.yml>    — Docker Compose → ContainerAppResource
//! fsn-builder validate <package-dir>    — validate a resource package
//! fsn-builder publish  <package-dir>    — sign + git-commit + push to Store
//! ```

mod analyze;
mod publish;
mod validate;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "fsn-builder",
    about = "FreeSynergy Resource Builder — analyze, validate, and publish FSN resources",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Analyze a Docker Compose file and output a ContainerAppResource.
    Analyze {
        /// Path to the Docker Compose YAML file.
        #[arg(value_name = "COMPOSE_FILE")]
        path: std::path::PathBuf,
        /// Output format: "toml" (default) or "json".
        #[arg(long, default_value = "toml")]
        format: String,
    },
    /// Validate a resource package directory.
    Validate {
        /// Path to the resource package directory (must contain resource.toml).
        #[arg(value_name = "PACKAGE_DIR")]
        path: std::path::PathBuf,
    },
    /// Sign and publish a resource package to the Store.
    Publish {
        /// Path to the resource package directory.
        #[arg(value_name = "PACKAGE_DIR")]
        path: std::path::PathBuf,
        /// Git remote URL of the Store repository.
        #[arg(long, default_value = "git@github.com:FreeSynergy/Store.git")]
        store: String,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Analyze { path, format } => analyze::run(&path, &format),
        Command::Validate { path }        => validate::run(&path),
        Command::Publish { path, store }  => publish::run(&path, &store),
    }
}
