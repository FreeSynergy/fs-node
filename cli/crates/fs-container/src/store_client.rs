// Store client — optionally enrich analysis with data from the FSN store.
//
// The store integration is OPTIONAL: if the store is unreachable (offline or
// not running) the container app manager continues without it.
//
// Contract: store data SUPPLEMENTS the container app manager's own analysis — it never
// overwrites values the container app manager already determined. If there is a conflict
// the user is informed and the container app manager's value takes precedence.

use serde::{Deserialize, Serialize};

use crate::analysis::AnalyzedVar;

// ── Store knowledge types ─────────────────────────────────────────────────────

/// Variable metadata from the store catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreVarMeta {
    pub name: String,
    pub description: Option<String>,
    pub example: Option<String>,
    pub required: Option<bool>,
}

/// Package-level metadata from the store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorePackageMeta {
    pub id: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub vars: Vec<StoreVarMeta>,
}

// ── Enrichment ────────────────────────────────────────────────────────────────

/// Enrichment result: original analysis + any additions from the store.
#[derive(Debug, Clone)]
pub struct EnrichedVar {
    pub analyzed: AnalyzedVar,
    /// Description from the store (if available).
    pub description: Option<String>,
    /// Example value from the store (if available).
    pub example: Option<String>,
    /// Whether the store says this variable is required.
    pub required: Option<bool>,
}

impl EnrichedVar {
    fn from_analyzed(v: AnalyzedVar) -> Self {
        Self {
            analyzed: v,
            description: None,
            example: None,
            required: None,
        }
    }
}

/// Try to enrich analyzed variables with store knowledge.
///
/// - `base_url`: store API base URL (e.g. `"http://localhost:8080"`)
/// - `image`: container image used to look up the package (e.g. `"kanidm/server"`)
/// - `vars`: analyzed variables to enrich
///
/// Returns the enriched list. On any error the original list is returned unchanged.
pub async fn enrich(base_url: &str, image: &str, vars: Vec<AnalyzedVar>) -> Vec<EnrichedVar> {
    match fetch_package_meta(base_url, image).await {
        Ok(meta) => apply_enrichment(vars, &meta),
        Err(e) => {
            tracing::debug!("store enrichment unavailable for {image}: {e}");
            vars.into_iter().map(EnrichedVar::from_analyzed).collect()
        }
    }
}

// ── HTTP fetch ────────────────────────────────────────────────────────────────

async fn fetch_package_meta(base_url: &str, image: &str) -> anyhow::Result<StorePackageMeta> {
    // Encode image as package ID: "kanidm/server:latest" → "kanidm/server"
    let pkg_id = image.split(':').next().unwrap_or(image);
    let url = format!("{base_url}/api/store/know/{pkg_id}");

    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?
        .get(&url)
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("store returned {}", response.status());
    }

    Ok(response.json::<StorePackageMeta>().await?)
}

// ── Enrichment logic ──────────────────────────────────────────────────────────

fn apply_enrichment(vars: Vec<AnalyzedVar>, meta: &StorePackageMeta) -> Vec<EnrichedVar> {
    vars.into_iter()
        .map(|v| {
            let store_var = meta.vars.iter().find(|sv| sv.name == v.name);
            let mut enriched = EnrichedVar::from_analyzed(v);
            if let Some(sv) = store_var {
                // Only fill gaps — never overwrite container app manager's own analysis
                enriched.description = sv.description.clone();
                enriched.example = sv.example.clone();
                enriched.required = sv.required;
            }
            enriched
        })
        .collect()
}
