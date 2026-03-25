// commands/install_root.rs — `fsn config install-root` — manage installation paths.
//
// Commands:
//   fsn config install-root show              → print current base paths
//   fsn config install-root set <base> <path> → change a base path
//   fsn config install-root migrate <base> <path> → change path AND move existing files
//
// Available bases: system, config, font, icon, cursor
//
// When migrating: PathMigrator moves all installed files using rename()/copy+delete.
// The caller (this command) updates the DB records after migration.

use std::path::PathBuf;

use anyhow::{bail, Result};
use fs_db::InstalledPackageRepo;
use fs_pkg::install_paths::{InstallPaths, PathMigrator};
use fs_types::ResourceType;

// ── show ──────────────────────────────────────────────────────────────────────

/// Print the current installation base paths.
pub async fn show() -> Result<()> {
    let paths = InstallPaths::load();
    println!(
        "FreeSynergy install paths ({})",
        InstallPaths::config_file_path().display()
    );
    println!();
    println!("  system   {}", paths.system_base.display());
    println!(
        "           Apps:              {}/apps/<name>/",
        paths.system_base.display()
    );
    println!(
        "           Bots:              {}/bots/<name>/",
        paths.system_base.display()
    );
    println!(
        "           Adapters:          {}/adapters/<name>/",
        paths.system_base.display()
    );
    println!();
    println!("  config   {}", paths.config_base.display());
    println!(
        "           Widgets:           {}/widgets/<name>/",
        paths.config_base.display()
    );
    println!(
        "           Tasks:             {}/tasks/<name>.toml",
        paths.config_base.display()
    );
    println!(
        "           i18n:              {}/i18n/<locale>/",
        paths.config_base.display()
    );
    println!(
        "           Themes/Styles/…:   {}/themes|styles|…/<name>.toml",
        paths.config_base.display()
    );
    println!();
    println!("  font     {}", paths.font_base.display());
    println!(
        "           Font sets:         {}/<name>/",
        paths.font_base.display()
    );
    println!();
    println!("  icon     {}", paths.icon_base.display());
    println!(
        "           Icon sets:         {}/<name>/",
        paths.icon_base.display()
    );
    println!();
    println!("  cursor   {}", paths.cursor_base.display());
    println!(
        "           Cursor sets:       {}/<name>/",
        paths.cursor_base.display()
    );
    println!();
    println!("Change a base with:  fsn config install-root set <base> <path>");
    println!("Move existing files: fsn config install-root migrate <base> <path>");
    Ok(())
}

// ── set ───────────────────────────────────────────────────────────────────────

/// Change a base path in the config file (does NOT move existing files).
pub async fn set(base: &str, new_path: PathBuf) -> Result<()> {
    let mut paths = InstallPaths::load();
    apply_base(&mut paths, base, new_path.clone())?;
    paths.save().map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("Updated '{base}' base to: {}", new_path.display());
    println!("Existing files were NOT moved. To move them, use:");
    println!(
        "  fsn config install-root migrate {base} {}",
        new_path.display()
    );
    Ok(())
}

// ── migrate ───────────────────────────────────────────────────────────────────

/// Change a base path AND move all installed files to the new location.
///
/// Uses `PathMigrator` (rename / copy+delete). Updates DB records after moving.
pub async fn migrate(base: &str, new_path: PathBuf) -> Result<()> {
    let old_paths = InstallPaths::load();
    let mut new_paths = old_paths.clone();
    apply_base(&mut new_paths, base, new_path.clone())?;

    // Determine which ResourceTypes are affected by this base change.
    let affected: &[ResourceType] = match base {
        "system" => InstallPaths::system_types(),
        "config" => InstallPaths::config_types(),
        "font" => InstallPaths::font_types(),
        "icon" | "cursor" => InstallPaths::icon_types(),
        _ => bail!(
            "Unknown base '{}'. Valid: system, config, font, icon, cursor",
            base
        ),
    };

    // Read installed packages to know what to move.
    let installed = get_installed_names().await;

    let migrator = PathMigrator {
        old: &old_paths,
        new: &new_paths,
    };
    let mut moved = 0usize;
    let mut skipped = 0usize;
    let mut failed = Vec::new();

    for (name, rt) in &installed {
        if !affected.contains(rt) {
            continue;
        }
        match migrator.move_resource(*rt, name) {
            Ok(outcome) if !outcome.new_path.is_empty() => {
                moved += 1;
                println!("  moved {} '{}' → {}", rt.label(), name, outcome.new_path);
            }
            Ok(_) => {
                skipped += 1;
            }
            Err(e) => {
                failed.push(format!("  {name}: {e}"));
            }
        }
    }

    if !failed.is_empty() {
        println!("\nFailed to move {} resource(s):", failed.len());
        for f in &failed {
            println!("{f}");
        }
        bail!("Migration partially failed — base path was NOT saved. Fix errors and retry.");
    }

    // Save new paths only after all moves succeeded.
    new_paths.save().map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("\nMigration complete: {moved} moved, {skipped} skipped (in-process or no files).");
    println!("Updated '{base}' base to: {}", new_path.display());
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Apply a base name change to `paths`.
fn apply_base(paths: &mut InstallPaths, base: &str, new_path: PathBuf) -> Result<()> {
    match base {
        "system" => paths.system_base = new_path,
        "config" => paths.config_base = new_path,
        "font" => paths.font_base = new_path,
        "icon" => paths.icon_base = new_path,
        "cursor" => paths.cursor_base = new_path,
        _ => bail!(
            "Unknown base '{}'. Valid values: system, config, font, icon, cursor",
            base
        ),
    }
    Ok(())
}

/// Read all installed packages and their ResourceType from the DB.
async fn get_installed_names() -> Vec<(String, ResourceType)> {
    let Some(conn) = crate::db::get_conn() else {
        return Vec::new();
    };
    let repo = InstalledPackageRepo::new(conn.inner());
    repo.list_all()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.active)
        .map(|p| (p.package_id, parse_resource_type(&p.package_type)))
        .collect()
}

/// Parse ResourceType from the stored label string.
fn parse_resource_type(label: &str) -> ResourceType {
    match label {
        "App" => ResourceType::App,
        "Container" => ResourceType::Container,
        "Bundle" => ResourceType::Bundle,
        "Widget" => ResourceType::Widget,
        "Bot" => ResourceType::Bot,
        "Bridge" => ResourceType::Bridge,
        "Task" => ResourceType::Task,
        "Language" => ResourceType::Language,
        "Color Scheme" => ResourceType::ColorScheme,
        "Style" => ResourceType::Style,
        "Font Set" => ResourceType::FontSet,
        "Cursor Set" => ResourceType::CursorSet,
        "Icon Set" => ResourceType::IconSet,
        "Button Style" => ResourceType::ButtonStyle,
        "Window Chrome" => ResourceType::WindowChrome,
        "Animation Set" => ResourceType::AnimationSet,
        "Messenger Adapter" => ResourceType::MessengerAdapter,
        _ => ResourceType::Container,
    }
}
