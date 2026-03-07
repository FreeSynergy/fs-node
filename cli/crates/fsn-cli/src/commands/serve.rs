use std::path::Path;
use anyhow::Result;

pub async fn run(_root: &Path, _project: Option<&Path>, bind: &str, port: u16) -> Result<()> {
    fsn_web::serve(bind, port).await
}
