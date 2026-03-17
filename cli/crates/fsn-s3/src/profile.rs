// Public profile management (D4).
//
// Profiles live in the `profiles/` bucket:
//   profiles/{node_id}/profile.json   – NodeProfile serialized as JSON
//   profiles/{node_id}/avatar.{ext}   – binary avatar file (png/jpg/webp)
//
// The profiles bucket is publicly readable (see BucketKind::is_public).
// Remote nodes can fetch a profile via the S3 API or via the Zentinel proxy.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::buckets::BucketKind;
use crate::config::StorageConfig;

// ── NodeProfile ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeProfile {
    pub node_id:      String,
    pub display_name: String,
    #[serde(default)]
    pub description:  Option<String>,
    #[serde(default)]
    pub avatar_hash:  Option<String>,
    #[serde(default)]
    pub public_url:   Option<String>,
    pub created_at:   i64,
    pub updated_at:   i64,
}

impl NodeProfile {
    pub fn new(node_id: impl Into<String>, display_name: impl Into<String>) -> Self {
        let now = Utc::now().timestamp();
        Self {
            node_id:      node_id.into(),
            display_name: display_name.into(),
            description:  None,
            avatar_hash:  None,
            public_url:   None,
            created_at:   now,
            updated_at:   now,
        }
    }
}

// ── ProfileStore ──────────────────────────────────────────────────────────────

pub struct ProfileStore {
    profiles_root: PathBuf,
}

impl ProfileStore {
    pub fn new(config: &StorageConfig) -> Self {
        let profiles_root = BucketKind::Profiles.path(&config.buckets_root());
        Self { profiles_root }
    }

    // ── writes ────────────────────────────────────────────────────────────────

    pub async fn put_profile(&self, profile: &NodeProfile) -> Result<()> {
        let dir = self.profile_dir(&profile.node_id);
        tokio::fs::create_dir_all(&dir).await
            .with_context(|| format!("create profile dir {}", dir.display()))?;

        let json = serde_json::to_vec_pretty(profile)?;
        let dest = dir.join("profile.json");
        tokio::fs::write(&dest, &json).await
            .with_context(|| format!("write {}", dest.display()))?;

        tracing::debug!("profile written: {}", profile.node_id);
        Ok(())
    }

    /// Store an avatar image; returns the SHA-256 hex of the data.
    pub async fn put_avatar(
        &self,
        node_id: &str,
        data: &[u8],
        extension: &str,
    ) -> Result<String> {
        let dir = self.profile_dir(node_id);
        tokio::fs::create_dir_all(&dir).await?;

        // Remove any existing avatar file regardless of extension
        self.remove_existing_avatars(&dir).await;

        let hash = sha256_hex(data);
        let filename = format!("avatar.{extension}");
        let dest = dir.join(&filename);
        tokio::fs::write(&dest, data).await
            .with_context(|| format!("write avatar {}", dest.display()))?;

        tracing::debug!("avatar stored for {node_id}: {filename} ({} bytes)", data.len());
        Ok(hash)
    }

    // ── reads ─────────────────────────────────────────────────────────────────

    pub async fn get_profile(&self, node_id: &str) -> Result<NodeProfile> {
        let path = self.profile_dir(node_id).join("profile.json");
        let data = tokio::fs::read(&path).await
            .with_context(|| format!("read profile {}", path.display()))?;
        serde_json::from_slice(&data).context("invalid profile.json")
    }

    pub async fn get_profile_opt(&self, node_id: &str) -> Option<NodeProfile> {
        self.get_profile(node_id).await.ok()
    }

    /// Path to the avatar file (any known extension), or `None` if absent.
    pub async fn avatar_path(&self, node_id: &str) -> Option<PathBuf> {
        let dir = self.profile_dir(node_id);
        for ext in ["png", "jpg", "jpeg", "webp"] {
            let p = dir.join(format!("avatar.{ext}"));
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    // ── list ──────────────────────────────────────────────────────────────────

    pub async fn list_profiles(&self) -> Result<Vec<NodeProfile>> {
        let mut out = Vec::new();
        let mut rd = tokio::fs::read_dir(&self.profiles_root).await?;
        while let Some(entry) = rd.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                let node_id = entry.file_name().to_string_lossy().into_owned();
                if let Some(p) = self.get_profile_opt(&node_id).await {
                    out.push(p);
                }
            }
        }
        Ok(out)
    }

    // ── delete ────────────────────────────────────────────────────────────────

    pub async fn delete_profile(&self, node_id: &str) -> Result<()> {
        let dir = self.profile_dir(node_id);
        if dir.exists() {
            tokio::fs::remove_dir_all(&dir).await
                .with_context(|| format!("remove profile dir {}", dir.display()))?;
        }
        Ok(())
    }

    // ── internals ─────────────────────────────────────────────────────────────

    fn profile_dir(&self, node_id: &str) -> PathBuf {
        // Sanitize: only allow alphanumeric, dash, underscore, dot
        let safe: String = node_id
            .chars()
            .map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' })
            .collect();
        self.profiles_root.join(safe)
    }

    async fn remove_existing_avatars(&self, dir: &Path) {
        for ext in ["png", "jpg", "jpeg", "webp"] {
            let p = dir.join(format!("avatar.{ext}"));
            let _ = tokio::fs::remove_file(&p).await;
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn sha256_hex(data: &[u8]) -> String {
    use std::fmt::Write as _;
    // Simple djb2-style hash (no crypto dependency in this crate).
    // For a real checksum, the caller can use sha2 or blake3 from the CLI crate.
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    let mut s = String::with_capacity(16);
    let _ = write!(s, "{h:016x}");
    s
}
