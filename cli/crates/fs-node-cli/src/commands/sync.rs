// fsn sync – show what would change without applying anything.

use anyhow::Result;
use fs_deploy::{diff::compute_diff, observe::observe, resolve::resolve_desired};
use fs_node_core::config::{
    find_host, find_project, resolve_plugins_dir, HostConfig, ProjectConfig, ServiceRegistry,
    VaultConfig,
};
use std::path::Path;

pub async fn run(root: &Path, project: Option<&Path>) -> Result<()> {
    let proj_path =
        find_project(root, project).ok_or_else(|| anyhow::anyhow!("No project file found"))?;
    let host_path = find_host(root).ok_or_else(|| anyhow::anyhow!("No host file found"))?;
    let proj = ProjectConfig::load(&proj_path)?;
    let host = HostConfig::load(&host_path)?;
    let vault_pass = std::env::var("FS_VAULT_PASS").ok();
    let vault = VaultConfig::load(proj_path.parent().unwrap_or(root), vault_pass.as_deref())?;
    let registry = ServiceRegistry::load(&resolve_plugins_dir(root))?;
    let desired = resolve_desired(&proj, &host, &registry, &vault, None)?;
    let actual = observe().await?;
    let diff = compute_diff(&desired, &actual);

    if diff.is_empty() {
        println!("{}", fs_i18n::t("sync.up-to-date"));
        return Ok(());
    }

    if !diff.to_deploy.is_empty() {
        println!(
            "{}",
            fs_i18n::t_with(
                "sync.to-deploy",
                &[("n", &diff.to_deploy.len().to_string())]
            )
        );
        for m in &diff.to_deploy {
            println!("  + {}", m.name);
        }
    }
    if !diff.to_update.is_empty() {
        println!(
            "{}",
            fs_i18n::t_with(
                "sync.to-update",
                &[("n", &diff.to_update.len().to_string())]
            )
        );
        for m in &diff.to_update {
            println!("  ~ {}", m.name);
        }
    }
    if !diff.to_remove.is_empty() {
        println!(
            "{}",
            fs_i18n::t_with(
                "sync.to-remove",
                &[("n", &diff.to_remove.len().to_string())]
            )
        );
        for n in &diff.to_remove {
            println!("  - {}", n);
        }
    }
    Ok(())
}
