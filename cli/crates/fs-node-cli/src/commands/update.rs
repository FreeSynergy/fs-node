// commands/update.rs — `fsn update` — package update or container service redeploy.
//
// Variants:
//   fsn update <name>          → update installed package via Updater
//   fsn update --all           → update all installed packages
//   fsn update --service <name>→ redeploy container service (existing behavior)
//
// Update flow (package):
//   1. Fetch latest version from store catalog.
//   2. Compare with installed version.
//   3. If newer: run Updater (uninstall keep-data → install new → verify).
//   4. Record new version in DB.

use std::path::Path;

use anyhow::{Context, Result};
use fs_db::InstalledPackageRepo;
use fs_node_core::store::StoreEntry;
use fs_pkg::{
    channel::ReleaseChannel,
    install_paths::InstallPaths,
    installer_registry::InstallerRegistry,
    updater::{BatchUpdateOutcome, Updater},
};
use fs_store::StoreClient;
use fs_types::{ResourceMeta, ResourceType, ValidationStatus};

use std::path::PathBuf;

// ── run ───────────────────────────────────────────────────────────────────────

/// Entry point for `fsn update`.
pub async fn run(
    root: &Path,
    project: Option<&Path>,
    package: Option<&str>,
    service: Option<&str>,
    all: bool,
    dry_run: bool,
) -> Result<()> {
    // --service: container service redeploy (existing behavior).
    if let Some(svc) = service {
        return crate::commands::deploy::run(root, project, Some(svc), None).await;
    }

    // --all: update every installed package.
    if all {
        return cmd_update_all(dry_run).await;
    }

    // <name>: update a specific package.
    let name = package.ok_or_else(|| anyhow::anyhow!(
        "Provide a package name or use --all to update everything.\n\
         Run `fsn install --list` to see installed packages."
    ))?;

    cmd_update_one(name, dry_run).await
}

// ── cmd_update_one ────────────────────────────────────────────────────────────

async fn cmd_update_one(name: &str, dry_run: bool) -> Result<()> {
    let Some(conn) = crate::db::get_conn() else {
        anyhow::bail!("Database not available — cannot look up installed packages.");
    };
    let repo = InstalledPackageRepo::new(conn.inner());

    let record = repo.find_active(name).await
        .context("looking up installed package")?
        .ok_or_else(|| anyhow::anyhow!(
            "Package '{}' is not installed.\n\
             Run `fsn install {}` to install it first.",
            name, name
        ))?;

    // Fetch latest version from store.
    let catalog = fetch_catalog().await?;
    let entry = catalog.packages.iter()
        .find(|e| e.id == name)
        .ok_or_else(|| anyhow::anyhow!(
            "Package '{}' not found in store catalog — cannot check for updates.",
            name
        ))?;

    let meta = store_entry_to_meta(entry, ResourceType::Container);

    let paths   = InstallPaths::load();
    let updater = Updater::new(InstallerRegistry::new(), paths);

    let outcome = updater.update(
        &meta,
        None,
        &record.version,
        ReleaseChannel::Stable,
        dry_run,
    ).map_err(|e| anyhow::anyhow!("Update failed: {e}"))?;

    println!("{}", outcome.report.summary);

    if !dry_run && outcome.old_version != outcome.new_version {
        // Record new version in DB.
        let _ = repo.set_active(record.id, false).await;
        let _ = repo.insert(
            name,
            &outcome.new_version,
            "stable",
            meta.resource_type.label(),
            None,
            true,
        ).await;
        println!("Updated '{}': {} → {}", name, outcome.old_version, outcome.new_version);
    }

    Ok(())
}

// ── cmd_update_all ────────────────────────────────────────────────────────────

async fn cmd_update_all(dry_run: bool) -> Result<()> {
    let Some(conn) = crate::db::get_conn() else {
        anyhow::bail!("Database not available.");
    };
    let repo = InstalledPackageRepo::new(conn.inner());

    let all = repo.list_all().await.context("reading installed packages")?;
    let active: Vec<_> = all.iter().filter(|p| p.active).collect();

    if active.is_empty() {
        println!("No packages installed.");
        return Ok(());
    }

    let catalog = fetch_catalog().await?;
    let paths   = InstallPaths::load();
    let updater = Updater::new(InstallerRegistry::new(), paths.clone());

    let mut outcome = BatchUpdateOutcome::default();

    for record in &active {
        let Some(entry) = catalog.packages.iter().find(|e| e.id == record.package_id) else {
            outcome.skipped.push(format!("{} (not in catalog)", record.package_id));
            continue;
        };

        let meta = store_entry_to_meta(entry, ResourceType::Container);

        match updater.update(&meta, None, &record.version, ReleaseChannel::Stable, dry_run) {
            Ok(u) => {
                if u.old_version == u.new_version {
                    outcome.skipped.push(record.package_id.clone());
                } else {
                    if !dry_run {
                        let _ = repo.set_active(record.id, false).await;
                        let _ = repo.insert(
                            &record.package_id,
                            &u.new_version,
                            "stable",
                            meta.resource_type.label(),
                            None,
                            true,
                        ).await;
                    }
                    outcome.updated.push(u);
                }
            }
            Err(e) => {
                outcome.failures.push((record.package_id.clone(), e.to_string()));
            }
        }
    }

    outcome.print_summary();
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn fetch_catalog() -> Result<fs_store::Catalog<StoreEntry>> {
    let mut client = StoreClient::node_store();
    client.fetch_catalog("node", false)
        .await
        .context("fetching store catalog")
}

fn store_entry_to_meta(entry: &StoreEntry, resource_type: ResourceType) -> ResourceMeta {
    ResourceMeta {
        id:            entry.id.clone(),
        name:          entry.name.clone(),
        description:   entry.description.clone(),
        version:       entry.version.clone(),
        author:        entry.author.clone().unwrap_or_default(),
        license:       entry.license.clone().unwrap_or_default(),
        icon:          PathBuf::new(),
        tags:          entry.tags.clone(),
        resource_type,
        dependencies:  Vec::new(),
        signature:     None,
        status:        ValidationStatus::Incomplete,
        source:        None,
        platform:      None,
    }
}
