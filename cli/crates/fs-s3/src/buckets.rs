// Bucket structure and initialization.
//
// Five canonical buckets:
//   profiles/   – public node profiles (JSON + avatar), readable by remote nodes
//   backups/    – private backups, local-only
//   media/      – service media files (scoped per service)
//   packages/   – Store package cache
//   shared/     – files shared between node users

use std::path::{Path, PathBuf};

use anyhow::Result;

// ── BucketKind ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BucketKind {
    Profiles,
    Backups,
    Media,
    Packages,
    Shared,
}

impl BucketKind {
    pub fn name(self) -> &'static str {
        match self {
            BucketKind::Profiles => "profiles",
            BucketKind::Backups => "backups",
            BucketKind::Media => "media",
            BucketKind::Packages => "packages",
            BucketKind::Shared => "shared",
        }
    }

    /// Whether remote nodes may read from this bucket without auth.
    pub fn is_public(self) -> bool {
        matches!(self, BucketKind::Profiles)
    }

    pub fn all() -> &'static [BucketKind] {
        &[
            BucketKind::Profiles,
            BucketKind::Backups,
            BucketKind::Media,
            BucketKind::Packages,
            BucketKind::Shared,
        ]
    }

    pub fn path(self, root: &Path) -> PathBuf {
        root.join(self.name())
    }
}

// ── Bucket info ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BucketInfo {
    pub kind: BucketKind,
    pub path: PathBuf,
    pub is_public: bool,
    pub object_count: u64,
    pub size_bytes: u64,
}

impl BucketInfo {
    pub fn collect(kind: BucketKind, root: &Path) -> BucketInfo {
        let path = kind.path(root);
        let (object_count, size_bytes) = du(&path);
        BucketInfo {
            kind,
            path,
            is_public: kind.is_public(),
            object_count,
            size_bytes,
        }
    }
}

// ── initialization ────────────────────────────────────────────────────────────

/// Create all bucket directories under `root` (idempotent).
pub async fn ensure_buckets(root: &Path) -> Result<()> {
    for kind in BucketKind::all() {
        let dir = kind.path(root);
        tokio::fs::create_dir_all(&dir).await?;
        tracing::debug!("bucket ready: {}", dir.display());
    }
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Recursively count objects and sum sizes (best-effort, ignores errors).
fn du(path: &Path) -> (u64, u64) {
    let mut count = 0u64;
    let mut bytes = 0u64;
    if let Ok(rd) = std::fs::read_dir(path) {
        for entry in rd.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    count += 1;
                    bytes += meta.len();
                } else if meta.is_dir() {
                    let (c, b) = du(&entry.path());
                    count += c;
                    bytes += b;
                }
            }
        }
    }
    (count, bytes)
}
