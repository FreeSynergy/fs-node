// Top-level CLI definition (clap) and command dispatch.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::commands;

/// FSN – FreeSynergy.Node management tool
#[derive(Parser)]
#[command(
    name = "fsn",
    version,
    author,
    about = "FreeSynergy.Node – deploy and manage your self-hosted platform",
    long_about = None,
)]
pub struct Cli {
    /// Path to the FSN root directory (default: auto-detected)
    #[arg(long, global = true, env = "FSN_ROOT")]
    pub root: Option<PathBuf>,

    /// Path to the project config file
    #[arg(long, global = true, env = "FSN_PROJECT")]
    pub project: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Deploy all services (or a single service) to reach desired state
    Deploy {
        /// Deploy only this service instance (e.g. "forgejo")
        #[arg(long)]
        service: Option<String>,

        /// Deploy to this remote host by name (must match a *.host.toml file)
        #[arg(long)]
        host: Option<String>,
    },

    /// Stop services without removing data
    Undeploy {
        /// Undeploy only this service instance
        #[arg(long)]
        service: Option<String>,
    },

    /// Pull new images and redeploy modules where version changed
    Update {
        /// Update only this service instance
        #[arg(long)]
        service: Option<String>,
    },

    /// Restart services
    Restart {
        /// Restart only this service instance
        #[arg(long)]
        service: Option<String>,
    },

    /// Remove services and all their data permanently
    Remove {
        /// Remove only this service instance
        #[arg(long)]
        service: Option<String>,

        /// Skip the confirmation prompt
        #[arg(long)]
        confirm: bool,
    },

    /// Remove orphaned containers and volumes not in any project
    Clean,

    /// Show what would change without applying any changes (dry-run)
    Sync,

    /// Show running services and their health status
    Status,

    /// Show live logs for a service
    Logs {
        /// Service instance name (e.g. "forgejo")
        service: String,

        /// Follow log output (like tail -f)
        #[arg(short, long)]
        follow: bool,
    },

    /// Config file management
    Config {
        #[command(subcommand)]
        cmd: ConfigCommand,
    },

    /// Start the web management UI
    Serve {
        /// Port to listen on
        #[arg(long, default_value = "8080")]
        port: u16,

        /// Bind address
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,
    },

    /// Interactive first-time setup wizard (replaces fsn-install.sh for ongoing use)
    Init,

    /// Open the terminal UI dashboard
    Tui,

    /// Container management (start/stop/restart/logs/list)
    Conductor {
        #[command(subcommand)]
        cmd: ConductorCommand,
    },

    /// Module store (search/info/install/update)
    Store {
        #[command(subcommand)]
        cmd: StoreCommand,
    },

    /// Server-level administration (run as root)
    Server {
        #[command(subcommand)]
        cmd: ServerCommand,
    },

    /// Install a package from the store into the current project
    Install {
        /// Package ID (e.g. "git/forgejo")
        package: String,
        /// Preview without applying changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Export all project configuration to a portable TOML bundle
    Export {
        /// Output file path
        #[arg(long, short)]
        output: std::path::PathBuf,
    },

    /// Import project configuration from a TOML bundle
    Import {
        /// Input bundle file
        input: std::path::PathBuf,
    },

    /// Show the dependency graph for a service
    Deps {
        /// Service instance name (as declared in the project config)
        service: String,
    },

    /// Manage the embedded S3 storage server
    Storage {
        #[command(subcommand)]
        cmd: StorageCommand,
    },
}

#[derive(Subcommand)]
pub enum StorageCommand {
    /// Show bucket status (sizes, object counts)
    Status,

    /// Initialize bucket directory structure
    Init,

    /// Manage the local node's public profile
    Profile {
        #[command(subcommand)]
        cmd: ProfileCommand,
    },

    /// Sync data with a remote node (federation)
    Sync {
        #[command(subcommand)]
        cmd: StorageSyncCommand,
    },
}

#[derive(Subcommand)]
pub enum ProfileCommand {
    /// Show the current local profile
    Show,

    /// Set profile display name and metadata
    Set {
        /// Display name
        #[arg(long)]
        name: String,

        /// Short description of this node
        #[arg(long)]
        description: Option<String>,

        /// Public URL where this node is reachable
        #[arg(long)]
        public_url: Option<String>,
    },

    /// Upload an avatar image (png / jpg / webp)
    Avatar {
        /// Path to the image file
        file: std::path::PathBuf,
    },
}

#[derive(Subcommand)]
pub enum StorageSyncCommand {
    /// Pull a bucket from a remote node
    Pull {
        /// S3 endpoint of the remote node (e.g. http://peer.example:9000)
        #[arg(long)]
        remote_url: String,

        /// Bucket name (profiles, backups, media, packages, shared)
        #[arg(long)]
        bucket: String,

        /// S3 access key for the remote node
        #[arg(long)]
        access_key: String,

        /// S3 secret key for the remote node
        #[arg(long)]
        secret_key: String,
    },

