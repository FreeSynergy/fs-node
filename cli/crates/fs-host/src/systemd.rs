//! Remote systemd control via SSH.

use anyhow::Result;
use tracing::info;

use crate::session::SshSession;

/// Controls systemd units on a remote host via SSH.
pub struct RemoteSystemd<'a> {
    session: &'a SshSession,
    /// Use `--user` scope (rootless Podman). Set false for system-level units.
    pub user: bool,
}

impl<'a> RemoteSystemd<'a> {
    pub fn new(session: &'a SshSession) -> Self {
        Self {
            session,
            user: true,
        }
    }

    pub fn system(session: &'a SshSession) -> Self {
        Self {
            session,
            user: false,
        }
    }

    /// `systemctl [--user] daemon-reload`
    pub async fn daemon_reload(&self) -> Result<()> {
        info!("remote: systemctl daemon-reload");
        self.run("daemon-reload").await
    }

    /// `systemctl [--user] start <unit>`
    pub async fn start(&self, unit: &str) -> Result<()> {
        info!("remote: systemctl start {unit}");
        self.run(&format!("start {unit}")).await
    }

    /// `systemctl [--user] stop <unit>`
    pub async fn stop(&self, unit: &str) -> Result<()> {
        info!("remote: systemctl stop {unit}");
        self.run(&format!("stop {unit}")).await
    }

    /// `systemctl [--user] enable <unit>`
    pub async fn enable(&self, unit: &str) -> Result<()> {
        info!("remote: systemctl enable {unit}");
        self.run(&format!("enable {unit}")).await
    }

    /// `systemctl [--user] disable <unit>`
    pub async fn disable(&self, unit: &str) -> Result<()> {
        info!("remote: systemctl disable {unit}");
        self.run(&format!("disable {unit}")).await
    }

    /// `systemctl [--user] restart <unit>`
    pub async fn restart(&self, unit: &str) -> Result<()> {
        info!("remote: systemctl restart {unit}");
        self.run(&format!("restart {unit}")).await
    }

    /// `systemctl [--user] is-active <unit>` — returns true if active.
    pub async fn is_active(&self, unit: &str) -> Result<bool> {
        let out = self
            .session
            .exec(&self.cmd(&format!("is-active {unit}")))
            .await?;
        Ok(out.exit_code == 0)
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    fn scope_flag(&self) -> &'static str {
        if self.user {
            "--user"
        } else {
            ""
        }
    }

    fn cmd(&self, sub: &str) -> String {
        format!("systemctl {} {sub}", self.scope_flag())
    }

    async fn run(&self, sub: &str) -> Result<()> {
        self.session.exec(&self.cmd(sub)).await?.into_result()?;
        Ok(())
    }
}
