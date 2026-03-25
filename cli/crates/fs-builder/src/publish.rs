//! `fs-builder publish` — sign a resource package and commit it to the Store.
//!
//! # Steps
//!
//! 1. Validate the resource (must be ✅ Ok to publish).
//! 2. Sign `resource.toml` with the node's Ed25519 key.
//! 3. Write the signature into `resource.toml` as `meta.signature`.
//! 4. `git add .` + `git commit` in a local clone of the Store.
//! 5. `git push` to the configured Store remote.

use anyhow::{bail, Context, Result};
use fs_types::resources::{
    container::ContainerResource, meta::ValidationStatus, validator::Validate,
};
use std::path::Path;

// ── Publisher ─────────────────────────────────────────────────────────────────

pub struct Publisher<'a> {
    path: &'a Path,
    store_remote: &'a str,
}

impl<'a> Publisher<'a> {
    pub fn new(path: &'a Path, store_remote: &'a str) -> Self {
        Self { path, store_remote }
    }

    pub fn run(&self) -> Result<()> {
        let toml_path = self.path.join("resource.toml");
        if !toml_path.exists() {
            bail!("No resource.toml found in {}", self.path.display());
        }

        let raw = std::fs::read_to_string(&toml_path)
            .with_context(|| format!("Cannot read {}", toml_path.display()))?;

        // Parse + validate
        let mut resource: ContainerResource = toml::from_str(&raw)
            .with_context(|| "Failed to parse resource.toml as ContainerResource")?;
        resource.validate();

        if resource.meta.status != ValidationStatus::Ok {
            bail!(
                "Resource is not valid ({:?}). Fix all issues before publishing.",
                resource.meta.status
            );
        }

        // Sign the resource content
        let signature = Self::sign_resource(&raw)?;
        resource.meta.signature = Some(signature);

        // Write the signed resource.toml back
        let signed_toml =
            toml::to_string_pretty(&resource).context("Failed to serialize signed resource")?;
        std::fs::write(&toml_path, &signed_toml)
            .with_context(|| format!("Cannot write {}", toml_path.display()))?;

        println!("✅ Signed resource.toml");

        // Git commit + push
        self.git_publish(&resource.meta.id)?;

        println!("✅ Published {} to {}", resource.meta.id, self.store_remote);
        Ok(())
    }

    // ── Signing ───────────────────────────────────────────────────────────────

    /// Sign the resource TOML content.
    ///
    /// Uses the node's Ed25519 signing key stored at `~/.config/fsn/signing.key`.
    /// If no key file exists, a new keypair is generated and saved.
    fn sign_resource(content: &str) -> Result<String> {
        use fs_crypto::signing::FsSigningKey;

        let key_path = Self::home_dir().join(".config/fsn/signing.key");
        let key = if key_path.exists() {
            let hex = std::fs::read_to_string(&key_path)
                .with_context(|| format!("Cannot read {}", key_path.display()))?;
            FsSigningKey::from_hex(hex.trim())
                .map_err(|e| anyhow::anyhow!("Invalid signing key: {e}"))?
        } else {
            let k = FsSigningKey::generate();
            if let Some(parent) = key_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&key_path, k.to_hex())?;
            println!("🔑 Generated new signing key at {}", key_path.display());
            k
        };

        let sig = key.sign(content.as_bytes());
        Ok(sig.to_hex())
    }

    // ── Git publish ───────────────────────────────────────────────────────────

    fn git_publish(&self, resource_id: &str) -> Result<()> {
        let store_dir = Self::home_dir().join(".local/share/fsn/store-clone");

        // Clone if not present.
        if !store_dir.join(".git").exists() {
            std::fs::create_dir_all(&store_dir).context("Cannot create store clone dir")?;
            Self::git_run(&store_dir, &["clone", self.store_remote, "."])?;
        } else {
            Self::git_run(&store_dir, &["pull", "--rebase"])?;
        }

        // Copy package into clone.
        let dest = store_dir.join("packages").join(resource_id);
        if dest.exists() {
            std::fs::remove_dir_all(&dest).context("Cannot remove old package dir")?;
        }
        Self::copy_dir_all(self.path, &dest)?;

        // Commit + push.
        Self::git_run(&store_dir, &["add", "."])?;
        let msg = format!("publish: {resource_id}");
        Self::git_run(&store_dir, &["commit", "-m", &msg])?;
        Self::git_run(&store_dir, &["push"])?;

        Ok(())
    }

    fn git_run(dir: &Path, args: &[&str]) -> Result<()> {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .context("Failed to run git")?;
        if !status.success() {
            bail!("git {} failed with status {}", args.join(" "), status);
        }
        Ok(())
    }

    fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let dst_path = dst.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                Self::copy_dir_all(&entry.path(), &dst_path)?;
            } else {
                std::fs::copy(entry.path(), dst_path)?;
            }
        }
        Ok(())
    }

    fn home_dir() -> std::path::PathBuf {
        std::env::var("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
    }
}

// ── Public shim ───────────────────────────────────────────────────────────────

pub fn run(path: &Path, store_remote: &str) -> Result<()> {
    Publisher::new(path, store_remote).run()
}
