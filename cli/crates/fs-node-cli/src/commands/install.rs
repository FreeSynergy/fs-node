// commands/install.rs — `fsn install` — package installation from store or local path.
//
// Subcommand variants (via flags):
//   fsn install <name>            → fetch from store, install via InstallerRegistry
//   fsn install --from <path>     → install from local path (any ResourceType)
//   fsn install --list            → list installed packages from DB
//   fsn install --check <name>    → check prerequisites only, do not install
//   fsn install --dry-run <name>  → show what would happen, write nothing
//
// Install flow:
//   1. Resolve package (store or local path).
//   2. Check platform requirements (existing helper).
//   3. Check prerequisites via InstallerRegistry.
//   4. Install via InstallerRegistry.
//   5. Record in InstalledPackageRepo.
//
// Design: Chain of Responsibility (steps 2-4 in sequence).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fs_db::InstalledPackageRepo;
use fs_node_core::store::{Catalog, NodeStoreClient, StoreEntry};
use fs_pkg::{InstallPaths, InstallerRegistry};
use fs_sysinfo::{OsType, SysInfoCache};
use fs_types::{
    platform_filter_from_tags, FsTag, OsFamily, RequiredFeature, ResourceMeta, ResourceType,
    SemVer, ValidationStatus,
};

// ── run ───────────────────────────────────────────────────────────────────────

/// Main entry point for `fsn install`.
///
/// Dispatches to the correct sub-flow based on flags.
pub async fn run(
    _root: &Path,
    package: Option<&str>,
    from: Option<&Path>,
    list: bool,
    check: bool,
    dry_run: bool,
) -> Result<()> {
    // --list: show installed packages
    if list {
        return cmd_list().await;
    }

    // --from <path>: install from local path
    if let Some(src_path) = from {
        let name = package.unwrap_or_else(|| {
            src_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
        });
        return cmd_install_local(name, src_path, check, dry_run).await;
    }

    // <name>: install from store
    let name = package.ok_or_else(|| {
        anyhow::anyhow!(
            "Provide a package name or use --from <path> for local install.\n\
             Run `fsn install --list` to see installed packages.\n\
             Run `fsn store search` to browse available packages."
        )
    })?;

    cmd_install_from_store(name, check, dry_run).await
}

// ── cmd_list ──────────────────────────────────────────────────────────────────

/// List all installed packages from the local database.
async fn cmd_list() -> Result<()> {
    let Some(conn) = crate::db::get_conn() else {
        println!("No packages installed (database not initialized).");
        return Ok(());
    };
    let repo = InstalledPackageRepo::new(conn.inner());
    let packages = repo
        .list_all()
        .await
        .context("reading installed packages")?;

    let active: Vec<_> = packages.iter().filter(|p| p.active).collect();

    if active.is_empty() {
        println!("No packages installed.");
        println!("Run `fsn store search` to browse available packages.");
        return Ok(());
    }

    println!("{:<28} {:<10} {:<8} TYPE", "PACKAGE", "VERSION", "CHANNEL");
    println!("{}", "─".repeat(60));
    for pkg in &active {
        println!(
            "{:<28} {:<10} {:<8} {}",
            pkg.package_id, pkg.version, pkg.channel, pkg.package_type
        );
    }
    println!("\n{} package(s) installed.", active.len());
    Ok(())
}

// ── cmd_install_from_store ────────────────────────────────────────────────────

