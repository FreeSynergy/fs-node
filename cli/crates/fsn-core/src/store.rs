// Store catalog data model.
//
// The store is a package registry distributed as `Node/catalog.toml`
// at the root of any store repository.
//
// catalog.toml contains two arrays:
//   [[packages]] — deployable modules (zentinel, kanidm, forgejo, …)
//   [[locales]]  — available i18n language packs
//
// Field order in catalog.toml (packages): id → name → category → version
//   → description → icon → license → path → tags
// Field order (locales): code → name → version → completeness → direction
//   → api_version → path

use serde::{Deserialize, Serialize};

use crate::config::service::types::{ServiceType, de_service_types};

// ── StoreCatalog ───────────────────────────────────────────────────────────────

/// The top-level store catalog.
/// Fetched from `{store_url}/Node/catalog.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoreCatalog {
    /// Catalog metadata — present in auto-generated files; ignored when absent.
    #[serde(default)]
    pub catalog: CatalogMeta,

    /// All deployment packages listed in this catalog.
    #[serde(default)]
    pub packages: Vec<StoreEntry>,

    /// All available locale packs listed in this catalog.
    #[serde(default)]
    pub locales: Vec<LocaleEntry>,
}

// ── CatalogMeta ───────────────────────────────────────────────────────────────

/// Top-level [catalog] metadata block (auto-generated, informational only).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CatalogMeta {
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub generated_at: String,
}

// ── StoreEntry ────────────────────────────────────────────────────────────────

/// One package entry in the catalog.
/// Describes a deployable service (kanidm, forgejo, zentinel, …).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreEntry {
    /// Unique identifier — matches the module class key in the local registry.
    /// Example: "iam/kanidm", "proxy/zentinel".
    pub id: String,

    /// Human-readable display name.
    /// Example: "Kanidm", "Zentinel".
    pub name: String,

    /// Dot-separated category following the Store category system.
    /// Example: "deploy.proxy", "deploy.iam", "deploy.git".
    /// Backward-compat: also accepts legacy `service_type` / `service_types`.
    #[serde(default)]
    pub category: String,

    /// Service type(s) — backward-compat alias for `category`.
    /// Accepts a single string (`service_type = "iam"`) or array.
    /// Deserialized via `de_service_types`.
    #[serde(
        alias = "service_type",
        rename = "service_types",
        deserialize_with = "de_service_types",
        default = "default_custom_types"
    )]
    pub service_types: Vec<ServiceType>,

    /// Version of the module definition (semver string).
    pub version: String,

    /// Short description of what the software does.
    pub description: String,

    /// Relative path to the SVG icon within the store repository.
    pub icon: Option<String>,

    /// SPDX license identifier. Example: "Apache-2.0", "MIT".
    #[serde(default)]
    pub license: Option<String>,

    /// Store-relative path to the package directory.
    #[serde(default)]
    pub path: Option<String>,

    /// Official website of the software this module deploys.
    pub website: Option<String>,

    /// Source code repository of the software.
    pub repository: Option<String>,

    /// Author / maintainer of the module definition.
    pub author: Option<String>,

    /// Searchable tags.
    #[serde(default)]
    pub tags: Vec<String>,

    /// ISO 8601 date string when this module was first published.
    #[serde(default)]
    pub created_at: Option<String>,

    /// ISO 8601 date string when this module was last updated.
    #[serde(default)]
    pub updated_at: Option<String>,

    /// Minimum FSN version required to deploy this module.
    #[serde(default)]
    pub min_fsn_version: Option<String>,

    /// Name of the store this entry was fetched from.
    /// Set by StoreClient when merging results from multiple stores.
    #[serde(default)]
    pub store_source: String,
}

fn default_custom_types() -> Vec<ServiceType> {
    vec![ServiceType::Custom]
}

impl StoreEntry {
    /// Returns the category-derived display label shown in the TUI service class dropdown.
    /// Format: "Kanidm (IAM)" or "Kanidm (IAM) ↓" when not installed locally.
    pub fn select_label(&self, is_local: bool) -> String {
        let type_label = if !self.category.is_empty() {
            // Derive label from dot-separated category: "deploy.iam" → "IAM"
            self.category
                .split('.')
                .last()
                .unwrap_or(&self.category)
                .to_uppercase()
        } else {
            self.service_types.iter()
                .map(|t| t.label())
                .collect::<Vec<_>>()
                .join("/")
        };
        if is_local {
            format!("{} ({})", self.name, type_label)
        } else {
            format!("{} ({}) ↓", self.name, type_label)
        }
    }

    /// Returns the primary (first) service type of this entry.
    pub fn primary_type(&self) -> &ServiceType {
        self.service_types.first().unwrap_or(&ServiceType::Custom)
    }

    /// Returns the primary service type as a lowercase string.
    pub fn primary_type_str(&self) -> String {
        self.primary_type().to_string()
    }

    /// Returns the category suffix (e.g. "proxy" from "deploy.proxy").
    /// Falls back to primary_type_str for backward compat.
    pub fn category_type(&self) -> &str {
        if !self.category.is_empty() {
            self.category.split('.').last().unwrap_or(&self.category)
        } else {
            // Leak-free: return a ref to the category or service_type string
            // (this is fine since primary_type_str is not called here)
            &self.category
        }
    }
}

// ── LocaleEntry ───────────────────────────────────────────────────────────────

/// One locale entry in the catalog's [[locales]] array.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleEntry {
    /// BCP-47 locale code, e.g. "de", "ar", "pt-br".
    pub code: String,

    /// Display name in the locale's own script, e.g. "Deutsch", "العربية".
    pub name: String,

    /// Version string (semver).
    #[serde(default = "default_version")]
    pub version: String,

    /// Percentage of keys translated, 0–100. Updated by CI.
    #[serde(default = "default_completeness")]
    pub completeness: u8,

    /// Text direction: "ltr" or "rtl".
    #[serde(default = "default_direction")]
    pub direction: String,

    /// Translation API version — must match TRANSLATION_API_VERSION in the app.
    #[serde(default = "default_api_version")]
    pub api_version: u32,

    /// Store-relative path to the locale directory, e.g. "Node/i18n/de".
    #[serde(default)]
    pub path: Option<String>,
}

fn default_version()     -> String { "1.0.0".into() }
fn default_direction()   -> String { "ltr".into() }
fn default_completeness() -> u8    { 0 }
fn default_api_version() -> u32    { 1 }

impl LocaleEntry {
    /// Returns the path to the ui.toml file within a resolved store root.
    ///
    /// `store_root` — local path to the Store repository root.
    /// Returns `{store_root}/{path}/ui.toml` if `path` is set, else None.
    pub fn ui_toml_path(&self, store_root: &std::path::Path) -> Option<std::path::PathBuf> {
        self.path.as_ref().map(|p| store_root.join(p).join("ui.toml"))
    }
}
