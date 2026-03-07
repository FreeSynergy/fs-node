use std::path::{Path, PathBuf};
use anyhow::Result;
use fsn_ansible::AnsibleBridge;

pub async fn run(root: &Path, project: Option<&Path>, service: Option<&str>) -> Result<()> {
    let bridge = make_bridge(root, project);
    println!("Deploying{}...", service.map(|s| format!(" {}", s)).unwrap_or_default());
    bridge.deploy()
}

pub(crate) fn make_bridge(root: &Path, project: Option<&Path>) -> AnsibleBridge {
    let mut b = AnsibleBridge::new(root);
    b.project_config = project.map(|p| p.to_path_buf()).or_else(|| find_project(root));
    b.vault_file = find_vault(root);
    b
}

fn find_project(root: &Path) -> Option<PathBuf> {
    // Look for the first *.project.yml in projects/
    let projects = root.join("projects");
    std::fs::read_dir(&projects).ok()?.flatten()
        .filter(|e| e.path().is_dir())
        .flat_map(|d| std::fs::read_dir(d.path()).ok()?.collect::<Vec<_>>())
        .flatten()
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().ends_with(".project.toml"))
}

fn find_vault(root: &Path) -> Option<PathBuf> {
    // Look for vault.yml in any project directory
    let projects = root.join("projects");
    std::fs::read_dir(&projects).ok()?.flatten()
        .filter(|e| e.path().is_dir())
        .map(|d| d.path().join("vault.toml"))
        .find(|p| p.exists())
}
