// Store index data model.
//
// The store is a module registry (like apt/npm) distributed as a
// `store/index.toml` file at the root of any store repository.
//
// Field order in index.toml (mandatory): id → name → service_type → version
//   → description → icon → website → repository → author → tags

use serde::{Deserialize, Serialize};

// ── StoreIndex ────────────────────────────────────────────────────────────────

/// The top-level store manifest.
/// Fetched from `{store_url}/store/index.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoreIndex {
    /// All modules listed in this store.
    #[serde(default)]
    pub modules: Vec<StoreEntry>,
}

// ── StoreEntry ────────────────────────────────────────────────────────────────

/// One module entry in the store index.
/// Describes a deployable service (kanidm, forgejo, zentinel, …).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreEntry {
    /// Unique identifier — matches the module class key in the local registry.
    /// Example: "iam/kanidm", "proxy/zentinel".
    pub id: String,

    /// Human-readable display name.
    /// Example: "Kanidm", "Zentinel".
    pub name: String,

    /// Service type category — matches `ServiceType` variants (lowercase).
    /// Example: "iam", "proxy", "wiki".
    pub service_type: String,

    /// Version of the module definition.
    pub version: String,

    /// Short description of what the software does.
    pub description: String,

    /// Relative path to the SVG icon within the store repository.
    /// Used by the web UI — terminal falls back to the module name.
    pub icon: Option<String>,

    /// Official website of the software this module deploys.
    /// Example: "https://kanidm.com" — link to project documentation.
    pub website: Option<String>,

    /// Source code repository of the software (not the module definition).
    /// Example: "https://github.com/kanidm/kanidm".
    pub repository: Option<String>,

    /// Author / maintainer of the module definition.
    pub author: Option<String>,

    /// Searchable tags for the store browser.
    #[serde(default)]
    pub tags: Vec<String>,
}

impl StoreEntry {
    /// Returns the formatted label shown in the TUI service class dropdown.
    /// Format: "Kanidm (IAM)" or "Kanidm (IAM) ↓" when not installed locally.
    pub fn select_label(&self, is_local: bool) -> String {
        let type_upper = self.service_type.to_uppercase();
        if is_local {
            format!("{} ({})", self.name, type_upper)
        } else {
            format!("{} ({}) ↓", self.name, type_upper)
        }
    }
}
