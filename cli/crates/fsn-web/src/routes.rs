// Static SPA routes – serves the embedded index.html.

use axum::{Router, routing::get, response::Html};

const INDEX_HTML: &str = include_str!("../static/index.html");

pub fn ui_routes() -> Router {
    Router::new().route("/", get(index))
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}