/// Install a package from the store catalog.
async fn cmd_install_from_store(name: &str, check_only: bool, dry_run: bool) -> Result<()> {
    // 1. Fetch catalog.
    let mut client = NodeStoreClient::node_store();
    let catalog: Catalog<StoreEntry> = client
        .fetch_catalog("node", false)
        .await
        .context("fetching store catalog")?;

    // 2. Find entry.
    let entry = catalog
        .packages
        .iter()
        .find(|e| e.id == name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Package '{}' not found in store catalog.\n\
             Run `fsn store search` to browse available packages.",
                name
            )
        })?;

    // 3. Platform check.
    check_platform_requirements(entry)?;

    // 4. Build ResourceMeta (store entries are Container resources).
    let meta = store_entry_to_meta(entry, ResourceType::Container);

    // 5. Prerequisites via InstallerRegistry.
    let paths = InstallPaths::load();
    let registry = InstallerRegistry::new();

    if let Err(e) = registry.check_prerequisites(meta.resource_type, &meta) {
        anyhow::bail!(
            "Prerequisites not met for '{}':\n  {}\n\
             Fix the issue above and try again.",
            name,
            e
        );
    }

    if check_only {
        println!("Prerequisites satisfied for '{}'.", name);
        println!("  Version:  {}", entry.version);
        println!("  Type:     {}", meta.resource_type.label());
        let install_path = paths.install_path_for(meta.resource_type, &meta.id);
        if !install_path.is_empty() {
            println!("  Would install to: {install_path}");
        }
        return Ok(());
    }

    // 6. Check if already installed.
    if let Some(conn) = crate::db::get_conn() {
        let repo = InstalledPackageRepo::new(conn.inner());
        if let Ok(Some(existing)) = repo.find_active(name).await {
            if existing.version == entry.version {
                println!(
                    "'{}' is already installed at version {}.",
                    name, entry.version
                );
                println!("Run `fsn update` to check for newer versions.");
                return Ok(());
            }
        }
    }

    // 7. Install.
    let report = registry
        .install(&meta, None, &paths, dry_run)
        .map_err(|e| anyhow::anyhow!("Installation failed: {e}"))?;

    println!("{}", report.summary);

    // 8. Record in DB.
    if !dry_run {
        if let Some(conn) = crate::db::get_conn() {
            let repo = InstalledPackageRepo::new(conn.inner());
            // Deactivate any previous version.
            for old in repo
                .list_all()
                .await
                .unwrap_or_default()
                .iter()
                .filter(|p| p.package_id == name && p.active)
            {
                let _ = repo.set_active(old.id, false).await;
            }
            let _ = repo
                .insert(
                    name,
                    &entry.version,
                    "stable",
                    meta.resource_type.label(),
                    None,
                    true,
                )
                .await;
        }
    }

    // 9. For Container resources: also add to project config for `fsn deploy`.
    if meta.resource_type == ResourceType::Container && !dry_run {
        println!(
            "\nContainer '{}' registered. Run `fsn deploy` to start it.",
            name
        );
    }

    Ok(())
}

// ── cmd_install_local ─────────────────────────────────────────────────────────

