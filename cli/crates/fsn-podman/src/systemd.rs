// systemd unit management via subprocess (systemctl --user).

use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitStatus {
    Active,
    Inactive,
    Failed,
    NotFound,
}

/// Run systemctl --user daemon-reload (required after writing Quadlet files).
pub async fn daemon_reload() -> Result<()> {
    let status = tokio::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()
        .await?;
    anyhow::ensure!(status.success(), "systemctl --user daemon-reload failed");
    Ok(())
}

/// Start a systemd user unit.
pub async fn start(unit_name: &str) -> Result<()> {
    run_systemctl(&["start", unit_name]).await
}

/// Stop a systemd user unit.
pub async fn stop(unit_name: &str) -> Result<()> {
    run_systemctl(&["stop", unit_name]).await
}

/// Enable a systemd user unit (start on login/linger).
pub async fn enable(unit_name: &str) -> Result<()> {
    run_systemctl(&["enable", unit_name]).await
}

/// Query the active state of a systemd user unit.
pub async fn status(unit_name: &str) -> Result<UnitStatus> {
    let out = tokio::process::Command::new("systemctl")
        .args(["--user", "is-active", unit_name])
        .output()
        .await?;

    Ok(match out.stdout.as_slice() {
        b if b.starts_with(b"active")   => UnitStatus::Active,
        b if b.starts_with(b"inactive") => UnitStatus::Inactive,
        b if b.starts_with(b"failed")   => UnitStatus::Failed,
        _                               => UnitStatus::NotFound,
    })
}

/// List all active fsn-managed user units (units whose names end in ".service"
/// and are listed by `systemctl --user list-units`).
pub async fn list_fsn_units() -> Result<Vec<String>> {
    let out = tokio::process::Command::new("systemctl")
        .args(["--user", "--type=service", "--state=loaded", "--plain", "--no-legend", "--no-pager"])
        .output()
        .await?;

    let units = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let unit = line.split_whitespace().next()?;
            if unit.ends_with(".service") { Some(unit.to_string()) } else { None }
        })
        .collect();

    Ok(units)
}

async fn run_systemctl(args: &[&str]) -> Result<()> {
    let mut full = vec!["--user"];
    full.extend_from_slice(args);
    let st = tokio::process::Command::new("systemctl")
        .args(&full)
        .status()
        .await?;
    anyhow::ensure!(st.success(), "systemctl {:?} failed", args);
    Ok(())
}
