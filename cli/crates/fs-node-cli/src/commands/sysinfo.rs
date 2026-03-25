// `fsn sysinfo` — display and monitor system information.
//
// Subcommands (via flags):
//   fsn sysinfo              → static data (OsInfo + DetectedFeatures) from cache
//   fsn sysinfo --live       → live data (disk, memory, temperature)
//   fsn sysinfo --refresh    → clear cache, re-detect
//   fsn sysinfo --check <f>  → check if a single feature is available
//   fsn sysinfo --monitor    → continuous alert loop (publishes to bus if running)

use anyhow::Result;
use fs_sysinfo::{
    AlertChecker, AlertThresholds, DiskInfo, Feature, FeatureDetect, MemInfo, SysInfoCache,
    ThermalInfo,
};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

// ── fsn sysinfo (static) ──────────────────────────────────────────────────────

/// Show static system information (uses 24-hour cache; re-detects if stale).
pub async fn info() -> Result<()> {
    let cache = SysInfoCache::default_path();
    let (os, features) = cache.get_or_detect();

    println!("── OS ──────────────────────────────────────────");
    println!("  OS:       {} ({})", os.version, os.os_type.label());
    println!("  Arch:     {}", os.arch);
    println!("  Kernel:   {}", os.kernel);
    println!("  Hostname: {}", os.hostname);

    println!("\n── Features ────────────────────────────────────");
    if features.available.is_empty() {
        println!("  (none detected)");
    } else {
        for f in &features.available {
            println!("  ✅ {}", f.label());
        }
    }

    let all = [
        Feature::Systemd,
        Feature::Pam,
        Feature::Launchd,
        Feature::WindowsServices,
        Feature::Podman,
        Feature::Docker,
        Feature::Git,
        Feature::Ssh,
        Feature::Smartctl,
    ];
    let missing: Vec<_> = all.iter().filter(|f| !features.has(**f)).collect();
    if !missing.is_empty() {
        for f in &missing {
            println!("  ❌ {}", f.label());
        }
    }

    println!("\n  Cache: {}", cache.path().display());
    Ok(())
}

// ── fsn sysinfo --live ────────────────────────────────────────────────────────

/// Show live (on-demand) system metrics: memory, disk, CPU temperature.
pub async fn live() -> Result<()> {
    // Re-read OS info (not cached, to show current hostname etc.)
    let (os, _) = SysInfoCache::default_path().get_or_detect();

    println!("── OS ──────────────────────────────────────────");
    println!(
        "  {} — {} ({})",
        os.hostname,
        os.version,
        os.os_type.label()
    );

    // Memory
    let mem = MemInfo::detect();
    println!("\n── Memory ──────────────────────────────────────");
    println!("  Total:     {:>7.2} GiB", mem.total_bytes as f64 / GIB);
    println!(
        "  Used:      {:>7.2} GiB  ({:.1}%)",
        mem.used_bytes as f64 / GIB,
        mem.used_percent()
    );
    println!("  Available: {:>7.2} GiB", mem.available_bytes as f64 / GIB);
    if mem.swap_total_bytes > 0 {
        println!(
            "  Swap:      {:>7.2} / {:.2} GiB",
            mem.swap_used_bytes as f64 / GIB,
            mem.swap_total_bytes as f64 / GIB,
        );
    }

    // Disk
    let disk = DiskInfo::detect();
    println!("\n── Disks ───────────────────────────────────────");
    for part in &disk.partitions {
        println!(
            "  {:<22} {:>6.1} / {:>6.1} GiB  ({:>5.1}%)  [{}]",
            part.mount_point,
            part.used_bytes() as f64 / GIB,
            part.total_bytes as f64 / GIB,
            part.used_percent(),
            part.fs_type,
        );
    }

    // Temperature
    let thermal = ThermalInfo::detect();
    if !thermal.sensors.is_empty() {
        println!("\n── CPU Temperature ─────────────────────────────");
        for s in &thermal.sensors {
            println!("  {:<24} {:>6.1} °C", s.label, s.temp_celsius);
        }
    }

    Ok(())
}

// ── fsn sysinfo --refresh ─────────────────────────────────────────────────────

/// Clear the sysinfo cache and re-detect immediately.
pub async fn refresh() -> Result<()> {
    let cache = SysInfoCache::default_path();
    cache.clear()?;
    println!("Cache cleared — re-detecting …");
    let (os, features) = cache.get_or_detect();
    println!("  OS:       {} ({})", os.version, os.os_type.label());
    println!("  Features: {} available", features.available.len());
    println!("  Cache:    {}", cache.path().display());
    Ok(())
}

// ── fsn sysinfo --check <feature> ────────────────────────────────────────────

/// Check whether a single named feature is available on this system.
pub async fn check_feature(name: &str) -> Result<()> {
    let feature = Feature::from_str_loose(name).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown feature '{name}'. Valid values: \
             systemd, pam, launchd, windows-services, podman, docker, git, ssh, smartctl"
        )
    })?;

    if FeatureDetect::check(feature) {
        println!("✅ {} is available", feature.label());
    } else {
        println!("❌ {} is NOT available", feature.label());
    }
    Ok(())
}

// ── fsn sysinfo --monitor ─────────────────────────────────────────────────────

/// Run a continuous alert check loop, publishing to the bus if it is reachable.
///
/// Checks every `interval_secs` seconds.  Press Ctrl+C to stop.
pub async fn monitor(interval_secs: u64, thresholds: AlertThresholds) -> Result<()> {
    let checker = AlertChecker::new(thresholds);
    let interval = tokio::time::Duration::from_secs(interval_secs);

    println!("Monitoring system metrics (interval: {interval_secs}s) — Ctrl+C to stop");
    println!(
        "  Disk threshold:   {:.0}%",
        checker.thresholds.disk_full_percent
    );
    println!(
        "  Memory threshold: {:.0}%",
        checker.thresholds.memory_full_percent
    );
    println!(
        "  CPU threshold:    {:.0}°C",
        checker.thresholds.cpu_hot_celsius
    );

    loop {
        let alerts = checker.check_once();
        if alerts.is_empty() {
            tracing::debug!("sysinfo check: no alerts");
        }
        for alert in &alerts {
            let desc = alert.description();
            let topic = alert.bus_topic();
            tracing::warn!(topic, desc, "sysinfo alert");
            eprintln!("[ALERT] {topic}: {desc}");

            // Try to publish to bus via HTTP (non-fatal if bus is not running).
            publish_alert_to_bus(topic, &alert_payload(alert)).await;
        }
        tokio::time::sleep(interval).await;
    }
}

fn alert_payload(alert: &fs_sysinfo::SysInfoAlert) -> serde_json::Value {
    serde_json::to_value(alert).unwrap_or(serde_json::json!({}))
}

async fn publish_alert_to_bus(topic: &str, payload: &serde_json::Value) {
    let client = reqwest::Client::new();
    let _ = client
        .post("http://127.0.0.1:8081/api/bus/publish")
        .json(&serde_json::json!({
            "topic":   topic,
            "source":  "sysinfo",
            "payload": payload,
        }))
        .send()
        .await;
}
