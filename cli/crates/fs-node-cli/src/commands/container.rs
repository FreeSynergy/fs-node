// `fsn container-app` — compose YAML → Quadlet pipeline + service management.
//
// All container lifecycle operations go through systemctl --user and journalctl.
// No podman socket, no bollard. Quadlet files are written to
// ~/.config/containers/systemd/ and managed by systemd's quadlet generator.

use std::path::Path;

use anyhow::Result;
use fs_container::{QuadletManager, SystemctlManager};

// ── ContainerCmd ──────────────────────────────────────────────────────────────

pub struct ContainerCmd;

impl ContainerCmd {
    /// Parse + analyze a compose file and print a variable report.
    pub async fn analyze(&self, path: &Path, name: Option<&str>, _offline: bool) -> Result<()> {
        let result = fs_container::analyze(path, name)?;
        result.print_report();
        Ok(())
    }

    /// Full pipeline: parse → validate → generate quadlets → daemon-reload.
    pub async fn install(
        &self,
        path: &Path,
        name: Option<&str>,
        dry_run: bool,
        store_url: Option<&str>,
    ) -> Result<()> {
        fs_container::install(path, name, dry_run, store_url).await?;
        Ok(())
    }

    /// Start a container-app-managed service.
    pub async fn start(&self, service: &str) -> Result<()> {
        SystemctlManager::user()
            .start(&Self::unit(service))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        println!("Started: {service}");
        Ok(())
    }

    /// Stop a container-app-managed service.
    pub async fn stop(&self, service: &str) -> Result<()> {
        SystemctlManager::user()
            .stop(&Self::unit(service))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        println!("Stopped: {service}");
        Ok(())
    }

    /// Restart a container-app-managed service.
    pub async fn restart(&self, service: &str) -> Result<()> {
        SystemctlManager::user()
            .restart(&Self::unit(service))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        println!("Restarted: {service}");
        Ok(())
    }

    /// Show recent log lines via journalctl.
    pub async fn logs(&self, service: &str, lines: usize) -> Result<()> {
        let output = QuadletManager::user_default()
            .service_logs(service, lines)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        for line in output {
            println!("{line}");
        }
        Ok(())
    }

    /// Show systemctl status of a container-app-managed service.
    pub async fn status(&self, service: &str) -> Result<()> {
        let status = SystemctlManager::user()
            .service_status(&Self::unit(service))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        println!("{:?}", status);
        Ok(())
    }

    /// List all FSN-managed systemd services.
    pub async fn list(&self) -> Result<()> {
        let output = tokio::process::Command::new("systemctl")
            .args([
                "--user",
                "--type=service",
                "--plain",
                "--no-legend",
                "--no-pager",
                "--state=loaded",
            ])
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let units: Vec<&str> = stdout.lines().filter(|l| l.contains("fs-")).collect();

        if units.is_empty() {
            println!("No container-app-managed services found.");
            return Ok(());
        }

        println!("{:<32} {:<12} SUB", "SERVICE", "ACTIVE");
        println!("{}", "─".repeat(60));
        for line in units {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let name = parts[0].trim_end_matches(".service");
                let active = parts.get(2).copied().unwrap_or("-");
                let sub = parts.get(3).copied().unwrap_or("-");
                println!("{:<32} {:<12} {}", name, active, sub);
            }
        }
        Ok(())
    }

    fn unit(service: &str) -> String {
        if service.ends_with(".service") {
            service.to_string()
        } else {
            format!("fs-{service}.service")
        }
    }
}
