// Distributed storage – inter-node S3 communication (D5).
//
// `FederatedS3Client` wraps an opendal `Operator` configured to talk to a
// remote FreeSynergy.Node's S3 endpoint.  Each node exposes the same S3 API
// (via `fsn-s3`), so nodes can replicate data between each other using the
// standard S3 protocol without any extra framing.
//
// Typical use:
//   – Pulling the public profile of a remote node (`profiles/` bucket)
//   – Syncing shared files from a parent node to a child node
//   – Replicating package cache between nodes in the same federation

use std::path::Path;

use anyhow::{Context, Result};
use opendal::Operator;

use crate::backend::SyncStats;
use crate::buckets::BucketKind;

// ── FederatedS3Client ─────────────────────────────────────────────────────────

pub struct FederatedS3Client {
    endpoint:   String,
    access_key: String,
    secret_key: String,
}

impl FederatedS3Client {
    /// Connect to a remote FSN node's S3 endpoint.
    ///
    /// `endpoint` must include scheme + host + port, e.g. `http://peer.example:9000`.
    pub fn new(
        endpoint: impl Into<String>,
        access_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        Self {
            endpoint:   endpoint.into(),
            access_key: access_key.into(),
            secret_key: secret_key.into(),
        }
    }

    /// Build an `opendal::Operator` targeting `bucket` on the remote node.
    pub fn operator(&self, bucket: BucketKind) -> Result<Operator> {
        self.build_operator(bucket.name())
    }

    // ── profile pull ──────────────────────────────────────────────────────────

    /// Fetch a remote node's public profile JSON.
    pub async fn fetch_profile(
        &self,
        remote_node_id: &str,
    ) -> Result<crate::profile::NodeProfile> {
        let op = self.operator(BucketKind::Profiles)?;
        let key = format!("{remote_node_id}/profile.json");
        let data = op.read(&key).await
            .with_context(|| format!("fetch profile {key} from {}", self.endpoint))?
            .to_bytes();
        serde_json::from_slice(&data).context("invalid remote profile.json")
    }

    /// Download the avatar for a remote node into `dest_path`.
    pub async fn fetch_avatar(
        &self,
        remote_node_id: &str,
        dest_path: &Path,
    ) -> Result<()> {
        let op = self.operator(BucketKind::Profiles)?;
        for ext in ["png", "jpg", "jpeg", "webp"] {
            let key = format!("{remote_node_id}/avatar.{ext}");
            match op.read(&key).await {
                Ok(buf) => {
                    let dest = dest_path.join(format!("avatar.{ext}"));
                    tokio::fs::write(&dest, buf.to_bytes()).await
                        .with_context(|| format!("write {}", dest.display()))?;
                    tracing::debug!("fetched avatar {key} → {}", dest.display());
                    return Ok(());
                }
                Err(_) => continue,
            }
        }
        anyhow::bail!("no avatar found for node {remote_node_id} at {}", self.endpoint)
    }

    // ── bucket sync ───────────────────────────────────────────────────────────

    /// Pull all objects from a remote bucket into a local directory.
    pub async fn pull_bucket(
        &self,
        bucket: BucketKind,
        local_dest: &Path,
    ) -> Result<SyncStats> {
        let op = self.operator(bucket)?;
        sync_remote_to_local(&op, "/", local_dest).await
    }

    /// Push all local files into a remote bucket.
    pub async fn push_bucket(
        &self,
        bucket: BucketKind,
        local_src: &Path,
    ) -> Result<SyncStats> {
        let op = self.operator(bucket)?;
        sync_local_to_remote(local_src, "/", &op).await
    }

    // ── internals ─────────────────────────────────────────────────────────────

    #[cfg(feature = "backend-hetzner")]
    fn build_operator(&self, bucket: &str) -> Result<Operator> {
        let builder = opendal::services::S3::default()
            .endpoint(&self.endpoint)
            .bucket(bucket)
            .region("fsn")  // pseudo-region required by S3 spec
            .access_key_id(&self.access_key)
            .secret_access_key(&self.secret_key)
            .enable_virtual_host_style(); // FSN nodes use path-style → disable vhost
        Ok(Operator::new(builder)?.finish())
    }

    #[cfg(not(feature = "backend-hetzner"))]
    fn build_operator(&self, _bucket: &str) -> Result<Operator> {
        // Without the backend-hetzner feature, fall back to a no-op local operator.
        // Network replication requires `--features backend-hetzner`.
        tracing::warn!(
            "FederatedS3Client: feature 'backend-hetzner' not enabled; \
             federation commands will use a no-op local operator"
        );
        let builder = opendal::services::Fs::default().root("/tmp/fsn-fed-dummy");
        Ok(Operator::new(builder)?.finish())
    }
}

// ── sync helpers ──────────────────────────────────────────────────────────────

async fn sync_remote_to_local(op: &Operator, prefix: &str, dest: &Path) -> Result<SyncStats> {
    let mut stats = SyncStats::default();
    tokio::fs::create_dir_all(dest).await?;

    let entries = op.list(prefix).await
        .with_context(|| format!("list remote prefix {prefix}"))?;

    for entry in entries {
        if entry.metadata().is_file() {
            let data = op.read(entry.path()).await
                .with_context(|| format!("read {}", entry.path()))?
                .to_bytes();
            let local_path = dest.join(entry.name());
            let len = data.len() as u64;
            tokio::fs::write(&local_path, &data).await?;
            stats.files_downloaded += 1;
            stats.bytes_transferred += len;
        }
    }
    Ok(stats)
}

async fn sync_local_to_remote(src: &Path, prefix: &str, op: &Operator) -> Result<SyncStats> {
    let mut stats = SyncStats::default();
    let mut rd = tokio::fs::read_dir(src).await?;
    while let Some(entry) = rd.next_entry().await? {
        if entry.file_type().await?.is_file() {
            let data = tokio::fs::read(entry.path()).await?;
            let key = format!(
                "{}/{}",
                prefix.trim_end_matches('/'),
                entry.file_name().to_string_lossy()
            );
            let len = data.len() as u64;
            op.write(&key, data).await
                .with_context(|| format!("write {key}"))?;
            stats.files_uploaded += 1;
            stats.bytes_transferred += len;
        }
    }
    Ok(stats)
}
