// `fsn server setup` – prepare a Linux server for FreeSynergy.Node.
//
// Replaces playbooks/setup-server.yml
//
// What it does (must be run as root or via sudo):
//   1. Verify Podman ≥ 5.0 is installed
//   2. Ensure the deploy user exists (default: current user)
//   3. Enable systemd linger so user services survive logouts
//   4. Lower unprivileged port start to 80 (net.ipv4.ip_unprivileged_port_start=80)
//   5. Print a summary

use anyhow::{bail, Context, Result};
use std::path::Path;
use tracing::info;

const MIN_PODMAN_MAJOR: u32 = 5;

pub async fn run(_root: &Path) -> Result<()> {
    ServerSetup::new().run().await
}

// ── ServerSetup ───────────────────────────────────────────────────────────────

struct ServerSetup;

impl ServerSetup {
    fn new() -> Self {
        Self
    }

    async fn run(&self) -> Result<()> {
        Self::check_root()?;
        let user = Self::detect_deploy_user();

        let podman_ver = self.check_podman().await?;
        self.ensure_user(&user).await?;
        self.enable_linger(&user).await?;
        self.set_unprivileged_port_start().await?;

        println!();
        println!("━━━  FreeSynergy.Node – Server Setup Complete  ━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  Deploy user:          {user}");
        println!("  Podman:               {podman_ver}");
        println!("  Linger:               enabled");
        println!("  Unprivileged ports:   from 80");
        println!();
        println!("Next step:  su - {user}  &&  fsn init");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        Ok(())
    }

    fn check_root() -> Result<()> {
        let uid = unsafe { libc::getuid() };
        if uid != 0 {
            bail!(
                "fsn server setup must be run as root (current uid: {})",
                uid
            );
        }
        Ok(())
    }

    fn detect_deploy_user() -> String {
        std::env::var("SUDO_USER")
            .ok()
            .filter(|u| !u.is_empty() && u != "root")
            .unwrap_or_else(|| "fsn".to_string())
    }

    async fn check_podman(&self) -> Result<String> {
        let out = tokio::process::Command::new("podman")
            .arg("--version")
            .output()
            .await
            .context("podman not found – install Podman 5+ first")?;

        let stdout = String::from_utf8_lossy(&out.stdout);
        let version = stdout.split_whitespace().last().unwrap_or("?").to_string();
        let major: u32 = version
            .split('.')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        if major < MIN_PODMAN_MAJOR {
            bail!(
                "Podman {version} is too old (minimum: {MIN_PODMAN_MAJOR}.0). \
                 Please upgrade: https://podman.io/getting-started/installation"
            );
        }

        info!("Podman {version} detected – OK");
        Ok(version)
    }

    async fn ensure_user(&self, user: &str) -> Result<()> {
        let st = tokio::process::Command::new("id")
            .arg(user)
            .status()
            .await?;
        if st.success() {
            info!("User '{user}' already exists – skipping creation");
            return Ok(());
        }

        info!("Creating user '{user}'…");
        let st = tokio::process::Command::new("useradd")
            .args(["--create-home", "--shell", "/bin/bash", user])
            .status()
            .await?;

        anyhow::ensure!(st.success(), "useradd {user} failed");
        println!("  Created user '{user}'");
        Ok(())
    }

    async fn enable_linger(&self, user: &str) -> Result<()> {
        info!("Enabling linger for '{user}'…");
        let st = tokio::process::Command::new("loginctl")
            .args(["enable-linger", user])
            .status()
            .await
            .context("loginctl not found – is systemd installed?")?;

        anyhow::ensure!(st.success(), "loginctl enable-linger {user} failed");
        Ok(())
    }

    async fn set_unprivileged_port_start(&self) -> Result<()> {
        let conf_file = "/etc/sysctl.d/99-fs-unprivileged-ports.conf";
        std::fs::write(conf_file, "net.ipv4.ip_unprivileged_port_start = 80\n")
            .with_context(|| format!("writing {conf_file}"))?;

        let st = tokio::process::Command::new("sysctl")
            .args(["--system"])
            .status()
            .await
            .context("sysctl not found")?;

        anyhow::ensure!(st.success(), "sysctl --system failed");
        info!("net.ipv4.ip_unprivileged_port_start set to 80");
        Ok(())
    }
}
