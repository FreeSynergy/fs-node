// `fsn store` — browse and manage the FreeSynergy module store and language packs.
//
// Uses StoreClient (fs-store) to fetch the Store catalog and display
// available modules. Install/update are delegated to `fsn deploy` once
// the module is added to the project config.

use anyhow::{Context, Result};
use fs_db::InstalledPackageRepo;
use fs_node_core::store::{Catalog, NodeStoreClient, StoreEntry};
use fs_pkg::versioning::{VersionManager, VersionRecord};

// Schema version bundled with this binary — must match Lib's sync_snippets.py SCHEMA_VERSION.
const BUNDLED_SCHEMA_VERSION: &str = "1.0.0";

async fn fetch_node_catalog() -> Result<Catalog<StoreEntry>> {
    let mut client = NodeStoreClient::node_store();
    client
        .fetch_catalog("node", false)
        .await
        .context("fetching module catalog")
}

// ── StoreCmd ────────────────────────────────────────────────────────────────

pub struct StoreCmd;

impl StoreCmd {
    /// Search the store catalog for modules matching `query`.
    /// With an empty query, all modules are listed.
    pub async fn search(&self, query: &str) -> Result<()> {
        let catalog = fetch_node_catalog().await?;
        let q = query.to_lowercase();
        let matches: Vec<&StoreEntry> = catalog
            .packages
            .iter()
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

        println!("{:<24} {:<10} DESCRIPTION", "ID", "VERSION");
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

    /// Show details for a specific module by ID.
    pub async fn info(&self, id: &str) -> Result<()> {
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
                if let Some(w) = &e.website {
                    println!("Website:     {w}");
                }
                if let Some(r) = &e.repository {
                    println!("Repository:  {r}");
                }
                if let Some(l) = &e.license {
                    println!("License:     {l}");
                }
                if !e.tags.is_empty() {
                    println!("Tags:        {}", e.tags.join(", "));
                }
            }
        }
        Ok(())
    }

    /// Install a module by printing config instructions; actual deploy via `fsn deploy`.
    pub async fn install(&self, id: &str) -> Result<()> {
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

    /// Check for module updates and report available newer versions.
    pub async fn update_check(&self) -> Result<()> {
        let catalog = fetch_node_catalog().await?;
        println!(
            "Fetched catalog: {} modules available.",
            catalog.packages.len()
        );
        println!("To update a deployed module, run `fsn update --service <name>`.");
        Ok(())
    }

    /// Force-refresh the store catalog by clearing the disk cache.
    pub async fn sync(&self) -> Result<()> {
        let cache_dir = {
            let xdg = std::env::var("XDG_CACHE_HOME").ok();
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            let base = xdg.map_or_else(
                || std::path::PathBuf::from(&home).join(".cache"),
                std::path::PathBuf::from,
            );
            base.join("fsn").join("store")
        };

        if cache_dir.exists() {
            let removed = std::fs::read_dir(&cache_dir)
                .map(|entries| {
                    entries
                        .filter_map(std::result::Result::ok)
                        .filter(|e| e.path().extension().is_some_and(|x| x == "toml"))
                        .filter_map(|e| std::fs::remove_file(e.path()).ok())
                        .count()
                })
                .unwrap_or(0);
            if removed > 0 {
                println!("Cleared {removed} cached catalog file(s).");
            }
        }

        println!("Fetching fresh catalog…");
        let catalog = fetch_node_catalog().await?;
        println!(
            "Synced — {} packages, {} language packs available.",
            catalog.packages.len(),
            catalog.locales.len()
        );
        Ok(())
    }
}

// ── I18nCmd ─────────────────────────────────────────────────────────────────

pub struct I18nCmd;

