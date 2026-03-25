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
    #[arg(long, global = true, env = "FS_ROOT")]
    pub root: Option<PathBuf>,

    /// Path to the project config file
    #[arg(long, global = true, env = "FS_PROJECT")]
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

    /// Update an installed package or redeploy a container service
    Update {
        /// Package name to update (e.g. "kanidm")
        package: Option<String>,
        /// Redeploy only this container service instance
        #[arg(long)]
        service: Option<String>,
        /// Update all installed packages
        #[arg(long)]
        all: bool,
        /// Preview without applying changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Restart services
    Restart {
        /// Restart only this service instance
        #[arg(long)]
        service: Option<String>,
    },

    /// Remove an installed package or permanently remove a container service
    Remove {
        /// Package name to remove (e.g. "kanidm")
        package: Option<String>,
        /// Remove only this deployed container service instance (stops + deletes)
        #[arg(long)]
        service: Option<String>,
        /// Keep data directories; only remove binaries and config files
        #[arg(long)]
        keep_data: bool,
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

    /// Interactive first-time setup wizard (replaces fs-install.sh for ongoing use)
    Init,

    /// Open the terminal UI dashboard
    Tui,

    /// Compose YAML → Quadlet pipeline (analyze, install, start/stop/restart/logs)
    Container {
        #[command(subcommand)]
        cmd: ContainerCommand,
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

    /// Install a package from the store or a local path
    Install {
        /// Package ID to install (e.g. "iam/kanidm") — omit when using --list
        package: Option<String>,
        /// Install from a local path instead of the store
        #[arg(long, value_name = "PATH")]
        from: Option<std::path::PathBuf>,
        /// List all installed packages
        #[arg(long)]
        list: bool,
        /// Check prerequisites only, do not install
        #[arg(long)]
        check: bool,
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

    /// Message bus daemon and event routing
    Bus {
        #[command(subcommand)]
        cmd: BusCommand,
    },

    /// Show system information (OS, features, disk, memory, CPU temperature)
    Sysinfo {
        /// Show live on-demand data (disk, memory, temperature) instead of cached static info
        #[arg(long)]
        live: bool,

        /// Clear the 24-hour cache and re-detect immediately
        #[arg(long)]
        refresh: bool,

        /// Check if a specific feature is available (e.g. systemd, podman, git)
        #[arg(long, value_name = "FEATURE")]
        check: Option<String>,

        /// Run a continuous alert monitor loop; publish alerts to the bus if reachable
        #[arg(long)]
        monitor: bool,

        /// Alert check interval in seconds (only with --monitor, default: 300)
        #[arg(long, default_value = "300")]
        interval: u64,

        /// Disk-full alert threshold in percent (default: 90)
        #[arg(long, default_value = "90")]
        disk_threshold: f64,

        /// Memory-full alert threshold in percent (default: 90)
        #[arg(long, default_value = "90")]
        mem_threshold: f64,

        /// CPU temperature alert threshold in degrees Celsius (default: 85)
        #[arg(long, default_value = "85")]
        cpu_threshold: f32,
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
pub enum ContainerCommand {
    /// Parse + analyze a compose YAML file and show a variable report
    Analyze {
        /// Path to docker-compose.yml or Podman compose file
        file: std::path::PathBuf,

        /// Override instance name (default: first service name in compose file)
        #[arg(long)]
        name: Option<String>,

        /// Do not contact the store for enrichment
        #[arg(long)]
        offline: bool,
    },

    /// Install a compose YAML file as a Quadlet-managed service
    Install {
        /// Path to docker-compose.yml or Podman compose file
        file: std::path::PathBuf,

        /// Override instance name (default: first service name in compose file)
        #[arg(long)]
        name: Option<String>,

        /// Validate and show what would be written — do not write files
        #[arg(long)]
        dry_run: bool,

        /// Store API base URL for enrichment (e.g. http://localhost:8080)
        #[arg(long)]
        store_url: Option<String>,
    },

    /// Start a container-app-managed service instance via systemctl
    Start {
        /// Instance name (e.g. "kanidm")
        service: String,
    },

    /// Stop a container-app-managed service instance via systemctl
    Stop {
        /// Instance name
        service: String,
    },

    /// Restart a container-app-managed service instance via systemctl
    Restart {
        /// Instance name
        service: String,
    },

    /// Show recent logs for a container-app-managed service (via journalctl)
    Logs {
        /// Instance name
        service: String,
        /// Number of log lines to show
        #[arg(short, long, default_value = "50")]
        lines: usize,
    },

    /// Show systemctl status of a container-app-managed service
    Status {
        /// Instance name
        service: String,
    },

    /// List all container-app-managed systemd services
    List,
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
pub enum BusCommand {
    /// Start the bus REST + WebSocket server
    Serve {
        /// Port to listen on
        #[arg(long, default_value = "8081")]
        port: u16,
        /// Bind address
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,
        /// Path to routing config TOML (optional)
        #[arg(long)]
        config: Option<String>,
    },
    /// Show current bus status (subscriptions + standing orders)
    Status,
    /// Publish a single event to the running bus
    Publish {
        /// Event topic (e.g. "deploy.started")
        #[arg(long)]
        topic: String,
        /// Source role or service name
        #[arg(long, default_value = "cli")]
        source: String,
        /// JSON payload (optional)
        #[arg(long)]
        payload: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Show the merged resolved config (module defaults + project.yml)
    Show,

    /// Open project.yml in $EDITOR
    Edit,

    /// Validate config files and check constraints
    Validate,

    /// Manage installation base paths (show, set, migrate)
    InstallRoot {
        #[command(subcommand)]
        cmd: InstallRootCommand,
    },
}

#[derive(Subcommand)]
pub enum InstallRootCommand {
    /// Show all current installation base paths
    Show,

    /// Change a base path (does NOT move existing files)
    ///
    /// Available bases: system, config, font, icon, cursor
    Set {
        /// Which base to change (system | config | font | icon | cursor)
        base: String,
        /// New absolute path
        path: std::path::PathBuf,
    },

    /// Change a base path AND move all existing installed files there
    ///
    /// Uses rename (mv) or copy+delete for cross-filesystem moves.
    Migrate {
        /// Which base to change (system | config | font | icon | cursor)
        base: String,
        /// New absolute path
        path: std::path::PathBuf,
    },
}

/// Parse args and dispatch to the right command handler.
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Resolve FSN root: --root flag > env var > auto-detect
    let root = cli
        .root
        .or_else(|| std::env::var("FS_ROOT").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));

    match cli.command {
        Command::Deploy { service, host } => {
            commands::deploy::run(
                &root,
                cli.project.as_deref(),
                service.as_deref(),
                host.as_deref(),
            )
            .await
        }
        Command::Undeploy { service } => {
            commands::undeploy::run(&root, cli.project.as_deref(), service.as_deref()).await
        }
        Command::Update {
            package,
            service,
            all,
            dry_run,
        } => {
            commands::update::run(
                &root,
                cli.project.as_deref(),
                package.as_deref(),
                service.as_deref(),
                all,
                dry_run,
            )
            .await
        }
        Command::Restart { service } => {
            commands::restart::run(&root, cli.project.as_deref(), service.as_deref()).await
        }
        Command::Remove {
            package,
            service,
            keep_data,
            confirm,
        } => {
            commands::remove::run(
                &root,
                cli.project.as_deref(),
                package.as_deref(),
                service.as_deref(),
                keep_data,
                confirm,
            )
            .await
        }
        Command::Clean => commands::clean::run(&root, cli.project.as_deref()).await,
        Command::Sync => commands::sync::run(&root, cli.project.as_deref()).await,
        Command::Status => commands::status::run(&root, cli.project.as_deref()).await,
        Command::Logs { service, follow } => commands::logs::run(&service, follow).await,
        Command::Config { cmd } => commands::config::run(&root, cli.project.as_deref(), cmd).await,
        Command::Serve { port, bind } => {
            commands::serve::run(&root, cli.project.as_deref(), &bind, port).await
        }
        Command::Init => commands::init::run(&root).await,
        Command::Tui => commands::tui::run(&root).await,
        Command::Container { cmd } => match cmd {
            ContainerCommand::Analyze {
                file,
                name,
                offline,
            } => {
                commands::container::ContainerCmd
                    .analyze(&file, name.as_deref(), offline)
                    .await
            }
            ContainerCommand::Install {
                file,
                name,
                dry_run,
                store_url,
            } => {
                commands::container::ContainerCmd
                    .install(&file, name.as_deref(), dry_run, store_url.as_deref())
                    .await
            }
            ContainerCommand::Start { service } => {
                commands::container::ContainerCmd.start(&service).await
            }
            ContainerCommand::Stop { service } => {
                commands::container::ContainerCmd.stop(&service).await
            }
            ContainerCommand::Restart { service } => {
                commands::container::ContainerCmd.restart(&service).await
            }
            ContainerCommand::Logs { service, lines } => {
                commands::container::ContainerCmd
                    .logs(&service, lines)
                    .await
            }
            ContainerCommand::Status { service } => {
                commands::container::ContainerCmd.status(&service).await
            }
            ContainerCommand::List => commands::container::ContainerCmd.list().await,
        },
        Command::Store { cmd } => match cmd {
            StoreCommand::Search { query } => commands::store::StoreCmd.search(&query).await,
            StoreCommand::Info { id } => commands::store::StoreCmd.info(&id).await,
            StoreCommand::Install { id } => commands::store::StoreCmd.install(&id).await,
            StoreCommand::Update => commands::store::StoreCmd.update_check().await,
            StoreCommand::List { r#type } => {
                commands::store::PackageCmd.list(r#type.as_deref()).await
            }
            StoreCommand::Remove { id, confirm } => {
                commands::store::PackageCmd.remove(&id, confirm).await
            }
            StoreCommand::Rollback { id, version } => {
                commands::store::PackageCmd
                    .rollback(&id, version.as_deref())
                    .await
            }
            StoreCommand::Sync => commands::store::StoreCmd.sync().await,
            StoreCommand::I18n { cmd } => match cmd {
                I18nCommand::Status => commands::store::I18nCmd.status().await,
                I18nCommand::Set { lang } => commands::store::I18nCmd.set(&lang).await,
                I18nCommand::Check => commands::store::I18nCmd.check().await,
            },
            StoreCommand::Theme { cmd } => match cmd {
                PackageAssetCommand::Available { query } => {
                    commands::store::AssetCmd::theme().available(&query).await
                }
                PackageAssetCommand::List => commands::store::AssetCmd::theme().list().await,
                PackageAssetCommand::Install { id, dry_run } => {
                    commands::store::AssetCmd::theme()
                        .install(&id, dry_run)
                        .await
                }
                PackageAssetCommand::Remove { id, confirm } => {
                    commands::store::AssetCmd::theme()
                        .remove(&id, confirm)
                        .await
                }
            },
            StoreCommand::Widget { cmd } => match cmd {
                PackageAssetCommand::Available { query } => {
                    commands::store::AssetCmd::widget().available(&query).await
                }
                PackageAssetCommand::List => commands::store::AssetCmd::widget().list().await,
                PackageAssetCommand::Install { id, dry_run } => {
                    commands::store::AssetCmd::widget()
                        .install(&id, dry_run)
                        .await
                }
                PackageAssetCommand::Remove { id, confirm } => {
                    commands::store::AssetCmd::widget()
                        .remove(&id, confirm)
                        .await
                }
            },
        },
        Command::Server { cmd } => match cmd {
            ServerCommand::Setup => commands::server_setup::run(&root).await,
        },
        Command::Install {
            package,
            from,
            list,
            check,
            dry_run,
        } => {
            commands::install::run(
                &root,
                package.as_deref(),
                from.as_deref(),
                list,
                check,
                dry_run,
            )
            .await
        }
        Command::Export { output } => {
            commands::export_import::export(&root, cli.project.as_deref(), &output).await
        }
        Command::Import { input } => commands::export_import::import(&root, &input).await,
        Command::Deps { service } => {
            commands::deps::run(&root, cli.project.as_deref(), &service).await
        }
        Command::Storage { cmd } => match cmd {
            StorageCommand::Status => commands::storage::status(&root).await,
            StorageCommand::Init => commands::storage::init(&root).await,
            StorageCommand::Profile { cmd } => commands::storage::profile(&root, cmd).await,
            StorageCommand::Sync { cmd } => commands::storage::sync(&root, cmd).await,
        },
        Command::Bus { cmd } => match cmd {
            BusCommand::Serve { port, bind, config } => {
                commands::bus::serve(&bind, port, config.as_deref()).await
            }
            BusCommand::Status => commands::bus::status().await,
            BusCommand::Publish {
                topic,
                source,
                payload,
            } => commands::bus::publish_event(&topic, &source, payload.as_deref()).await,
        },
        Command::Sysinfo {
            live,
            refresh,
            check,
            monitor,
            interval,
            disk_threshold,
            mem_threshold,
            cpu_threshold,
        } => {
            if let Some(feature) = check {
                commands::sysinfo::check_feature(&feature).await
            } else if refresh {
                commands::sysinfo::refresh().await
            } else if live {
                commands::sysinfo::live().await
            } else if monitor {
                let thresholds = fs_sysinfo::AlertThresholds {
                    disk_full_percent: disk_threshold,
                    memory_full_percent: mem_threshold,
                    cpu_hot_celsius: cpu_threshold,
                };
                commands::sysinfo::monitor(interval, thresholds).await
            } else {
                commands::sysinfo::info().await
            }
        }
    }
}
