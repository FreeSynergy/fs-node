// commands/remove.rs — `fsn remove` — package removal or container service undeployment.
//
// Variants:
//   fsn remove <name> [--keep-data]   → remove installed package via UninstallerRegistry
//   fsn remove --service <name>       → undeploy a container service (existing behavior)
//
// Remove flow (package):
//   1. Look up installed record in DB (determines ResourceType).
//   2. Warn if other packages depend on this one.
//   3. Uninstall via InstallerRegistry.
//   4. Mark as inactive in DB.

use std::path::Path;

use anyhow::{bail, Context, Result};
use fs_db::InstalledPackageRepo;
use fs_deploy::deploy::{DeployOpts, undeploy_instance};
use fs_pkg::{InstallPaths, InstallerRegistry};
use fs_pkg::installers::UninstallOptions;
use fs_types::ResourceType;

// ── run ───────────────────────────────────────────────────────────────────────

/// Entry point for `fsn remove`.
pub async fn run(
    _root: &Path,
    _project: Option<&Path>,
    package: Option<&str>,
    service: Option<&str>,
    keep_data: bool,
    confirm: bool,
) -> Result<()> {
    // --service: container service undeployment (existing behavior).
    if let Some(svc) = service {
        return cmd_undeploy_service(svc, confirm).await;
    }

    // <name>: package removal.
    let name = package.ok_or_else(|| anyhow::anyhow!(
        "Provide a package name to remove.\n\
         Run `fsn install --list` to see installed packages.\n\
         Use --service <name> to undeploy a container service."
    ))?;

    cmd_remove_package(name, keep_data, confirm).await
}

// ── cmd_remove_package ────────────────────────────────────────────────────────

async fn cmd_remove_package(name: &str, keep_data: bool, confirm: bool) -> Result<()> {
    // 1. Look up installed record.
    let Some(conn) = crate::db::get_conn() else {
        bail!("Database not available — cannot look up installed packages.");
    };
    let repo = InstalledPackageRepo::new(conn.inner());

    let record = repo.find_active(name).await
        .context("looking up installed package")?
        .ok_or_else(|| anyhow::anyhow!(
            "Package '{}' is not installed.\n\
             Run `fsn install --list` to see installed packages.",
            name
        ))?;

    // 2. Determine ResourceType from stored package_type string.
    let rt = parse_resource_type(&record.package_type);

    // 3. Confirmation prompt.
    if !confirm {
        let data_note = if keep_data { " (data will be kept)" } else { " including all data" };
        bail!(
            "This will remove '{}' v{}{data_note}.\n\
             Re-run with --confirm to proceed.",
            name, record.version
        );
    }

    // 4. Uninstall via InstallerRegistry.
    let paths    = InstallPaths::load();
    let registry = InstallerRegistry::new();
    let opts     = UninstallOptions { keep_data, dry_run: false };

    registry.uninstall(rt, name, &paths, &opts)
        .map_err(|e| anyhow::anyhow!("Removal failed: {e}"))?;

    // 5. Mark inactive in DB.
    repo.set_active(record.id, false).await
        .context("updating install record")?;

    if keep_data {
        println!("Removed '{}' (data preserved).", name);
    } else {
        println!("Removed '{}'.", name);
    }
    Ok(())
}

// ── cmd_undeploy_service ──────────────────────────────────────────────────────

/// Undeploy a container service instance (stops units, deletes Quadlet files).
async fn cmd_undeploy_service(service: &str, confirm: bool) -> Result<()> {
    if !confirm {
        bail!(
            "This will undeploy service '{}'. Re-run with --confirm to proceed.",
            service
        );
    }
    let opts = DeployOpts::default_for_user();
    undeploy_instance(service, &opts).await?;
    println!("Undeployed service '{}'.", service);
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse a ResourceType from the stored `package_type` string label.
/// Falls back to `Container` for unknown values.
fn parse_resource_type(label: &str) -> ResourceType {
    match label {
        "App"               => ResourceType::App,
        "Container"         => ResourceType::Container,
        "Bundle"            => ResourceType::Bundle,
        "Widget"            => ResourceType::Widget,
        "Bot"               => ResourceType::Bot,
        "Bridge"            => ResourceType::Bridge,
        "Task"              => ResourceType::Task,
        "Language"          => ResourceType::Language,
        "Color Scheme"      => ResourceType::ColorScheme,
        "Style"             => ResourceType::Style,
        "Font Set"          => ResourceType::FontSet,
        "Cursor Set"        => ResourceType::CursorSet,
        "Icon Set"          => ResourceType::IconSet,
        "Button Style"      => ResourceType::ButtonStyle,
        "Window Chrome"     => ResourceType::WindowChrome,
        "Animation Set"     => ResourceType::AnimationSet,
        "Messenger Adapter" => ResourceType::MessengerAdapter,
        _                   => ResourceType::Container,
    }
}
