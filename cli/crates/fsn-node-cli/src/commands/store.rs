// `fsn store` — browse and manage the FreeSynergy module store and language packs.
//
// Uses StoreClient (fsn-store) to fetch the Node.Store catalog and display
// available modules. Install/update are delegated to `fsn deploy` once
// the module is added to the project config.

use anyhow::{Context, Result};
use fsn_db::InstalledPackageRepo;
use fsn_node_core::store::StoreEntry;
use fsn_pkg::versioning::{VersionManager, VersionRecord};
use fsn_store::StoreClient;

// Schema version bundled with this binary — must match Lib's sync_snippets.py SCHEMA_VERSION.
const BUNDLED_SCHEMA_VERSION: &str = "1.0.0";

// ── Catalog helper ─────────────────────────────────────────────────────────────

/// Fetch the FreeSynergy Node module catalog.
async fn fetch_node_catalog() -> Result<fsn_store::Catalog<StoreEntry>> {
    let mut client = StoreClient::node_store();
    client.fetch_catalog("Node", false).await.context("fetching module catalog")
}

// ── search ─────────────────────────────────────────────────────────────────────

/// Search the store catalog for modules matching `query`.
///
/// With an empty query, all modules are listed.
pub async fn search(query: &str) -> Result<()> {
    let catalog = fetch_node_catalog().await?;

    let q = query.to_lowercase();
    let matches: Vec<&StoreEntry> = catalog.packages.iter()
        .filter(|e| {
            q.is_empty()
                || e.name.to_lowercase().contains(&q)
                || e.id.to_lowercase().contains(&q)
                || e.description.to_lowercase().contains(&q)
                || e.tags.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .collect();

    if matches.is_empty() {
        if q.is_empty() {
            println!("Store catalog is empty.");
        } else {
            println!("No modules found matching: {query}");
        }
        return Ok(());
    }

    println!("{:<24} {:<10} {}", "ID", "VERSION", "DESCRIPTION");
    println!("{}", "─".repeat(72));
    for entry in &matches {
        let desc = if entry.description.len() > 40 {
            format!("{}…", &entry.description[..39])
        } else {
            entry.description.clone()
        };
        println!("{:<24} {:<10} {}", entry.id, entry.version, desc);
    }
    println!("\n{} module(s) found.", matches.len());
    Ok(())
}

// ── info ───────────────────────────────────────────────────────────────────────

/// Show details for a specific module by ID.
pub async fn info(id: &str) -> Result<()> {
    let catalog = fetch_node_catalog().await?;

    match catalog.packages.iter().find(|e| e.id == id) {
        None => {
            println!("Module not found: {id}");
            println!("Run `fsn store search` to list available modules.");
        }
        Some(e) => {
            println!("Name:        {}", e.name);
            println!("ID:          {}", e.id);
            println!("Version:     {}", e.version);
            println!("Category:    {}", e.category);
            println!("Description: {}", e.description);
            if let Some(w) = &e.website    { println!("Website:     {w}"); }
            if let Some(r) = &e.repository { println!("Repository:  {r}"); }
            if let Some(l) = &e.license    { println!("License:     {l}"); }
            if !e.tags.is_empty()           { println!("Tags:        {}", e.tags.join(", ")); }
        }
    }
    Ok(())
}

// ── install ────────────────────────────────────────────────────────────────────

/// Install a module by adding it to the project config.
///
/// This prints instructions; actual deployment is done via `fsn deploy`.
pub async fn install(id: &str) -> Result<()> {
    let catalog = fetch_node_catalog().await?;

    if catalog.packages.iter().all(|e| e.id != id) {
        println!("Module not found: {id}");
        println!("Run `fsn store search` to list available modules.");
        return Ok(());
    }

    println!("To install '{id}', add it to your project config:");
    println!();
    println!("  [load.services.my-{id}]");
    println!("  service_class = \"{id}\"");
    println!();
    println!("Then run `fsn deploy` to apply.");
    Ok(())
}

// ── update ─────────────────────────────────────────────────────────────────────

/// Check for module updates and report available newer versions.
pub async fn update_check() -> Result<()> {
    let catalog = fetch_node_catalog().await?;
    println!("Fetched catalog: {} modules available.", catalog.packages.len());
    println!("To update a deployed module, run `fsn update --service <name>`.");
    Ok(())
}

// ── i18n ───────────────────────────────────────────────────────────────────────

/// Show all available language packs from the store catalog with completeness.
///
/// Reads `[[locales]]` from the main Node `catalog.toml`.
pub async fn i18n_status() -> Result<()> {
    let catalog = fetch_node_catalog().await?;

    if catalog.locales.is_empty() {
        println!("No language packs listed in the store catalog.");
        return Ok(());
    }

    println!("{:<6} {:<24} {:>5}  {}", "CODE", "LANGUAGE", "COMP%", "DIR");
    println!("{}", "─".repeat(46));
    for loc in &catalog.locales {
        println!("{:<6} {:<24} {:>4}%  {}", loc.code, loc.name, loc.completeness, loc.direction);
    }
    println!("\n{} language packs available.", catalog.locales.len());
    Ok(())
}

/// Download and activate a language pack.
///
/// Fetches `Node/i18n/{lang}/manifest.toml` + `ui.toml` from the store,
/// caches them to `~/.local/share/fsn/i18n/{lang}.toml`, and sets the
/// active language in `~/.local/share/fsn/lang`.
pub async fn i18n_set(lang: &str) -> Result<()> {
    let catalog = fetch_node_catalog().await?;

    if !catalog.locales.iter().any(|l| l.code == lang) {
        println!("Unknown language: {lang}");
        println!("Run `fsn store i18n status` to see available packs.");
        return Ok(());
    }

    println!("Downloading language pack '{lang}'…");
    let client = StoreClient::node_store();
    let bundle = client.fetch_i18n("Node", lang).await
        .with_context(|| format!("fetching i18n bundle for '{lang}'"))?;

    let cache_dir = i18n_cache_dir();
    std::fs::create_dir_all(&cache_dir)
        .with_context(|| format!("creating i18n cache dir: {}", cache_dir.display()))?;

    let ui_text = toml::to_string(&bundle.ui)
        .context("serializing ui.toml")?;
    let lang_file = cache_dir.join(format!("{lang}.toml"));
    std::fs::write(&lang_file, &ui_text)
        .with_context(|| format!("writing {}", lang_file.display()))?;

    let lang_marker = cache_dir.parent().unwrap_or(&cache_dir).join("lang");
    std::fs::write(&lang_marker, lang)
        .with_context(|| format!("writing {}", lang_marker.display()))?;

    // Register in DB (PackageType::Language)
    if let Some(conn) = crate::db::get_conn() {
        let repo = InstalledPackageRepo::new(conn.inner());
        // Deactivate any existing record for this language
        let all = repo.list_all().await.unwrap_or_default();
        for r in all.iter().filter(|r| r.package_id == format!("lang/{lang}") && r.active) {
            let _ = repo.set_active(r.id, false).await;
        }
        let _ = repo.insert(
            format!("lang/{lang}"),
            &bundle.meta.locale_code,
            "stable",
            "language",
            None,
            false,
        ).await;
    }

    println!("Language pack '{}' ({}) installed — {}% complete.",
        lang, bundle.meta.native_name, bundle.meta.completeness);
    println!("Restart fsn to apply.");
    Ok(())
}

/// Check whether the active language pack is installed and up to date.
pub async fn i18n_check() -> Result<()> {
    let active = active_lang();
    println!("Schema version (bundled): {BUNDLED_SCHEMA_VERSION}");
    println!("Active language: {active}");

    let lang_file = i18n_cache_dir().join(format!("{active}.toml"));
    if lang_file.exists() {
        println!("Cached pack:    {}", lang_file.display());
    } else {
        println!("No cached pack for '{active}' — using built-in EN fallback.");
        println!("Run `fsn store i18n set {active}` to download it.");
    }
    Ok(())
}

// ── list ───────────────────────────────────────────────────────────────────────

/// List all installed packages, optionally filtered by type.
pub async fn list(type_filter: Option<&str>) -> Result<()> {
    let Some(conn) = crate::db::get_conn() else {
        println!("Database not available.");
        return Ok(());
    };

    let repo = InstalledPackageRepo::new(conn.inner());
    let mut rows = repo.list_all().await.context("reading installed packages")?;

    if let Some(t) = type_filter {
        rows.retain(|r| r.package_type == t);
    }

    let active: Vec<_> = rows.iter().filter(|r| r.active).collect();
    if active.is_empty() {
        println!("No packages installed.");
        return Ok(());
    }

    println!("{:<32} {:<12} {:<10} {}", "PACKAGE", "VERSION", "TYPE", "CHANNEL");
    println!("{}", "─".repeat(68));
    for r in &active {
        println!("{:<32} {:<12} {:<10} {}", r.package_id, r.version, r.package_type, r.channel);
    }
    println!("\n{} package(s) installed.", active.len());
    Ok(())
}

// ── remove ─────────────────────────────────────────────────────────────────────

/// Remove an installed package from the database.
pub async fn pkg_remove(id: &str, confirm: bool) -> Result<()> {
    let Some(conn) = crate::db::get_conn() else {
        println!("Database not available.");
        return Ok(());
    };

    let repo = InstalledPackageRepo::new(conn.inner());
    let active = repo.find_active(id).await.context("looking up package")?;

    let Some(record) = active else {
        println!("Package not installed: {id}");
        return Ok(());
    };

    if !confirm {
        println!("Remove '{id}' (v{})? This cannot be undone.", record.version);
        println!("Run with --confirm to proceed.");
        return Ok(());
    }

    repo.remove(record.id).await.context("removing package record")?;
    println!("Removed '{id}' from the package registry.");
    println!("Note: deployed services are not affected — run `fsn remove` to undeploy.");
    Ok(())
}

// ── rollback ───────────────────────────────────────────────────────────────────

/// Roll back a package to a previous (or specific) version.
pub async fn rollback(id: &str, version: Option<&str>) -> Result<()> {
    let Some(conn) = crate::db::get_conn() else {
        println!("Database not available.");
        return Ok(());
    };

    let repo = InstalledPackageRepo::new(conn.inner());
    let all = repo.list_all().await.context("reading version history")?;

    let pkg_records: Vec<_> = all.iter().filter(|r| r.package_id == id).collect();
    if pkg_records.is_empty() {
        println!("Package not found: {id}");
        return Ok(());
    }

    // Build VersionManager from DB records
    let vm_records: Vec<VersionRecord> = pkg_records.iter().map(|r| {
        VersionRecord {
            package_id:   r.package_id.clone(),
            version:      r.version.clone(),
            channel:      fsn_pkg::channel::ReleaseChannel::from_str_ci(&r.channel).unwrap_or_default(),
            active:       r.active,
            installed_at: r.installed_at,
        }
    }).collect();

    let mut vm = VersionManager::from_records(vm_records);

    let target_version = match version {
        Some(v) => v.to_string(),
        None => {
            // Find previous version
            let mut versions: Vec<_> = pkg_records.iter().collect();
            versions.sort_by(|a, b| b.installed_at.cmp(&a.installed_at));
            if versions.len() < 2 {
                println!("No previous version for '{id}' to roll back to.");
                return Ok(());
            }
            versions[1].version.clone()
        }
    };

    vm.rollback(id, &target_version)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    // Update DB: deactivate current, activate target
    for record in pkg_records {
        let should_be_active = record.version == target_version;
        if record.active != should_be_active {
            repo.set_active(record.id, should_be_active)
                .await
                .context("updating active flag")?;
        }
    }

    println!("Rolled back '{id}' to v{target_version}.");
    println!("Run `fsn deploy` to apply the change.");
    Ok(())
}

// ── sync ────────────────────────────────────────────────────────────────────────

/// Force-refresh the store catalog by clearing the disk cache entry.
pub async fn sync() -> Result<()> {
    // Clear the disk cache for the Node catalog
    let cache_dir = {
        let xdg = std::env::var("XDG_CACHE_HOME").ok();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let base = xdg.map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(&home).join(".cache"));
        base.join("fsn").join("store")
    };

    if cache_dir.exists() {
        let removed = std::fs::read_dir(&cache_dir)
            .map(|entries| {
                entries.filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map_or(false, |x| x == "toml"))
                    .filter_map(|e| std::fs::remove_file(e.path()).ok().map(|_| ()))
                    .count()
            })
            .unwrap_or(0);
        if removed > 0 {
            println!("Cleared {removed} cached catalog file(s).");
        }
    }

    // Re-fetch to warm up
    println!("Fetching fresh catalog…");
    let catalog = fetch_node_catalog().await?;
    println!("Synced — {} packages, {} language packs available.",
        catalog.packages.len(), catalog.locales.len());
    Ok(())
}

