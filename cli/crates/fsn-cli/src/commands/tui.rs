// `fsn tui` — opens the FreeSynergy.Desktop (fsd binary must be installed).

use std::path::Path;
use anyhow::{bail, Result};

pub async fn run(_root: &Path) -> Result<()> {
    // Try to find `fsd` in PATH
    let fsd = which_fsd();

    if let Some(bin) = fsd {
        eprintln!("Starting FreeSynergy.Desktop ({})…", bin.display());
        let status = std::process::Command::new(&bin)
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to launch {}: {e}", bin.display()))?;

        if !status.success() {
            bail!("fsd exited with status {status}");
        }
        Ok(())
    } else {
        eprintln!("FreeSynergy.Desktop (fsd) not found in PATH.");
        eprintln!("Build it with:");
        eprintln!("  cd /home/kal/Server/FreeSynergy.Desktop");
        eprintln!("  cargo build -p fsd-app --release");
        eprintln!("  sudo cp target/release/fsd /usr/local/bin/fsd");
        bail!("fsd not installed")
    }
}

/// Find the `fsd` binary: check PATH, then common local build locations.
fn which_fsd() -> Option<std::path::PathBuf> {
    // Check PATH via `which` shell command
    if let Ok(out) = std::process::Command::new("which").arg("fsd").output() {
        if out.status.success() {
            let p = std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
            if p.exists() {
                return Some(p);
            }
        }
    }
    // Check well-known build locations
    let candidates = [
        "/usr/local/bin/fsd",
        "/home/kal/Server/FreeSynergy.Desktop/target/release/fsd",
        "/home/kal/Server/FreeSynergy.Desktop/target/debug/fsd",
    ];
    candidates.iter()
        .map(std::path::PathBuf::from)
        .find(|p| p.exists())
}
