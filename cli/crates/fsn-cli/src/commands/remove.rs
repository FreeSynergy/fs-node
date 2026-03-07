use std::path::Path;
use anyhow::{bail, Result};

use super::deploy::make_bridge;

pub async fn run(root: &Path, project: Option<&Path>, service: Option<&str>, confirm: bool) -> Result<()> {
    if !confirm {
        bail!(
            "Remove deletes ALL data for {}. Re-run with --confirm to proceed.",
            service.unwrap_or("all services")
        );
    }
    let bridge = make_bridge(root, project);
    println!("Removing{}...", service.map(|s| format!(" {}", s)).unwrap_or_default());
    bridge.remove()
}