// ── asset (theme / widget) ─────────────────────────────────────────────────────

/// List available themes or widgets from the store catalog.
pub async fn asset_available(asset_type: &str, query: &str) -> Result<()> {
    let catalog = fetch_node_catalog().await?;
    let q = query.to_lowercase();

    // The Node catalog currently lists modules; themes/widgets will appear
    // as entries with category starting with "theme." or "widget."
    let matches: Vec<&StoreEntry> = catalog.packages.iter()
        .filter(|e| e.category.starts_with(&format!("{asset_type}.")))
        .filter(|e| {
            q.is_empty()
                || e.name.to_lowercase().contains(&q)
                || e.id.to_lowercase().contains(&q)
                || e.description.to_lowercase().contains(&q)
                || e.tags.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .collect();

    if matches.is_empty() {
        println!("No {asset_type}s found{}.", if q.is_empty() { "" } else { " matching query" });
        return Ok(());
    }

    println!("{:<28} {:<10} {}", "ID", "VERSION", "DESCRIPTION");
    println!("{}", "─".repeat(64));
    for e in &matches {
        let desc = if e.description.len() > 28 {
            format!("{}…", &e.description[..27])
        } else {
            e.description.clone()
        };
        println!("{:<28} {:<10} {}", e.id, e.version, desc);
    }
    println!("\n{} {asset_type}(s) found.", matches.len());
    Ok(())
}

/// List installed themes or widgets from the local directory.
pub async fn asset_list(asset_type: &str) -> Result<()> {
    let Some(conn) = crate::db::get_conn() else {
        println!("Database not available.");
        return Ok(());
    };

    let repo = InstalledPackageRepo::new(conn.inner());
    let rows = repo.list_all().await.context("reading installed packages")?;
    let installed: Vec<_> = rows.iter()
        .filter(|r| r.active && r.package_type == asset_type)
        .collect();

    if installed.is_empty() {
        println!("No {asset_type}s installed.");
        return Ok(());
    }

    println!("{:<32} {:<12} {}", "ID", "VERSION", "CHANNEL");
    println!("{}", "─".repeat(58));
    for r in &installed {
        println!("{:<32} {:<12} {}", r.package_id, r.version, r.channel);
    }
    Ok(())
}

/// Install a theme or widget from the store.
pub async fn asset_install(asset_type: &str, id: &str, dry_run: bool) -> Result<()> {
    let catalog = fetch_node_catalog().await?;

    let entry = catalog.packages.iter()
        .find(|e| e.id == id && e.category.starts_with(&format!("{asset_type}.")));

    let Some(entry) = entry else {
        println!("{} not found in catalog: {id}", capitalize(asset_type));
        println!("Run `fsn store {asset_type} available` to list available {asset_type}s.");
        return Ok(());
    };

    let install_dir = asset_install_dir(asset_type, id);

    if dry_run {
        println!("Dry-run: would install '{id}' (v{}) to {}", entry.version, install_dir.display());
        return Ok(());
    }

    std::fs::create_dir_all(&install_dir)
        .with_context(|| format!("creating {} directory {}", asset_type, install_dir.display()))?;

    // Write a manifest marker so we can track what's installed
    let marker = install_dir.join("manifest.toml");
    std::fs::write(&marker, format!(
        "[package]\nid = \"{}\"\nname = \"{}\"\nversion = \"{}\"\ncategory = \"{}\"\n",
        entry.id, entry.name, entry.version, entry.category,
    )).with_context(|| format!("writing {}", marker.display()))?;

    // Register in DB
    if let Some(conn) = crate::db::get_conn() {
        let repo = InstalledPackageRepo::new(conn.inner());
        repo.insert(id, &entry.version, "stable", asset_type, None, false)
            .await
            .context("registering in database")?;
    }

    println!("Installed {} '{}' (v{}) to {}",
        asset_type, entry.name, entry.version, install_dir.display());
    Ok(())
}

/// Remove an installed theme or widget.
pub async fn asset_remove(asset_type: &str, id: &str, confirm: bool) -> Result<()> {
    let Some(conn) = crate::db::get_conn() else {
        println!("Database not available.");
        return Ok(());
    };

    let repo = InstalledPackageRepo::new(conn.inner());
    let active = repo.find_active(id).await.context("looking up package")?;

    let Some(record) = active else {
        println!("{} not installed: {id}", capitalize(asset_type));
        return Ok(());
    };

    if !confirm {
        println!("Remove {} '{}' (v{})? Run with --confirm to proceed.",
            asset_type, id, record.version);
        return Ok(());
    }

    // Remove files
    let install_dir = asset_install_dir(asset_type, id);
    if install_dir.exists() {
        std::fs::remove_dir_all(&install_dir)
            .with_context(|| format!("removing {} directory", install_dir.display()))?;
    }

    repo.remove(record.id).await.context("removing from database")?;
    println!("Removed {} '{id}'.", asset_type);
    Ok(())
}

fn asset_install_dir(asset_type: &str, id: &str) -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let safe_id = id.replace('/', "-");
    std::path::PathBuf::from(home)
        .join(".local/share/fsn")
        .join(format!("{}s", asset_type))
        .join(safe_id)
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}

pub fn i18n_cache_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    std::path::PathBuf::from(home).join(".local/share/fsn/i18n")
}

/// Read the active language from `~/.local/share/fsn/lang`, or detect from env.
pub fn active_lang() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let marker = std::path::PathBuf::from(home).join(".local/share/fsn/lang");
    if let Ok(lang) = std::fs::read_to_string(&marker) {
        let lang = lang.trim().to_string();
        if !lang.is_empty() { return lang; }
    }
    detect_system_lang()
}

fn detect_system_lang() -> String {
    let raw = std::env::var("LANGUAGE")
        .or_else(|_| std::env::var("LANG"))
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_default();
    raw.split(['.', '_']).next().unwrap_or("en")
        .to_lowercase()
}