/// Install a package from a local path.
async fn cmd_install_local(name: &str, src: &Path, check_only: bool, dry_run: bool) -> Result<()> {
    if !src.exists() {
        anyhow::bail!("Path does not exist: {}", src.display());
    }

    // Determine ResourceType: try to read from resource-type.txt or default to App.
    let rt = detect_resource_type(src);
    let meta = build_local_meta(name, rt, src);

    let paths = InstallPaths::load();
    let registry = InstallerRegistry::new();

    if let Err(e) = registry.check_prerequisites(rt, &meta) {
        anyhow::bail!("Prerequisites not met:\n  {e}\nFix the issue above and try again.");
    }

    if check_only {
        println!(
            "Prerequisites satisfied for '{name}' (type: {}).",
            rt.label()
        );
        let install_path = paths.install_path_for(rt, name);
        if !install_path.is_empty() {
            println!("Would install to: {install_path}");
        }
        return Ok(());
    }

    let report = registry
        .install(&meta, Some(src), &paths, dry_run)
        .map_err(|e| anyhow::anyhow!("Installation failed: {e}"))?;

    println!("{}", report.summary);

    if !dry_run {
        if let Some(conn) = crate::db::get_conn() {
            let repo = InstalledPackageRepo::new(conn.inner());
            for old in repo
                .list_all()
                .await
                .unwrap_or_default()
                .iter()
                .filter(|p| p.package_id == name && p.active)
            {
                let _ = repo.set_active(old.id, false).await;
            }
            let _ = repo
                .insert(name, "local", "local", rt.label(), None, true)
                .await;
        }
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a minimal `ResourceMeta` from a store entry.
///
/// Uses `resource_type` (caller determines from context).
fn store_entry_to_meta(entry: &StoreEntry, resource_type: ResourceType) -> ResourceMeta {
    ResourceMeta {
        id: entry.id.clone(),
        name: entry.name.clone(),
        summary: entry.description.clone(),
        description: entry.description.clone(),
        description_file: PathBuf::new(),
        version: entry.version.parse::<SemVer>().unwrap_or(SemVer {
            major: 0,
            minor: 0,
            patch: 1,
            pre: None,
        }),
        author: entry.author.clone().unwrap_or_default(),
        license: entry.license.clone().unwrap_or_default(),
        icon: PathBuf::new(),
        tags: entry.tags.iter().map(|t| FsTag::new(t.as_str())).collect(),
        resource_type,
        dependencies: Vec::new(),
        signature: None,
        status: ValidationStatus::Incomplete,
        source: None,
        platform: None,
    }
}

/// Build a minimal `ResourceMeta` for a local package install.
fn build_local_meta(name: &str, resource_type: ResourceType, _src: &Path) -> ResourceMeta {
    ResourceMeta {
        id: name.to_owned(),
        name: name.to_owned(),
        summary: String::new(),
        description: String::new(),
        description_file: PathBuf::new(),
        version: SemVer {
            major: 0,
            minor: 0,
            patch: 1,
            pre: None,
        },
        author: String::new(),
        license: String::new(),
        icon: PathBuf::new(),
        tags: Vec::new(),
        resource_type,
        dependencies: Vec::new(),
        signature: None,
        status: ValidationStatus::Incomplete,
        source: None,
        platform: None,
    }
}

/// Detect ResourceType from a local package path.
///
/// Reads `resource-type.txt` in the directory if present, otherwise defaults to `App`.
fn detect_resource_type(src: &Path) -> ResourceType {
    let type_file = if src.is_dir() {
        src.join("resource-type.txt")
    } else {
        return ResourceType::App;
    };

    if let Ok(content) = std::fs::read_to_string(&type_file) {
        match content.trim() {
            "app" => ResourceType::App,
            "container" => ResourceType::Container,
            "widget" => ResourceType::Widget,
            "bot" => ResourceType::Bot,
            "bridge" => ResourceType::Bridge,
            "bundle" => ResourceType::Bundle,
            "task" => ResourceType::Task,
            "language" => ResourceType::Language,
            "color_scheme" => ResourceType::ColorScheme,
            "style" => ResourceType::Style,
            "font_set" => ResourceType::FontSet,
            "cursor_set" => ResourceType::CursorSet,
            "icon_set" => ResourceType::IconSet,
            "button_style" => ResourceType::ButtonStyle,
            "window_chrome" => ResourceType::WindowChrome,
            "animation_set" => ResourceType::AnimationSet,
            "messenger_adapter" => ResourceType::MessengerAdapter,
            _ => ResourceType::App,
        }
    } else {
        ResourceType::App
    }
}

/// Check whether the host satisfies the package's `platform:*` and `requires:*` tags.
fn check_platform_requirements(entry: &StoreEntry) -> Result<()> {
    let ftags: Vec<FsTag> = entry.tags.iter().map(|t| FsTag::new(t.as_str())).collect();
    let Some(filter) = platform_filter_from_tags(&ftags) else {
        return Ok(());
    };

    let cache = SysInfoCache::default_path();
    let (os_info, features) = cache.get_or_detect();

    let current_os = match os_info.os_type {
        OsType::Linux => OsFamily::Linux,
        OsType::MacOs => OsFamily::MacOs,
        OsType::Windows => OsFamily::Windows,
        OsType::Unknown => OsFamily::Any,
    };

    let available: Vec<RequiredFeature> = features
        .available
        .iter()
        .filter_map(|f| RequiredFeature::from_tag(f.label()))
        .collect();

    let unmet = filter.unmet(current_os, &available);
    if unmet.is_empty() {
        return Ok(());
    }

    anyhow::bail!(
        "Package '{}' cannot be installed on this system.\n  Unmet requirements:\n{}",
        entry.id,
        unmet
            .iter()
            .map(|u| format!("    • {u}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
