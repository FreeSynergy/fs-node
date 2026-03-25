// `fsn tui` — launches FreeSynergy.Desktop (fsd / fs-container-app).
//
// Search order:
//   1. `fs-container-app` in PATH  — standalone container app manager binary (if built separately)
//   2. `fsd` in PATH            — full Desktop shell (includes container app manager as a window)
//   3. Well-known build locations for both binaries
//
// fs-container-app is the container management app (service list, logs, health status).
// fsd is the full Desktop shell that hosts container app manager and other apps.

use anyhow::{bail, Result};
use std::path::Path;

pub async fn run(_root: &Path) -> Result<()> {
    if let Some(bin) = which_desktop_bin() {
        eprintln!("Starting FreeSynergy.Desktop ({})…", bin.display());
        let status = std::process::Command::new(&bin)
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to launch {}: {e}", bin.display()))?;

        if !status.success() {
            bail!("{} exited with status {status}", bin.display());
        }
        Ok(())
    } else {
        eprintln!("FreeSynergy.Desktop (fsd / fs-container-app) not found in PATH.");
        eprintln!("Build it with:");
        eprintln!("  cargo build -p fs-app --release   (in the fs-desktop repo)");
        eprintln!("  sudo cp target/release/fsd /usr/local/bin/fsd");
        eprintln!("Or set FS_DESKTOP_DIR=/path/to/fs-desktop for local build fallback.");
        bail!("fsd not installed")
    }
}

/// Find the best available Desktop binary.
///
/// Prefers `fs-container-app` (standalone container app manager mode) over the full `fsd`
/// shell, so that `fsn tui` opens container management directly.
/// Falls back to `fsd` if container app manager is not separately installed.
fn which_desktop_bin() -> Option<std::path::PathBuf> {
    // Check PATH for both candidates (container app manager first)
    for name in &["fs-container-app", "fsd"] {
        if let Ok(out) = std::process::Command::new("which").arg(name).output() {
            if out.status.success() {
                let p = std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }

    // Check FS_DESKTOP_DIR env var for local build fallback (development only).
    // Set FS_DESKTOP_DIR to the root of the fs-desktop repo to enable this.
    if let Ok(desktop_dir) = std::env::var("FS_DESKTOP_DIR") {
        let base = format!("{desktop_dir}/target");
        let candidates = [
            format!("{base}/release/fs-container-app"),
            format!("{base}/debug/fs-container-app"),
            format!("{base}/release/fsd"),
            format!("{base}/debug/fsd"),
        ];
        if let Some(p) = candidates
            .iter()
            .map(std::path::PathBuf::from)
            .find(|p| p.exists())
        {
            return Some(p);
        }
    }

    None
}
