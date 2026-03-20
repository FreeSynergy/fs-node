// validate-store — validate all packages listed in a store catalog.
//
// Reads `{store_dir}/{namespace}/catalog.toml`, checks every package entry:
//   - TOML manifest exists at declared path
//   - Required fields present (id, name, version, description, tags)
//   - Icon file exists (for local icons)
//   - For app packages: at least one distribution URL declared
//
// Reports ✅ / ⚠️ / ❌ per entry and exits with code 1 if any are broken.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

// ── Catalog types (minimal deserialization) ───────────────────────────────────

#[derive(Debug, Deserialize)]
struct CatalogFile {
    #[serde(default)]
    packages: Vec<PackageEntry>,
}

#[derive(Debug, Deserialize)]
struct PackageEntry {
    id:          Option<String>,
    name:        Option<String>,
    version:     Option<String>,
    description: Option<String>,
    #[serde(default)]
    tags:        Vec<String>,
    icon:        Option<String>,
    path:        Option<String>,
    #[serde(rename = "type")]
    pkg_type:    Option<String>,
    repo:        Option<String>,
    distribution: Option<toml::Value>,
}

// ── Validation ────────────────────────────────────────────────────────────────

struct Issues {
    warnings: Vec<String>,
    errors:   Vec<String>,
}

impl Issues {
    fn new() -> Self { Self { warnings: Vec::new(), errors: Vec::new() } }
    fn warn(&mut self, msg: impl Into<String>) { self.warnings.push(msg.into()); }
    fn error(&mut self, msg: impl Into<String>) { self.errors.push(msg.into()); }
}

fn validate_entry(entry: &PackageEntry, store_dir: &Path, issues: &mut Issues) {
    // Required fields
    if entry.id.as_deref().unwrap_or("").is_empty()          { issues.error("missing id"); }
    if entry.name.as_deref().unwrap_or("").is_empty()        { issues.error("missing name"); }
    if entry.version.as_deref().unwrap_or("").is_empty()     { issues.error("missing version"); }
    if entry.description.as_deref().unwrap_or("").is_empty() { issues.warn("missing description"); }
    if entry.tags.is_empty()                                  { issues.warn("no tags — package will be hard to find"); }

    // Icon
    match &entry.icon {
        None => issues.warn("no icon declared — generic icon will be shown"),
        Some(icon) if icon.starts_with("shared/") || icon.starts_with("node/") => {
            let icon_path = store_dir.join(icon);
            if !icon_path.exists() {
                issues.error(format!("icon file not found: {}", icon_path.display()));
            }
        }
        Some(icon) if icon.starts_with("http://") || icon.starts_with("https://") => {
            // External URL — not verified here
        }
        Some(_) => {
            issues.warn("icon path should start with 'shared/icons/' for local files");
        }
    }

    // Package-type specific checks
    let pkg_type = entry.pkg_type.as_deref().unwrap_or("");
    match pkg_type {
        "app" => {
            if entry.repo.is_none() {
                issues.error("app package must have a 'repo' URL");
            }
            if entry.distribution.is_none() {
                issues.warn("no [distribution] URLs — binary download will not be possible");
            }
        }
        "container" | "" => {
            if let Some(path) = &entry.path {
                let manifest_path = store_dir.join(path).join("manifest.toml");
                let legacy_toml = store_dir.join(path);
                if !manifest_path.exists() && !legacy_toml.with_extension("toml").exists() {
                    let category = path.split('/').next_back().unwrap_or("");
                    let legacy = store_dir.join(path).join(format!("{category}.toml"));
                    if !legacy.exists() {
                        issues.warn(format!("no manifest.toml found at {}", manifest_path.display()));
                    }
                }
            } else if pkg_type != "app" {
                issues.warn("no 'path' declared — store can't locate the package files");
            }
        }
        _ => {}
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run(store_dir: &Path, namespace: &str) -> Result<()> {
    let catalog_path = store_dir.join(namespace).join("catalog.toml");
    if !catalog_path.exists() {
        anyhow::bail!("catalog not found at {}", catalog_path.display());
    }

    let raw = std::fs::read_to_string(&catalog_path)
        .with_context(|| format!("read {}", catalog_path.display()))?;
    let catalog: CatalogFile = toml::from_str(&raw)
        .with_context(|| format!("parse {}", catalog_path.display()))?;

    let total = catalog.packages.len();
    if total == 0 {
        println!("⚠️  No packages found in {}", catalog_path.display());
        return Ok(());
    }

    println!("Validating {total} packages in {namespace}/catalog.toml\n");

    let mut ok = 0usize;
    let mut warn = 0usize;
    let mut broken = 0usize;

    for entry in &catalog.packages {
        let id = entry.id.as_deref().unwrap_or("<no id>");
        let mut issues = Issues::new();
        validate_entry(entry, store_dir, &mut issues);

        if issues.errors.is_empty() && issues.warnings.is_empty() {
            println!("✅ {id}");
            ok += 1;
        } else if issues.errors.is_empty() {
            println!("⚠️  {id}");
            for w in &issues.warnings { println!("     ⚠  {w}"); }
            warn += 1;
        } else {
            println!("❌ {id}");
            for e in &issues.errors   { println!("     ✗  {e}"); }
            for w in &issues.warnings { println!("     ⚠  {w}"); }
            broken += 1;
        }
    }

    println!("\n── Summary ─────────────────────────────────────────");
    println!("✅ {ok} ok  ⚠️  {warn} warnings  ❌ {broken} broken  (total: {total})");

    if broken > 0 {
        anyhow::bail!("{broken} package(s) are broken — fix errors above");
    }

    Ok(())
}
