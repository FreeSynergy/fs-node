use std::path::Path;
use anyhow::Result;
use crate::commands::deploy::make_bridge;

pub async fn run(root: &Path, project: Option<&Path>, service: Option<&str>) -> Result<()> {
    let bridge = make_bridge(root, project);
    bridge.restart()
}