    /// Push a local bucket to a remote node
    Push {
        /// S3 endpoint of the remote node
        #[arg(long)]
        remote_url: String,

        /// Bucket name
        #[arg(long)]
        bucket: String,

        /// S3 access key for the remote node
        #[arg(long)]
        access_key: String,

        /// S3 secret key for the remote node
        #[arg(long)]
        secret_key: String,
    },

    /// Fetch a remote node's public profile
    FetchProfile {
        /// S3 endpoint of the remote node
        #[arg(long)]
        remote_url: String,

        /// Node ID to fetch
        #[arg(long)]
        node_id: String,

        /// S3 access key (leave empty for public read)
        #[arg(long, default_value = "")]
        access_key: String,

        /// S3 secret key (leave empty for public read)
        #[arg(long, default_value = "")]
        secret_key: String,
    },
}

#[derive(Subcommand)]
pub enum ConductorCommand {
    /// List all containers and their state
    List {
        /// Include stopped containers
        #[arg(short, long)]
        all: bool,
    },
    /// Start a container
    Start {
        /// Container name
        service: String,
    },
    /// Stop a container
    Stop {
        /// Container name
        service: String,
    },
    /// Restart a container
    Restart {
        /// Container name
        service: String,
    },
    /// Show container logs
    Logs {
        /// Container name
        service: String,
        /// Follow log output (poll every second)
        #[arg(short, long)]
        follow: bool,
        /// Number of lines to show
        #[arg(short, long, default_value = "50")]
        tail: u64,
    },
}

#[derive(Subcommand)]
pub enum StoreCommand {
    /// Search the module catalog
    Search {
        /// Search query (leave empty to list all)
        #[arg(default_value = "")]
        query: String,
    },
    /// Show details for a module
    Info {
        /// Module ID (e.g. "iam/kanidm")
        id: String,
    },
    /// Show how to install a module
    Install {
        /// Module ID (e.g. "iam/kanidm")
        id: String,
    },
    /// Check for available updates
    Update,
    /// List all installed packages
    List {
        /// Filter by package type (app, container, language, theme, widget, …)
        #[arg(long)]
        r#type: Option<String>,
    },
    /// Remove an installed package
    Remove {
        /// Package ID (e.g. "lang/de", "theme/dark-pro")
        id: String,
        /// Skip confirmation prompt
        #[arg(long)]
        confirm: bool,
    },
    /// Roll back a package to a previous version
    Rollback {
        /// Package ID
        id: String,
        /// Target version (omit to roll back to the previous version)
        #[arg(long)]
        version: Option<String>,
    },
    /// Force-refresh the store catalog cache
    Sync,
    /// Manage UI language packs
    I18n {
        #[command(subcommand)]
        cmd: I18nCommand,
    },
    /// Manage themes
    Theme {
        #[command(subcommand)]
        cmd: PackageAssetCommand,
    },
    /// Manage widgets
    Widget {
        #[command(subcommand)]
        cmd: PackageAssetCommand,
    },
}

#[derive(Subcommand)]
pub enum I18nCommand {
    /// Show available languages and their completeness
    Status,
    /// Download and activate a language pack
    Set {
        /// BCP 47 language code (e.g. "de", "fr", "ja")
        lang: String,
    },
    /// Check if installed language is up to date with the current schema
    Check,
}

/// Subcommands for theme and widget management.
#[derive(Subcommand)]
pub enum PackageAssetCommand {
    /// List available packages in the catalog
    Available {
        /// Search query (leave empty to list all)
        #[arg(default_value = "")]
        query: String,
    },
    /// List installed packages
    List,
    /// Install a package from the store
    Install {
        /// Package ID
        id: String,
        /// Preview without applying changes
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove an installed package
    Remove {
        /// Package ID
        id: String,
        /// Skip confirmation prompt
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(Subcommand)]
pub enum ServerCommand {
    /// Prepare a server for FreeSynergy.Node (Podman, linger, unprivileged ports).
    /// Must be run as root or via sudo.
    Setup,
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Show the merged resolved config (module defaults + project.yml)
    Show,

    /// Open project.yml in $EDITOR
    Edit,

    /// Validate config files and check constraints
    Validate,
}

/// Parse args and dispatch to the right command handler.
pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Resolve FSN root: --root flag > env var > auto-detect
    let root = cli
        .root
        .or_else(|| std::env::var("FSN_ROOT").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));

