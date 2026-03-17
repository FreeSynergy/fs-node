// `fsn serve` — embedded HTTP server + S3 storage server.
//
// HTTP routes (all under /api/store/know/):
//   GET  /api/store/know/catalog          → full catalog as JSON
//   GET  /api/store/know/search?q=...     → filtered catalog
//   GET  /api/store/know/package/:id      → single package details
//   GET  /api/store/know/installed        → installed packages from DB
//   GET  /api/store/know/i18n             → available language packs
//
// S3 server (default port 9000):
//   Standard AWS S3 API, backed by the local filesystem.
//   Buckets: profiles (public), backups, media, packages, shared.
//
// The Desktop (fsd) connects to the HTTP API to render the Store UI.
// Remote nodes connect to the S3 port for federation.

use std::path::Path;

use anyhow::Result;
use axum::{
    Router,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
};
use fsn_db::InstalledPackageRepo;
use fsn_node_core::store::StoreEntry;
use fsn_s3::{S3Server, StorageConfig};
use fsn_store::StoreClient;
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

// ── shared state ──────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState;

// ── query params ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: String,
}

// ── response types ────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct InstalledRow {
    package_id:   String,
    version:      String,
    package_type: String,
    channel:      String,
    active:       bool,
    installed_at: i64,
}

// ── run ───────────────────────────────────────────────────────────────────────

pub async fn run(root: &Path, _project: Option<&Path>, bind: &str, port: u16) -> Result<()> {
    let addr = format!("{bind}:{port}");

    // ── S3 server ─────────────────────────────────────────────────────────────
    let s3_config = StorageConfig {
        enabled:    true,
        port:       9000,
        bind:       "127.0.0.1".to_owned(),
        data_root:  root.join("storage"),
        access_key: "fsn_local".to_owned(),
        secret_key: "changeme_secret_key".to_owned(),
        sync:       None,
    };
    let s3 = S3Server::new(s3_config);
    let _s3_handle = s3.start().await?;

    // ── HTTP store API ────────────────────────────────────────────────────────
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/store/know/catalog",       get(handle_catalog))
        .route("/api/store/know/search",        get(handle_search))
        .route("/api/store/know/package/:id",   get(handle_package))
        .route("/api/store/know/installed",     get(handle_installed))
        .route("/api/store/know/i18n",          get(handle_i18n))
        .layer(cors)
        .with_state(AppState);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("fsn store API listening on http://{addr}");
    println!("Store API : http://{addr}/api/store/know/");
    println!("S3 API    : http://127.0.0.1:9000");
    println!("Press Ctrl+C to stop.");

    axum::serve(listener, app).await?;
    Ok(())
}

// ── handlers ──────────────────────────────────────────────────────────────────

async fn handle_catalog(
    State(_): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let catalog = fetch_catalog().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let packages: Vec<serde_json::Value> = catalog.packages.iter().map(entry_to_json).collect();
    Ok(Json(serde_json::json!({
        "catalog": {
            "project":      catalog.catalog.project,
            "version":      catalog.catalog.version,
            "generated_at": catalog.catalog.generated_at,
        },
        "packages": packages,
    })))
}

async fn handle_search(
    State(_): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let catalog = fetch_catalog().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let q = params.q.to_lowercase();

    let matches: Vec<serde_json::Value> = catalog.packages.iter()
        .filter(|e| {
            q.is_empty()
                || e.name.to_lowercase().contains(&q)
                || e.id.to_lowercase().contains(&q)
                || e.description.to_lowercase().contains(&q)
                || e.tags.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .map(entry_to_json)
        .collect();

    Ok(Json(serde_json::json!({ "packages": matches, "total": matches.len() })))
}

async fn handle_package(
    State(_): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let catalog = fetch_catalog().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let decoded = id.replace("%2F", "/").replace("%2f", "/");

    match catalog.packages.iter().find(|e| e.id == decoded) {
        Some(e) => Ok(Json(entry_to_json(e))),
        None    => Err(StatusCode::NOT_FOUND),
    }
}

async fn handle_installed(
    State(_): State<AppState>,
) -> Result<Json<Vec<InstalledRow>>, StatusCode> {
    let Some(conn) = crate::db::get_conn() else {
        return Ok(Json(vec![]));
    };

    let repo = InstalledPackageRepo::new(conn.inner());
    let rows = repo.list_all().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result = rows.into_iter()
        .filter(|r| r.active)
        .map(|r| InstalledRow {
            package_id:   r.package_id,
            version:      r.version,
            package_type: r.package_type,
            channel:      r.channel,
            active:       r.active,
            installed_at: r.installed_at,
        })
        .collect();

    Ok(Json(result))
}

async fn handle_i18n(
    State(_): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let catalog = fetch_catalog().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    let locales: Vec<serde_json::Value> = catalog.locales.iter().map(|l| serde_json::json!({
        "code":         l.code,
        "name":         l.name,
        "completeness": l.completeness,
        "direction":    l.direction,
    })).collect();

    Ok(Json(serde_json::json!({ "locales": locales })))
}

// ── helpers ───────────────────────────────────────────────────────────────────

async fn fetch_catalog() -> Result<fsn_store::Catalog<StoreEntry>> {
    let mut client = StoreClient::node_store();
    client.fetch_catalog("Node", false).await.map_err(anyhow::Error::from)
}

fn entry_to_json(e: &StoreEntry) -> serde_json::Value {
    serde_json::json!({
        "id":          e.id,
        "name":        e.name,
        "category":    e.category,
        "version":     e.version,
        "description": e.description,
        "icon":        e.icon,
        "license":     e.license,
        "website":     e.website,
        "repository":  e.repository,
        "tags":        e.tags,
    })
}