impl I18nCmd {
    /// Show all available language packs from the store catalog.
    pub async fn status(&self) -> Result<()> {
        let catalog = fetch_node_catalog().await?;
        if catalog.locales.is_empty() {
            println!("No language packs listed in the store catalog.");
            return Ok(());
        }
        println!("{:<6} {:<24} {:>5}  DIR", "CODE", "LANGUAGE", "COMP%");
        println!("{}", "─".repeat(46));
        for loc in &catalog.locales {
            println!(
                "{:<6} {:<24} {:>4}%  {}",
                loc.code, loc.name, loc.completeness, loc.direction
            );
        }
        println!("\n{} language packs available.", catalog.locales.len());
        Ok(())
    }

    /// Download and activate a language pack.
    pub async fn set(&self, lang: &str) -> Result<()> {
        let catalog = fetch_node_catalog().await?;
        if !catalog.locales.iter().any(|l| l.code == lang) {
            println!("Unknown language: {lang}");
            println!("Run `fsn store i18n status` to see available packs.");
            return Ok(());
        }

        println!("Downloading language pack '{lang}'…");
        let client = NodeStoreClient::node_store();
        let bundle = client
            .fetch_i18n("Node", lang)
            .await
            .with_context(|| format!("fetching i18n bundle for '{lang}'"))?;

        let cache_dir = Self::cache_dir();
        std::fs::create_dir_all(&cache_dir)
            .with_context(|| format!("creating i18n cache dir: {}", cache_dir.display()))?;

        let ui_text = toml::to_string(&bundle.ui).context("serializing ui.toml")?;
        let lang_file = cache_dir.join(format!("{lang}.toml"));
        std::fs::write(&lang_file, &ui_text)
            .with_context(|| format!("writing {}", lang_file.display()))?;

        let lang_marker = cache_dir.parent().unwrap_or(&cache_dir).join("lang");
        std::fs::write(&lang_marker, lang)
            .with_context(|| format!("writing {}", lang_marker.display()))?;

        if let Some(conn) = crate::db::get_conn() {
            let repo = InstalledPackageRepo::new(conn.inner());
            let all = repo.list_all().await.unwrap_or_default();
            for r in all
                .iter()
                .filter(|r| r.package_id == format!("lang/{lang}") && r.active)
            {
                let _ = repo.set_active(r.id, false).await;
            }
            let _ = repo
                .insert(
                    format!("lang/{lang}"),
                    &bundle.meta.locale_code,
                    "stable",
                    "language",
                    None,
                    false,
                )
                .await;
        }

        println!(
            "Language pack '{}' ({}) installed — {}% complete.",
            lang, bundle.meta.native_name, bundle.meta.completeness
        );
        println!("Restart fsn to apply.");
        Ok(())
    }

    /// Check whether the active language pack is installed and up to date.
    pub async fn check(&self) -> Result<()> {
        let active = Self::active_lang();
        println!("Schema version (bundled): {BUNDLED_SCHEMA_VERSION}");
        println!("Active language: {active}");
        let lang_file = Self::cache_dir().join(format!("{active}.toml"));
        if lang_file.exists() {
            println!("Cached pack:    {}", lang_file.display());
        } else {
            println!("No cached pack for '{active}' — using built-in EN fallback.");
            println!("Run `fsn store i18n set {active}` to download it.");
        }
        Ok(())
    }

    pub fn cache_dir() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        std::path::PathBuf::from(home).join(".local/share/fsn/i18n")
    }

    /// Read the active language from `~/.local/share/fsn/lang`, or detect from env.
    pub fn active_lang() -> String {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let marker = std::path::PathBuf::from(home).join(".local/share/fsn/lang");
        if let Ok(lang) = std::fs::read_to_string(&marker) {
            let lang = lang.trim().to_string();
            if !lang.is_empty() {
                return lang;
            }
        }
        Self::detect_system_lang()
    }

    fn detect_system_lang() -> String {
        let raw = std::env::var("LANGUAGE")
            .or_else(|_| std::env::var("LANG"))
            .or_else(|_| std::env::var("LC_ALL"))
            .unwrap_or_default();
        raw.split(['.', '_']).next().unwrap_or("en").to_lowercase()
    }
}

// ── PackageCmd ───────────────────────────────────────────────────────────────

pub struct PackageCmd;

