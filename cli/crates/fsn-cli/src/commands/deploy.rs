// fsn deploy – reconcile desired state and start/update services.
// Replaces: ansible-playbook playbooks/deploy-stack.yml

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fsn_core::{
    config::{HostConfig, ServiceRegistry, ProjectConfig, VaultConfig, resolve_plugins_dir},
};
use fsn_deploy::{
    deploy::{DeployOpts, deploy_all},
    diff::compute_diff,
    observe::observe,
    resolve::resolve_desired,
};
use fsn_host::RemoteHost;
use tracing::info;

pub async fn run(
    root:       &Path,
    project:    Option<&Path>,
    service:    Option<&str>,
    target_host: Option<&str>,
) -> Result<()> {
    // ── Load configs ──────────────────────────────────────────────────────────
    let project_path = find_project(root, project)
        .context("No project file found. Run `fsn init` first.")?;
    let proj = ProjectConfig::load(&project_path)?;

    let host_path = find_host(root)
        .context("No host file found. Run `fsn init` first.")?;
    let host = HostConfig::load(&host_path)?;

    let vault_pass = std::env::var("FSN_VAULT_PASS").ok();
    let vault = VaultConfig::load(
        project_path.parent().unwrap_or(root),
        vault_pass.as_deref(),
    )?;

    let registry = ServiceRegistry::load(&resolve_plugins_dir(root))?;

    // ── Resolve desired state ─────────────────────────────────────────────────
    let data_root = project_path.parent()
        .map(|p| p.join("data"))
        .unwrap_or_else(|| root.join("data"));
    let desired = resolve_desired(&proj, &host, &registry, &vault, Some(&data_root))
        .context("Resolving desired state")?;

    // ── Observe actual state ──────────────────────────────────────────────────
    let actual = observe().await?;

    // ── Compute diff ──────────────────────────────────────────────────────────
    let diff = compute_diff(&desired, &actual);

    if diff.is_empty() && service.is_none() {
        println!("Nothing to do – all services are already up to date.");
        return Ok(());
    }

    info!("Deploy plan: {}", diff.summary());

    // Filter to a single service if requested
    let deploy_desired = if let Some(svc) = service {
        use fsn_core::state::DesiredState;
        let services = desired.services.into_iter()
            .filter(|m| m.name == svc || m.sub_services.iter().any(|s| s.name == svc))
            .collect();
        DesiredState { services, ..desired }
    } else {
        desired
    };

    // ── Build DeployOpts (local or remote) ───────────────────────────────────
    let mut opts = DeployOpts::default_for_user();

    if let Some(host_name) = target_host {
        let remote = find_remote_host(root, host_name)
            .with_context(|| format!("Host '{host_name}' not found. Check your *.host.toml files."))?;
        opts.remote_host = Some(remote);
    }

    deploy_all(&deploy_desired, &proj, &vault, &opts, root, &data_root).await
        .context("Deploy failed")?;

    crate::db::write_audit_entry(
        &fsn_core::audit::AuditEntry::new("system", "deploy", "project", &proj.project.meta.name),
    ).await;

    println!("\n✓ Deploy complete ({} service(s))", deploy_desired.services.len());
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub(crate) fn find_project(root: &Path, explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit { return Some(p.to_path_buf()); }
    let projects = root.join("projects");
    std::fs::read_dir(&projects).ok()?.flatten()
        .filter(|e| e.path().is_dir())
        .flat_map(|d| std::fs::read_dir(d.path()).into_iter().flatten().flatten())
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|e| e.to_str()) == Some("toml")
              && p.to_string_lossy().contains(".project."))
}

pub(crate) fn find_host(root: &Path) -> Option<PathBuf> {
    // Primary: project directories (TUI stores hosts as projects/{slug}/*.host.toml)
    let projects_dir = root.join("projects");
    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for proj_dir in entries.flatten().filter(|e| e.path().is_dir()) {
            if let Ok(inner) = std::fs::read_dir(proj_dir.path()) {
                let found = inner.flatten().map(|e| e.path()).find(|p| {
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    p.extension().and_then(|e| e.to_str()) == Some("toml")
                        && name.ends_with(".host.toml")
                        && name != "example.host.toml"
                });
                if let Some(host_path) = found {
                    return Some(host_path);
                }
            }
        }
    }
    // Fallback: legacy global hosts/ directory
    let hosts = root.join("hosts");
    std::fs::read_dir(&hosts).ok()?.flatten()
        .map(|e| e.path())
        .find(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            p.extension().and_then(|e| e.to_str()) == Some("toml")
                && name.ends_with(".host.toml")
                && name != "example.host.toml"
        })
}

/// Find a RemoteHost config by name from any *.host.toml in the projects tree.
/// The host name is matched against the `[host].name` field.
fn find_remote_host(root: &Path, host_name: &str) -> Option<RemoteHost> {
    let host_path = find_host_by_name(root, host_name)?;
    let cfg = HostConfig::load(&host_path).ok()?;
    let h = &cfg.host;
    Some(RemoteHost {
        name:         h.meta.name.clone(),
        address:      h.addr().to_string(),
        ssh_port:     h.ssh_port,
        ssh_user:     h.ssh_user.clone(),
        ssh_key_path: h.ssh_key_path.clone(),
    })
}

fn find_host_by_name(root: &Path, name: &str) -> Option<PathBuf> {
    let projects_dir = root.join("projects");
    for proj_dir in std::fs::read_dir(&projects_dir).ok()?.flatten().filter(|e| e.path().is_dir()) {
        for entry in std::fs::read_dir(proj_dir.path()).ok()?.flatten() {
            let path = entry.path();
            let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if fname.ends_with(".host.toml") && fname != "example.host.toml" {
                if fname.starts_with(&format!("{name}.")) || fname == &format!("{name}.host.toml") {
                    return Some(path);
                }
                // Also try loading and matching the host name field
                if let Ok(h) = HostConfig::load(&path) {
                    if h.host.name() == name {
                        return Some(path);
                    }
                }
            }
        }
    }
    // Fallback: legacy hosts/ directory
    let hosts = root.join("hosts");
    for entry in std::fs::read_dir(&hosts).ok()?.flatten() {
        let path = entry.path();
        let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if fname.ends_with(".host.toml") && fname != "example.host.toml" {
            if fname.starts_with(&format!("{name}.")) {
                return Some(path);
            }
        }
    }
    None
}
