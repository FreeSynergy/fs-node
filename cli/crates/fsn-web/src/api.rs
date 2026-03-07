// REST API routes for the WebUI.

use axum::{
    Json, Router,
    extract::Path,
    response::IntoResponse,
    routing::{get, post},
};
use fsn_podman::systemd::{self, UnitStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ServiceInfo {
    pub name: String,
    pub state: String,
}

pub fn api_routes() -> Router {
    Router::new()
        .route("/api/status", get(status))
        .route("/api/restart/:name", post(restart))
        .route("/api/stop/:name", post(stop))
        .route("/api/start/:name", post(start))
}

async fn status() -> impl IntoResponse {
    let units = systemd::list_fsn_units().await.unwrap_or_default();
    let mut services = Vec::new();
    for unit in &units {
        let name = unit.trim_end_matches(".service").to_string();
        let state = match systemd::status(&name).await {
            Ok(UnitStatus::Active)   => "active",
            Ok(UnitStatus::Inactive) => "inactive",
            Ok(UnitStatus::Failed)   => "failed",
            Ok(UnitStatus::NotFound) => "not-found",
            Err(_)                   => "error",
        };
        services.push(ServiceInfo { name, state: state.to_string() });
    }
    Json(services)
}

async fn restart(Path(name): Path<String>) -> impl IntoResponse {
    match systemd::stop(&name).await.and(systemd::start(&name).await) {
        Ok(()) => Json(serde_json::json!({"ok": true})),
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

async fn stop(Path(name): Path<String>) -> impl IntoResponse {
    match systemd::stop(&name).await {
        Ok(()) => Json(serde_json::json!({"ok": true})),
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

async fn start(Path(name): Path<String>) -> impl IntoResponse {
    match systemd::start(&name).await {
        Ok(()) => Json(serde_json::json!({"ok": true})),
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}