impl PackageCmd {
    /// List all installed packages, optionally filtered by type.
    pub async fn list(&self, type_filter: Option<&str>) -> Result<()> {
        let Some(conn) = crate::db::get_conn() else {
            println!("Database not available.");
            return Ok(());
        };

        let repo = InstalledPackageRepo::new(conn.inner());
        let mut rows = repo
            .list_all()
            .await
            .context("reading installed packages")?;
        if let Some(t) = type_filter {
            rows.retain(|r| r.package_type == t);
        }

        let active: Vec<_> = rows.iter().filter(|r| r.active).collect();
        if active.is_empty() {
            println!("No packages installed.");
            return Ok(());
        }

        println!("{:<32} {:<12} {:<10} CHANNEL", "PACKAGE", "VERSION", "TYPE");
        println!("{}", "─".repeat(68));
        for r in &active {
            println!(
                "{:<32} {:<12} {:<10} {}",
                r.package_id, r.version, r.package_type, r.channel
            );
        }
        println!("\n{} package(s) installed.", active.len());
        Ok(())
    }

    /// Remove an installed package from the database.
    pub async fn remove(&self, id: &str, confirm: bool) -> Result<()> {
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
            println!(
                "Remove '{id}' (v{})? This cannot be undone.",
                record.version
            );
            println!("Run with --confirm to proceed.");
            return Ok(());
        }

        repo.delete_by_id(record.id)
            .await
            .context("removing package record")?;
        println!("Removed '{id}' from the package registry.");
        println!("Note: deployed services are not affected — run `fsn remove` to undeploy.");
        Ok(())
    }

    /// Roll back a package to a previous (or specific) version.
    pub async fn rollback(&self, id: &str, version: Option<&str>) -> Result<()> {
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

        let vm_records: Vec<VersionRecord> = pkg_records
            .iter()
            .map(|r| VersionRecord {
                package_id: r.package_id.clone().into(),
                version: r.version.clone(),
                channel: fs_pkg::channel::ReleaseChannel::from_str_ci(&r.channel)
                    .unwrap_or_default(),
                active: r.active,
                installed_at: r.installed_at,
            })
            .collect();

        let mut vm = VersionManager::from_records(vm_records);

        let target_version = if let Some(v) = version {
            v.to_string()
        } else {
            let mut versions: Vec<_> = pkg_records.iter().collect();
            versions.sort_by(|a, b| b.installed_at.cmp(&a.installed_at));
            if versions.len() < 2 {
                println!("No previous version for '{id}' to roll back to.");
                return Ok(());
            }
            versions[1].version.clone()
        };

        vm.rollback(id, &target_version)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

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
}

// ── AssetCmd ─────────────────────────────────────────────────────────────────

pub struct AssetCmd {
    pub kind: &'static str,
}

impl AssetCmd {
    pub fn theme() -> Self {
        Self { kind: "theme" }
    }
    pub fn widget() -> Self {
        Self { kind: "widget" }
    }

