use std::io::{self, Write};
use std::path::Path;
use anyhow::Result;

/// Interactive setup wizard – generates project.toml, host.toml, vault.toml skeletons.
pub async fn run(root: &Path) -> Result<()> {
    println!("=== FreeSynergy.Node Setup Wizard ===\n");

    let project_name = prompt("Project name")?;
    let domain       = prompt("Primary domain (e.g. example.com)")?;
    let contact      = prompt("Contact email")?;
    let host_ip      = prompt("Server IP address")?;
    let dns_provider = prompt_with_default("DNS provider [hetzner/cloudflare/none]", "hetzner")?;
    let acme         = prompt_with_default("ACME provider [letsencrypt/smallstep-ca/none]", "letsencrypt")?;

    // Create project directory
    let slug = project_name.to_lowercase().replace(' ', "-");
    let proj_dir = root.join("projects").join(&slug);
    std::fs::create_dir_all(&proj_dir)?;

    // Write project.toml
    let project_toml = format!(
        r#"[project]
name        = "{name}"
domain      = "{domain}"
description = ""

[project.contact]
email       = "{contact}"
acme_email  = "{contact}"

[load.modules]
# Example:
# [load.modules.forgejo]
# module_class = "git/forgejo"
"#,
        name    = project_name,
        domain  = domain,
        contact = contact,
    );
    std::fs::write(proj_dir.join(format!("{}.project.toml", slug)), &project_toml)?;

    // Write host.toml
    let host_toml = format!(
        r#"[host]
name = "{slug}"
ip   = "{ip}"

[proxy.zentinel]
module_class = "proxy/zentinel"

[proxy.zentinel.load.plugins]
dns        = "{dns}"
acme       = "{acme}"
acme_email = "{contact}"
"#,
        slug    = slug,
        ip      = host_ip,
        dns     = dns_provider,
        acme    = acme,
        contact = contact,
    );
    let hosts_dir = root.join("hosts");
    std::fs::create_dir_all(&hosts_dir)?;
    std::fs::write(hosts_dir.join(format!("{}.host.toml", slug)), &host_toml)?;

    // Write empty vault.toml
    std::fs::write(proj_dir.join("vault.toml"), "# Secrets (vault_ prefix required)\n")?;

    println!("\nCreated:");
    println!("  projects/{slug}/{slug}.project.toml");
    println!("  hosts/{slug}.host.toml");
    println!("  projects/{slug}/vault.toml");
    println!("\nNext: edit the config files, then run `fsn deploy`.");

    Ok(())
}

fn prompt(label: &str) -> Result<String> {
    print!("{}: ", label);
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_string())
}

fn prompt_with_default(label: &str, default: &str) -> Result<String> {
    print!("{}: ", label);
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(trimmed.to_string())
    }
}