    match cli.command {
        Command::Deploy { service, host }    => commands::deploy::run(&root, cli.project.as_deref(), service.as_deref(), host.as_deref()).await,
        Command::Undeploy { service }      => commands::undeploy::run(&root, cli.project.as_deref(), service.as_deref()).await,
        Command::Update { service }        => commands::update::run(&root, cli.project.as_deref(), service.as_deref()).await,
        Command::Restart { service }       => commands::restart::run(&root, cli.project.as_deref(), service.as_deref()).await,
        Command::Remove { service, confirm } => commands::remove::run(&root, cli.project.as_deref(), service.as_deref(), confirm).await,
        Command::Clean                     => commands::clean::run(&root, cli.project.as_deref()).await,
        Command::Sync                      => commands::sync::run(&root, cli.project.as_deref()).await,
        Command::Status                    => commands::status::run(&root, cli.project.as_deref()).await,
        Command::Logs { service, follow }  => commands::logs::run(&service, follow).await,
        Command::Config { cmd }            => commands::config::run(&root, cli.project.as_deref(), cmd).await,
        Command::Serve { port, bind }      => commands::serve::run(&root, cli.project.as_deref(), &bind, port).await,
        Command::Init                      => commands::init::run(&root).await,
        Command::Tui                       => commands::tui::run(&root).await,
        Command::Conductor { cmd }         => {
            let c = commands::conductor::Conductor::new()?;
            match cmd {
                ConductorCommand::List { all }                    => c.list(all).await,
                ConductorCommand::Start { service }               => c.start(&service).await,
                ConductorCommand::Stop { service }                => c.stop(&service).await,
                ConductorCommand::Restart { service }             => c.restart(&service).await,
                ConductorCommand::Logs { service, follow, tail }  => c.logs(&service, follow, tail).await,
            }
        },
        Command::Store { cmd }             => match cmd {
            StoreCommand::Search { query }  => commands::store::search(&query).await,
            StoreCommand::Info { id }       => commands::store::info(&id).await,
            StoreCommand::Install { id }    => commands::store::install(&id).await,
            StoreCommand::Update            => commands::store::update_check().await,
            StoreCommand::List { r#type }   => commands::store::list(r#type.as_deref()).await,
            StoreCommand::Remove { id, confirm } => commands::store::pkg_remove(&id, confirm).await,
            StoreCommand::Rollback { id, version } => commands::store::rollback(&id, version.as_deref()).await,
            StoreCommand::Sync              => commands::store::sync().await,
            StoreCommand::I18n { cmd }      => match cmd {
                I18nCommand::Status        => commands::store::i18n_status().await,
                I18nCommand::Set { lang }  => commands::store::i18n_set(&lang).await,
                I18nCommand::Check         => commands::store::i18n_check().await,
            },
            StoreCommand::Theme { cmd }     => match cmd {
                PackageAssetCommand::Available { query } => commands::store::asset_available("theme", &query).await,
                PackageAssetCommand::List                => commands::store::asset_list("theme").await,
                PackageAssetCommand::Install { id, dry_run } => commands::store::asset_install("theme", &id, dry_run).await,
                PackageAssetCommand::Remove { id, confirm }  => commands::store::asset_remove("theme", &id, confirm).await,
            },
            StoreCommand::Widget { cmd }    => match cmd {
                PackageAssetCommand::Available { query } => commands::store::asset_available("widget", &query).await,
                PackageAssetCommand::List                => commands::store::asset_list("widget").await,
                PackageAssetCommand::Install { id, dry_run } => commands::store::asset_install("widget", &id, dry_run).await,
                PackageAssetCommand::Remove { id, confirm }  => commands::store::asset_remove("widget", &id, confirm).await,
            },
        },
        Command::Server { cmd }            => match cmd {
            ServerCommand::Setup           => commands::server_setup::run(&root).await,
        },
        Command::Install { package, dry_run } => {
            commands::install::run(&root, &package, dry_run).await
        },
        Command::Export { output } => {
            commands::export_import::export(&root, cli.project.as_deref(), &output).await
        },
        Command::Import { input } => {
            commands::export_import::import(&root, &input).await
        },
        Command::Deps { service } => {
            commands::deps::run(&root, cli.project.as_deref(), &service).await
        },
        Command::Storage { cmd } => match cmd {
            StorageCommand::Status => commands::storage::status(&root).await,
            StorageCommand::Init   => commands::storage::init(&root).await,
            StorageCommand::Profile { cmd } => commands::storage::profile(&root, cmd).await,
            StorageCommand::Sync { cmd }    => commands::storage::sync(&root, cmd).await,
        },
    }
}
