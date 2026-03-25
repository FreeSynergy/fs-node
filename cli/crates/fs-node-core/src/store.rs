// Store catalog data model — FSN-specific types for the package catalog.
//
// Architecture:
//   fs-node-core — Manifest trait, CatalogMeta, LocaleEntry, StoreEntry,
//                  StoreCatalog, NodeStoreClient (HTTP catalog/i18n fetch)
//   fs-deploy    — StoreClient (FSN multi-store aggregator, git sync)
//
// StoreCatalog is FSN's catalog format. StoreEntry is FSN's package entry.
// The `alias = "modules"` on packages is FSN legacy backward-compat.

use serde::{Deserialize, Serialize};

use crate::config::service::types::{de_service_types, ServiceType};

// ── Manifest ──────────────────────────────────────────────────────────────────

/// Minimal trait for store catalog entries — allows generic catalog filtering.
pub trait Manifest {
    fn id(&self) -> &str;
    fn version(&self) -> &str;
    fn category(&self) -> &str;
    fn name(&self) -> &str;
}

// ── CatalogMeta ───────────────────────────────────────────────────────────────

/// Auto-generated header block in `catalog.toml` — informational only.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CatalogMeta {
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
}

// ── LocaleEntry ───────────────────────────────────────────────────────────────

/// One locale pack listed in the store catalog.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocaleEntry {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub completeness: u8,
    #[serde(default)]
    pub direction: String,
}

// ── Catalog<T> ────────────────────────────────────────────────────────────────

/// Generic catalog returned by [`NodeStoreClient::fetch_catalog`].
///
/// `packages` contains entries deserialized as `T`.
/// `locales` lists all available locale packs.
/// `catalog` holds auto-generated metadata (informational only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog<T> {
    #[serde(default)]
    pub catalog: CatalogMeta,
    #[serde(default = "Vec::new", alias = "modules")]
    pub packages: Vec<T>,
    #[serde(default = "Vec::new")]
    pub locales: Vec<LocaleEntry>,
}

impl<T> Default for Catalog<T> {
    fn default() -> Self {
        Self {
            catalog: CatalogMeta::default(),
            packages: Vec::new(),
            locales: Vec::new(),
        }
    }
}

// ── StoreCatalog ───────────────────────────────────────────────────────────────

/// FSN's top-level store catalog (concrete alias for `Catalog<StoreEntry>`).
///
/// Deserializes `catalog.toml` fetched from `{store_url}/node/catalog.toml`.
pub type StoreCatalog = Catalog<StoreEntry>;

// ── StoreEntry ────────────────────────────────────────────────────────────────

/// One package entry in the FSN catalog.
/// Describes a deployable service module (zentinel, kanidm, forgejo, …).
///
/// Implements [`Manifest`] so generic catalog infrastructure can
/// filter and look up entries without knowing FSN-specific fields.
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
    #[serde(default)]
    pub category: String,

    /// Service type(s) — backward-compat alias for `category`.
    /// Accepts a single string (`service_type = "iam"`) or array.
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
    pub min_fs_version: Option<String>,

    /// Name of the store this entry was fetched from.
    /// Set by StoreClient when merging results from multiple stores.
    #[serde(default)]
    pub store_source: String,
}

fn default_custom_types() -> Vec<ServiceType> {
    vec![ServiceType::Custom]
}

// ── Manifest impl ─────────────────────────────────────────────────────────────

impl Manifest for StoreEntry {
    fn id(&self) -> &str {
        &self.id
    }
    fn version(&self) -> &str {
        &self.version
    }
    fn category(&self) -> &str {
        &self.category
    }
    fn name(&self) -> &str {
        &self.name
    }
}

// ── StoreEntry methods ────────────────────────────────────────────────────────

impl StoreEntry {
    /// Returns the category-derived display label shown in the TUI service class dropdown.
    /// Format: "Kanidm (IAM)" or "Kanidm (IAM) ↓" when not installed locally.
    pub fn select_label(&self, is_local: bool) -> String {
        let type_label = if !self.category.is_empty() {
            self.category
                .split('.')
                .next_back()
                .unwrap_or(&self.category)
                .to_uppercase()
        } else {
            self.service_types
                .iter()
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
            self.category
                .split('.')
                .next_back()
                .unwrap_or(&self.category)
        } else {
            &self.category
        }
    }
}

// ── NodeStoreClient ───────────────────────────────────────────────────────────

/// I18n bundle fetched from the store (UI strings + metadata).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I18nBundleMeta {
    #[serde(default)]
    pub locale_code: String,
    #[serde(default)]
    pub native_name: String,
    #[serde(default)]
    pub completeness: u8,
}

/// Full i18n bundle returned by [`NodeStoreClient::fetch_i18n`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I18nBundle {
    pub meta: I18nBundleMeta,
    pub ui: toml::Table,
}

/// HTTP client for the FreeSynergy Store.
///
/// Fetches catalog and i18n bundles from a raw-file store URL.
/// Use [`NodeStoreClient::node_store`] for the FSN production store.
pub struct NodeStoreClient {
    base_url: String,
    client: reqwest::Client,
}

impl NodeStoreClient {
    /// Create a client targeting `base_url` (no trailing slash).
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Pre-configured client for the FSN production store.
    pub fn node_store() -> Self {
        Self::new("https://raw.githubusercontent.com/FreeSynergy/fs-store/main")
    }

    /// Fetch `{base_url}/{section}/catalog.toml` and deserialize as `Catalog<T>`.
    ///
    /// `force_refresh` is accepted for API compatibility but has no effect
    /// (caching is handled by the caller via `fs-deploy`'s `StoreClient`).
    pub async fn fetch_catalog<T>(
        &mut self,
        section: &str,
        _force_refresh: bool,
    ) -> anyhow::Result<Catalog<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = format!("{}/{}/catalog.toml", self.base_url, section);
        let text = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("fetch catalog from {url}: {e}"))?
            .error_for_status()
            .map_err(|e| anyhow::anyhow!("catalog HTTP error: {e}"))?
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("read catalog body: {e}"))?;

        toml::from_str(&text).map_err(|e| anyhow::anyhow!("parse catalog TOML: {e}"))
    }

    /// Fetch `{base_url}/{section}/{lang}.toml` and return an [`I18nBundle`].
    pub async fn fetch_i18n(&self, section: &str, lang: &str) -> anyhow::Result<I18nBundle> {
        let url = format!("{}/{}/{lang}.toml", self.base_url, section);
        let text = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("fetch i18n from {url}: {e}"))?
            .error_for_status()
            .map_err(|e| anyhow::anyhow!("i18n HTTP error: {e}"))?
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("read i18n body: {e}"))?;

        toml::from_str(&text).map_err(|e| anyhow::anyhow!("parse i18n TOML: {e}"))
    }
}
