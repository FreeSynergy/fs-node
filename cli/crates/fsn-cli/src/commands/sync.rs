use std::path::Path;
use anyhow::Result;
use crate::commands::deploy::make_bridge;

pub async fn run(root: &Path, project: Option<&Path>) -> Result<()> {
    let bridge = make_bridge(root, project);
    bridge.sync()
}
