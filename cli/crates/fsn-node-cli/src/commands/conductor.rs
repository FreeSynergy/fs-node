// `fsn conductor` — compose YAML → Quadlet pipeline + service management.
//
// All container lifecycle operations go through systemctl --user and journalctl.
// No podman socket, no bollard. Quadlet files are written to
// ~/.config/containers/systemd/ and managed by systemd's quadlet generator.

use std::path::Path;

use anyhow::Result;
use fsn_container::{QuadletManager, SystemctlManager};

// ── Analyze ───────────────────────────────────────────────────────────────────

/// Parse + analyze a compose file and print a variable report.
pub async fn analyze(path: &Path, name: Option<&str>, _offline: bool) -> Result<()> {
    let result = fsn_conductor::analyze(path, name)?;
    result.print_report();
    Ok(())
}

// ── Install ───────────────────────────────────────────────────────────────────

/// Full pipeline: parse → validate → generate quadlets → daemon-reload.
pub async fn install(
    path:      &Path,
    name:      Option<&str>,
    dry_run:   bool,
    store_url: Option<&str>,
) -> Result<()> {
    fsn_conductor::install(path, name, dry_run, store_url).await?;
    Ok(())
}

// ── Service management ────────────────────────────────────────────────────────

/// Start a conductor-managed service.
pub async fn start(service: &str) -> Result<()> {
    let mgr = SystemctlManager::user();
    mgr.start(&unit(service)).await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("Started: {service}");
    Ok(())
}

/// Stop a conductor-managed service.
pub async fn stop(service: &str) -> Result<()> {
    let mgr = SystemctlManager::user();
    mgr.stop(&unit(service)).await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("Stopped: {service}");
    Ok(())
}

/// Restart a conductor-managed service.
pub async fn restart(service: &str) -> Result<()> {
    let mgr = SystemctlManager::user();
    mgr.restart(&unit(service)).await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("Restarted: {service}");
    Ok(())
}

/// Show recent log lines via journalctl.
pub async fn logs(service: &str, lines: usize) -> Result<()> {
    let mgr = QuadletManager::user_default();
    let output = mgr.service_logs(service, lines).await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    for line in output {
        println!("{line}");
    }
    Ok(())
}

/// Show systemctl status of a conductor-managed service.
pub async fn status(service: &str) -> Result<()> {
    let mgr = SystemctlManager::user();
    let status = mgr.service_status(&unit(service)).await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("{:?}", status);
    Ok(())
}

/// List all FSN-managed systemd services.
pub async fn list() -> Result<()> {
    let output = tokio::process::Command::new("systemctl")
        .args([
            "--user", "--type=service",
            "--plain", "--no-legend", "--no-pager",
            "--state=loaded",
        ])
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let units: Vec<&str> = stdout
        .lines()
        .filter(|l| l.contains("fsn-"))
        .collect();

    if units.is_empty() {
        println!("No conductor-managed services found.");
        return Ok(());
    }

    println!("{:<32} {:<12} {}", "SERVICE", "ACTIVE", "SUB");
    println!("{}", "─".repeat(60));
    for line in units {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let name   = parts[0].trim_end_matches(".service");
            let active = parts.get(2).copied().unwrap_or("-");
            let sub    = parts.get(3).copied().unwrap_or("-");
            println!("{:<32} {:<12} {}", name, active, sub);
        }
    }
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn unit(service: &str) -> String {
    if service.ends_with(".service") {
        service.to_string()
    } else {
        format!("fsn-{service}.service")
    }
}
