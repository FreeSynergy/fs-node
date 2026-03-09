// Store client – fetches and merges store indices from configured URLs.
//
// Each store is a repository with a `store/index.toml` at its root.
// The client merges all enabled stores into a unified module list,
// annotating each entry with whether it is already installed locally.
//
// HTTP fetching is async (reqwest); index parsing is synchronous (toml).

use std::path::Path;

use anyhow::{Context, Result};

use fsn_core::{
    config::{AppSettings, ServiceRegistry},
    store::{StoreEntry, StoreIndex},
};

// ── StoreClient ───────────────────────────────────────────────────────────────

/// Manages store indices and local module availability.
pub struct StoreClient {
    /// User-configured stores (from AppSettings).
    settings: AppSettings,
    /// Local registry — used to check `is_local()`.
    registry: ServiceRegistry,
}

impl StoreClient {
    pub fn new(settings: AppSettings, registry: ServiceRegistry) -> Self {
        Self { settings, registry }
    }

    /// Returns `true` when the module id is present in the local registry.
    pub fn is_local(&self, id: &str) -> bool {
        self.registry.get(id).is_some()
    }

    /// Fetch the index from a store URL.
    ///
    /// Index URL: `{store_url}/store/index.toml`.
    /// Returns an empty index on network error (caller shows "unavailable").
    pub async fn fetch_index(&self, store_url: &str) -> Result<StoreIndex> {
        let url = format!("{}/store/index.toml", store_url.trim_end_matches('/'));
        let text = reqwest::get(&url)
            .await
            .with_context(|| format!("fetching store index from {url}"))?
            .text()
            .await
            .with_context(|| "reading store index response")?;
        toml::from_str(&text).with_context(|| format!("parsing store index from {url}"))
    }

    /// Fetch and merge all enabled store indices into a single list.
    ///
    /// Entries from earlier stores take precedence when IDs collide.
    /// Each `StoreEntry` is annotated with `is_local` at call time.
    pub async fn fetch_all(&self) -> Vec<StoreEntry> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for store in &self.settings.stores {
            if !store.enabled { continue }
            match self.fetch_index(&store.url).await {
                Ok(index) => {
                    for entry in index.modules {
                        if seen.insert(entry.id.clone()) {
                            result.push(entry);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Store '{}' unavailable: {:#}", store.name, e);
                }
            }
        }
        result
    }

    /// Returns all entries for a given service type from the merged index.
    /// Used by the wizard to populate the service class dropdown.
    pub fn list_by_type<'a>(entries: &'a [StoreEntry], service_type: &str) -> Vec<&'a StoreEntry> {
        entries.iter()
            .filter(|e| e.service_type == service_type)
            .collect()
    }

    /// Load a bundled (offline) index from the local modules directory.
    ///
    /// Reads `{modules_dir}/../store/index.toml` — the index shipped with FSN.
    /// Falls back to an empty index when the file is absent.
    pub fn load_bundled(modules_dir: &Path) -> StoreIndex {
        let path = modules_dir.parent()
            .unwrap_or(modules_dir)
            .join("store")
            .join("index.toml");
        if !path.exists() { return StoreIndex::default(); }
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }
}
