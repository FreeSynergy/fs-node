// fsn-ansible – Ansible subprocess bridge (Phase 1 only).
//
// This crate wraps ansible-playbook calls so the CLI can delegate to Ansible
// while the native Rust engine (fsn-engine) is being built.
// Will be removed in Phase 3 when fsn-engine + fsn-podman replace Ansible.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

/// Configuration for the Ansible bridge.
pub struct AnsibleBridge {
    /// Root of the FSN repo (contains playbooks/, modules/, etc.)
    pub fsn_root: PathBuf,

    /// Path to the project config file (project.yml)
    pub project_config: Option<PathBuf>,

    /// Path to the vault/secrets file
    pub vault_file: Option<PathBuf>,

    /// User to deploy as (passed as deploy_user extra-var)
    pub deploy_user: Option<String>,
}

impl AnsibleBridge {
    pub fn new(fsn_root: &Path) -> Self {
        Self {
            fsn_root: fsn_root.to_path_buf(),
            project_config: None,
            vault_file: None,
            deploy_user: None,
        }
    }

    /// Run a playbook with the configured extra vars.
    pub fn run(&self, playbook: &str, extra: &[(&str, &str)]) -> Result<()> {
        let pb_path = self.fsn_root.join("playbooks").join(playbook);
        if !pb_path.exists() {
            bail!("Playbook not found: {}", pb_path.display());
        }

        let mut cmd = Command::new("ansible-playbook");
        cmd.arg(&pb_path);

        if let Some(ref project) = self.project_config {
            cmd.arg("-e").arg(format!("project_config={}", project.display()));
        }

        if let Some(ref vault) = self.vault_file {
            cmd.arg("-e").arg(format!("@{}", vault.display()));
        }

        if let Some(ref user) = self.deploy_user {
            cmd.arg("-e").arg(format!("deploy_user={}", user));
        }

        for (key, value) in extra {
            cmd.arg("-e").arg(format!("{}={}", key, value));
        }

        let status = cmd
            .status()
            .with_context(|| format!("Failed to run ansible-playbook {}", playbook))?;

        if !status.success() {
            bail!("ansible-playbook {} failed (exit code: {:?})", playbook, status.code());
        }

        Ok(())
    }

    // ── Operation shortcuts ───────────────────────────────────────────────────

    /// Setup the server OS (Podman, deploy user, unprivileged ports).
    pub fn setup_server(&self) -> Result<()> {
        self.run("setup-server.yml", &[])
    }

    /// Deploy / sync the full project stack.
    pub fn deploy(&self) -> Result<()> {
        self.run("deploy-stack.yml", &[])
    }

    /// Undeploy all services (stop, keep data).
    pub fn undeploy(&self) -> Result<()> {
        self.run("undeploy-stack.yml", &[])
    }

    /// Show what would change without applying (dry-run).
    pub fn sync(&self) -> Result<()> {
        self.run("sync-stack.yml", &[])
    }

    /// Pull new images and redeploy changed modules.
    pub fn update(&self) -> Result<()> {
        self.run("update-stack.yml", &[])
    }

    /// Restart all services.
    pub fn restart(&self) -> Result<()> {
        self.run("restart-stack.yml", &[])
    }

    /// Remove orphaned containers/volumes.
    pub fn clean(&self) -> Result<()> {
        self.run("clean-stack.yml", &[])
    }

    /// Remove all containers and data for the project.
    pub fn remove(&self) -> Result<()> {
        self.run("remove-stack.yml", &[])
    }

    /// Install a new project (create directories, generate examples).
    pub fn install_project(&self) -> Result<()> {
        self.run("install-project.yml", &[])
    }
}
