// `fsn store` — browse and manage the FreeSynergy module store and language packs.
//
// Uses StoreClient (fsn-store) to fetch the Node.Store catalog and display
// available modules. Install/update are delegated to `fsn deploy` once
// the module is added to the project config.

use anyhow::{Context, Result};
use fsn_node_core::store::StoreEntry;
use fsn_store::StoreClient;
use toml;

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

/// One entry from `[[languages]]` in the store's `catalog/i18n.toml`.
#[derive(serde::Deserialize)]
struct I18nCatalogEntry {
    code:           String,
    name:           String,
    #[serde(default)]
    completeness:   u8,
    #[serde(default)]
    schema_version: String,
    // file / sha256 / size are used by the updater, not displayed here
}

#[derive(serde::Deserialize)]
struct I18nCatalog {
    #[serde(default, rename = "languages")]
    languages: Vec<I18nCatalogEntry>,
}

/// Show all available language packs from the store catalog with completeness.
pub async fn i18n_status() -> Result<()> {
    let client = StoreClient::node_store();
    let raw = client
        .fetch_raw("Node/catalog/i18n.toml")
        .await
        .context("fetching i18n catalog")?;
    let catalog: I18nCatalog = toml::from_str(&raw).context("parsing i18n catalog")?;

    if catalog.languages.is_empty() {
        println!("No language packs available in the store yet.");
        return Ok(());
    }

    println!("{:<6} {:<24} {:>5}  {:<8} {}", "CODE", "LANGUAGE", "COMP%", "SCHEMA", "");
    println!("{}", "─".repeat(58));
    for e in &catalog.languages {
        let ok = e.schema_version.is_empty() || e.schema_version == BUNDLED_SCHEMA_VERSION;
        let marker = if ok { "✓" } else { "⚠ outdated" };
        let schema = if e.schema_version.is_empty() { BUNDLED_SCHEMA_VERSION } else { &e.schema_version };
        println!("{:<6} {:<24} {:>4}%  {:<8} {}", e.code, e.name, e.completeness, schema, marker);
    }
    println!("\nRequired schema: {BUNDLED_SCHEMA_VERSION}  (⚠ = needs update, run `fsn store i18n set <code>`)");
    Ok(())
}

/// Download and activate a language pack.
pub async fn i18n_set(lang: &str) -> Result<()> {
    // TODO: fetch merged lang file from store → write to ~/.config/fsn/i18n/{lang}.toml → set active
    println!("Language pack '{lang}' — download and activation not yet implemented.");
    println!("Run `fsn store i18n status` to see available packs.");
    Ok(())
}

/// Check whether the currently active language pack matches the bundled schema version.
pub async fn i18n_check() -> Result<()> {
    // TODO: read active lang from ~/.config/fsn/config.toml, compare schema_version
    println!("Schema version (bundled): {BUNDLED_SCHEMA_VERSION}");
    println!("Active language check — not yet implemented.");
    Ok(())
}
