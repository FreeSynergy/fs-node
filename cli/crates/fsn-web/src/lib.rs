// fsn-web – WebUI backend.
// Embedded HTMX + Alpine.js SPA, served from memory (no Node.js build needed).
//
// Routes:
//   GET  /                    → index.html (SPA shell)
//   GET  /api/status          → Vec<ServiceStatus>
//   POST /api/deploy          → trigger deploy
//   POST /api/undeploy/{name} → trigger undeploy
//   POST /api/restart/{name}  → trigger restart
//   GET  /api/logs/{name}     → SSE log stream (future)

pub mod api;
pub mod routes;

use anyhow::Result;
use axum::Router;
use tower_http::trace::TraceLayer;

/// Start the management WebUI on `bind:port`.
pub async fn serve(bind: &str, port: u16) -> Result<()> {
    let app = Router::new()
        .merge(routes::ui_routes())
        .merge(api::api_routes())
        .layer(TraceLayer::new_for_http());

    let addr = format!("{}:{}", bind, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("FSN WebUI listening on http://{}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}
