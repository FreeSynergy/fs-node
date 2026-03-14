// `fsn tui` — opens the desktop UI (FreeSynergy.Desktop must be installed).

use std::path::Path;
use anyhow::Result;

pub async fn run(_root: &Path) -> Result<()> {
    eprintln!("The TUI is now part of FreeSynergy.Desktop.");
    eprintln!("Run `fsd` to open the desktop, or `fsd-conductor` for container management.");
    Ok(())
}