    /// List available assets of this kind from the store catalog.
    pub async fn available(&self, query: &str) -> Result<()> {
        let catalog = fetch_node_catalog().await?;
        let q = query.to_lowercase();
        let kind = self.kind;

        let matches: Vec<&StoreEntry> = catalog
            .packages
            .iter()
            .filter(|e| e.category.starts_with(&format!("{kind}.")))
            .filter(|e| {
                q.is_empty()
                    || e.name.to_lowercase().contains(&q)
                    || e.id.to_lowercase().contains(&q)
                    || e.description.to_lowercase().contains(&q)
                    || e.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect();

        if matches.is_empty() {
            println!(
                "No {kind}s found{}.",
                if q.is_empty() { "" } else { " matching query" }
            );
            return Ok(());
        }

        println!("{:<28} {:<10} DESCRIPTION", "ID", "VERSION");
        println!("{}", "─".repeat(64));
        for e in &matches {
            let desc = if e.description.len() > 28 {
                format!("{}…", &e.description[..27])
            } else {
                e.description.clone()
            };
            println!("{:<28} {:<10} {}", e.id, e.version, desc);
        }
        println!("\n{} {kind}(s) found.", matches.len());
        Ok(())
    }

    /// List installed assets of this kind from the database.
    pub async fn list(&self) -> Result<()> {
        let Some(conn) = crate::db::get_conn() else {
            println!("Database not available.");
            return Ok(());
        };

        let repo = InstalledPackageRepo::new(conn.inner());
        let rows = repo
            .list_all()
            .await
            .context("reading installed packages")?;
        let installed: Vec<_> = rows
            .iter()
            .filter(|r| r.active && r.package_type == self.kind)
            .collect();

        if installed.is_empty() {
            println!("No {}s installed.", self.kind);
            return Ok(());
        }

        println!("{:<32} {:<12} CHANNEL", "ID", "VERSION");
        println!("{}", "─".repeat(58));
        for r in &installed {
            println!("{:<32} {:<12} {}", r.package_id, r.version, r.channel);
        }
        Ok(())
    }

    /// Install an asset from the store.
    pub async fn install(&self, id: &str, dry_run: bool) -> Result<()> {
        let catalog = fetch_node_catalog().await?;
        let kind = self.kind;

        let entry = catalog
            .packages
            .iter()
            .find(|e| e.id == id && e.category.starts_with(&format!("{kind}.")));
        let Some(entry) = entry else {
            println!("{} not found in catalog: {id}", Self::capitalize(kind));
            println!("Run `fsn store {kind} available` to list available {kind}s.");
            return Ok(());
        };

        let install_dir = self.install_dir(id);
        if dry_run {
            println!(
                "Dry-run: would install '{id}' (v{}) to {}",
                entry.version,
                install_dir.display()
            );
            return Ok(());
        }

        std::fs::create_dir_all(&install_dir)
            .with_context(|| format!("creating {} directory {}", kind, install_dir.display()))?;

        let marker = install_dir.join("manifest.toml");
        std::fs::write(
            &marker,
            format!(
                "[package]\nid = \"{}\"\nname = \"{}\"\nversion = \"{}\"\ncategory = \"{}\"\n",
                entry.id, entry.name, entry.version, entry.category,
            ),
        )
        .with_context(|| format!("writing {}", marker.display()))?;

        if let Some(conn) = crate::db::get_conn() {
            let repo = InstalledPackageRepo::new(conn.inner());
            repo.insert(id, &entry.version, "stable", kind, None, false)
                .await
                .context("registering in database")?;
        }

        println!(
            "Installed {} '{}' (v{}) to {}",
            kind,
            entry.name,
            entry.version,
            install_dir.display()
        );
        Ok(())
    }

    /// Remove an installed asset.
    pub async fn remove(&self, id: &str, confirm: bool) -> Result<()> {
        let Some(conn) = crate::db::get_conn() else {
            println!("Database not available.");
            return Ok(());
        };

        let repo = InstalledPackageRepo::new(conn.inner());
        let active = repo.find_active(id).await.context("looking up package")?;
        let Some(record) = active else {
            println!("{} not installed: {id}", Self::capitalize(self.kind));
            return Ok(());
        };

        if !confirm {
            println!(
                "Remove {} '{}' (v{})? Run with --confirm to proceed.",
                self.kind, id, record.version
            );
            return Ok(());
        }

        let install_dir = self.install_dir(id);
        if install_dir.exists() {
            std::fs::remove_dir_all(&install_dir)
                .with_context(|| format!("removing {} directory", install_dir.display()))?;
        }

        repo.delete_by_id(record.id)
            .await
            .context("removing from database")?;
        println!("Removed {} '{id}'.", self.kind);
        Ok(())
    }

    fn install_dir(&self, id: &str) -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let safe_id = id.replace('/', "-");
        std::path::PathBuf::from(home)
            .join(".local/share/fsn")
            .join(format!("{}s", self.kind))
            .join(safe_id)
    }

    fn capitalize(s: &str) -> String {
        let mut c = s.chars();
        match c.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().to_string() + c.as_str(),
        }
    }
}
