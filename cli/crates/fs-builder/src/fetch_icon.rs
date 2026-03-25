// fetch-icon — download an SVG icon and store it in the Store repo.
//
// Supported sources:
//   homarr:<name>   → Homarr Dashboard Icons (MIT)
//   simple:<name>   → Simple Icons (CC0)
//   <https://...>   → Any HTTPS URL (license must be verified manually)

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

const HOMARR_BASE: &str = "https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/svg";
const SIMPLE_BASE: &str = "https://cdn.simpleicons.org";

/// Resolve a source specifier to a download URL.
fn resolve_url(source: &str) -> Result<String> {
    if let Some(name) = source.strip_prefix("homarr:") {
        Ok(format!("{HOMARR_BASE}/{name}.svg"))
    } else if let Some(name) = source.strip_prefix("simple:") {
        Ok(format!("{SIMPLE_BASE}/{name}"))
    } else if source.starts_with("https://") || source.starts_with("http://") {
        Ok(source.to_string())
    } else {
        bail!(
            "unknown icon source '{source}'. \
             Use 'homarr:<name>', 'simple:<name>', or a full https:// URL"
        )
    }
}

/// Download the SVG and write it to `output`. Returns the output path.
async fn download_svg(url: &str, output: &Path) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("fs-builder/0.1 (FreeSynergy icon fetcher)")
        .build()
        .context("build reqwest client")?;

    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;

    let status = response.status();
    if !status.is_success() {
        bail!("HTTP {status} for {url}");
    }

    let bytes = response.bytes().await.context("read response body")?;

    // Basic check: SVG files start with '<'
    let text = std::str::from_utf8(&bytes).context("icon is not valid UTF-8")?;
    if !text.trim_start().starts_with('<') {
        bail!("response does not look like SVG (does not start with '<')");
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create directory {}", parent.display()))?;
    }

    std::fs::write(output, &bytes).with_context(|| format!("write {}", output.display()))?;

    Ok(())
}

/// Main entry point for `fs-builder fetch-icon`.
pub async fn run(source: &str, name: &str, store_dir: &Path) -> Result<()> {
    let url = resolve_url(source)?;
    let output: PathBuf = store_dir
        .join("shared")
        .join("icons")
        .join(format!("{name}.svg"));

    println!("Downloading icon '{name}' from {url}");
    download_svg(&url, &output).await?;
    println!("Saved to {}", output.display());
    println!();
    println!("Add to your manifest:");
    println!("  icon = \"shared/icons/{name}.svg\"");

    Ok(())
}
