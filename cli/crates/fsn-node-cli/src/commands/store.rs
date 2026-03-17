// `fsn store` — browse and manage the FreeSynergy module store and language packs.
//
// Uses StoreClient (fsn-store) to fetch the Node.Store catalog and display
// available modules. Install/update are delegated to `fsn deploy` once
// the module is added to the project config.

use anyhow::{Context, Result};
use fsn_node_core::store::StoreEntry;
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
