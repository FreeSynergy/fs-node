// opendal-based storage backends for replication / sync (D2).
//
// The primary storage path always uses s3s-fs (local filesystem).
// opendal is used for off-site replication to:
//   - SFTP servers       (feature: backend-sftp)
//   - Hetzner Storagebox (feature: backend-hetzner, uses S3-compatible API)
//
// The `SyncBackend` trait abstracts over both, providing a uniform
// `upload` / `download` API used by `FederatedS3Client`.

use std::path::Path;

use anyhow::{Context, Result};
use opendal::Operator;

use crate::config::{StorageConfig, SyncBackendKind};

// ── SyncBackend ───────────────────────────────────────────────────────────────

pub trait SyncBackend: Send + Sync {
    /// Upload all files from `local_path` into `remote_prefix` on the backend.
    fn upload<'a>(
        &'a self,
        local_path: &'a Path,
        remote_prefix: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<SyncStats>> + Send + 'a>>;

    /// Download all objects under `remote_prefix` into `local_path`.
    fn download<'a>(
        &'a self,
        remote_prefix: &'a str,
        local_path: &'a Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<SyncStats>> + Send + 'a>>;
}

#[derive(Debug, Default)]
pub struct SyncStats {
    pub files_uploaded:    u64,
    pub files_downloaded:  u64,
    pub bytes_transferred: u64,
}

// ── factory ───────────────────────────────────────────────────────────────────

/// Build the configured sync backend from the node config.
/// Returns `None` if sync is disabled or not configured.
pub fn build(config: &StorageConfig) -> Option<Box<dyn SyncBackend>> {
    let sync = config.sync.as_ref()?;
    match sync.backend {
        SyncBackendKind::None    => None,
        SyncBackendKind::Sftp    => build_sftp(config),
        SyncBackendKind::Hetzner => build_hetzner(config),
    }
}

// ── local (opendal Fs) ────────────────────────────────────────────────────────

pub struct LocalBackend {
    op: Operator,
}

impl LocalBackend {
    pub fn new(root: &Path) -> Result<Self> {
        let builder = opendal::services::Fs::default()
            .root(&root.to_string_lossy());
        let op = Operator::new(builder)?.finish();
        Ok(Self { op })
    }
}

impl SyncBackend for LocalBackend {
    fn upload<'a>(
        &'a self,
        local_path: &'a Path,
        remote_prefix: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<SyncStats>> + Send + 'a>> {
        Box::pin(sync_dir_up(&self.op, local_path, remote_prefix))
    }

    fn download<'a>(
        &'a self,
        remote_prefix: &'a str,
        local_path: &'a Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<SyncStats>> + Send + 'a>> {
        Box::pin(sync_dir_down(&self.op, remote_prefix, local_path))
    }
}

// ── SFTP (opendal Sftp, feature: backend-sftp) ────────────────────────────────

fn build_sftp(config: &StorageConfig) -> Option<Box<dyn SyncBackend>> {
    #[cfg(feature = "backend-sftp")]
    {
        let sftp = config.sync.as_ref()?.sftp.as_ref()?;
        let builder = opendal::services::Sftp::default()
            .endpoint(&format!("ssh://{}:{}", sftp.host, sftp.port))
            .user(&sftp.user)
            .key(&sftp.key_path.to_string_lossy())
            .root(&sftp.root);
        match Operator::new(builder).map(|b| b.finish()) {
            Ok(op) => Some(Box::new(OdalBackend { op })),
            Err(e) => {
                tracing::error!("Failed to build SFTP sync backend: {e}");
                None
            }
        }
    }
    #[cfg(not(feature = "backend-sftp"))]
    {
        let _ = config;
        tracing::warn!("SFTP sync backend requested but feature 'backend-sftp' not enabled");
        None
    }
}

// ── Hetzner / S3-compatible (opendal S3, feature: backend-hetzner) ───────────

fn build_hetzner(config: &StorageConfig) -> Option<Box<dyn SyncBackend>> {
    #[cfg(feature = "backend-hetzner")]
    {
        let h = config.sync.as_ref()?.hetzner.as_ref()?;
        let builder = opendal::services::S3::default()
            .bucket(&h.bucket)
            .region(&h.region)
            .endpoint(&h.endpoint)
            .access_key_id(&h.access_key)
            .secret_access_key(&h.secret_key);
        match Operator::new(builder).map(|b| b.finish()) {
            Ok(op) => Some(Box::new(OdalBackend { op })),
            Err(e) => {
                tracing::error!("Failed to build Hetzner sync backend: {e}");
                None
            }
        }
    }
    #[cfg(not(feature = "backend-hetzner"))]
    {
        let _ = config;
        tracing::warn!("Hetzner sync backend requested but feature 'backend-hetzner' not enabled");
        None
    }
}

// ── generic opendal backend ───────────────────────────────────────────────────

struct OdalBackend {
    op: Operator,
}

impl SyncBackend for OdalBackend {
    fn upload<'a>(
        &'a self,
        local_path: &'a Path,
        remote_prefix: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<SyncStats>> + Send + 'a>> {
        Box::pin(sync_dir_up(&self.op, local_path, remote_prefix))
    }

    fn download<'a>(
        &'a self,
        remote_prefix: &'a str,
        local_path: &'a Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<SyncStats>> + Send + 'a>> {
        Box::pin(sync_dir_down(&self.op, remote_prefix, local_path))
    }
}

// ── sync helpers ──────────────────────────────────────────────────────────────

async fn sync_dir_up(op: &Operator, local_path: &Path, remote_prefix: &str) -> Result<SyncStats> {
    let mut stats = SyncStats::default();
    let mut entries = tokio::fs::read_dir(local_path).await
        .with_context(|| format!("read_dir {}", local_path.display()))?;

    while let Some(entry) = entries.next_entry().await? {
        let file_type = entry.file_type().await?;
        if file_type.is_file() {
            let data = tokio::fs::read(entry.path()).await?;
            let remote_key = format!(
                "{}/{}",
                remote_prefix.trim_end_matches('/'),
                entry.file_name().to_string_lossy()
            );
            let len = data.len() as u64;
            op.write(&remote_key, data).await
                .with_context(|| format!("upload {remote_key}"))?;
            stats.files_uploaded += 1;
            stats.bytes_transferred += len;
        } else if file_type.is_dir() {
            let sub_prefix = format!(
                "{}/{}",
                remote_prefix.trim_end_matches('/'),
                entry.file_name().to_string_lossy()
            );
            let sub_stats = Box::pin(sync_dir_up(op, &entry.path(), &sub_prefix)).await?;
            stats.files_uploaded    += sub_stats.files_uploaded;
            stats.bytes_transferred += sub_stats.bytes_transferred;
        }
    }
    Ok(stats)
}

async fn sync_dir_down(op: &Operator, remote_prefix: &str, local_path: &Path) -> Result<SyncStats> {
    let mut stats = SyncStats::default();
    let prefix = format!("{}/", remote_prefix.trim_end_matches('/'));
    let entries = op.list(&prefix).await
        .with_context(|| format!("list {prefix}"))?;

    tokio::fs::create_dir_all(local_path).await?;

    for entry in entries {
        if entry.metadata().is_file() {
            let data = op.read(entry.path()).await
                .with_context(|| format!("download {}", entry.path()))?
                .to_bytes();
            let file_name = entry.name();
            let dest = local_path.join(file_name);
            let len = data.len() as u64;
            tokio::fs::write(&dest, &data).await
                .with_context(|| format!("write {}", dest.display()))?;
            stats.files_downloaded += 1;
            stats.bytes_transferred += len;
        }
    }
    Ok(stats)
}
