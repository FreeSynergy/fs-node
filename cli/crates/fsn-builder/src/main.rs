//! FreeSynergy Resource Builder CLI.
//!
//! # Commands
//!
//! ```text
//! fsn-builder analyze  <compose.yml>    — Docker Compose → ContainerResource
//! fsn-builder validate <package-dir>    — validate a resource package
//! fsn-builder publish  <package-dir>    — sign + git-commit + push to Store
//! ```

mod analyze;
mod fetch_icon;
mod publish;
mod validate;
mod validate_store;

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
    /// Analyze a Docker Compose file and output a ContainerResource.
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
    /// Validate all packages in a store catalog (checks paths, icons, required fields).
    ValidateStore {
        /// Path to the local Store repository root.
        #[arg(value_name = "STORE_DIR")]
        store_dir: std::path::PathBuf,
        /// Namespace to validate (e.g. "node").
        #[arg(value_name = "NAMESPACE", default_value = "node")]
        namespace: String,
    },
    /// Download an SVG icon and store it in the Store repo.
    ///
    /// Sources:
    ///   homarr:<name>    — Homarr Dashboard Icons (MIT)
    ///   simple:<name>    — Simple Icons (CC0)
    ///   https://...      — Any HTTPS URL (verify license manually)
    ///
    /// Example: fsn-builder fetch-icon homarr:kanidm kanidm --store-dir /path/to/Store
    FetchIcon {
        /// Icon source: "homarr:<name>", "simple:<name>", or "https://..."
        #[arg(value_name = "SOURCE")]
        source: String,
        /// Output icon name (without .svg extension).
        #[arg(value_name = "NAME")]
        name: String,
        /// Path to the local Store repository root.
        #[arg(long, value_name = "DIR", default_value = "../FreeSynergy.Store")]
        store_dir: std::path::PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Analyze { path, format }                   => analyze::run(&path, &format),
        Command::Validate { path }                          => validate::run(&path),
        Command::Publish { path, store }                    => publish::run(&path, &store),
        Command::ValidateStore { store_dir, namespace }     => validate_store::run(&store_dir, &namespace),
        Command::FetchIcon { source, name, store_dir }      =>
            fetch_icon::run(&source, &name, &store_dir).await,
    }
}
