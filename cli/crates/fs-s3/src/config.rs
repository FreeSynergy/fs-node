// S3 storage server configuration.
//
// Deserialized from the [storage] section of the node config TOML.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ── top-level ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Enable the embedded S3 server (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// TCP port for the S3 API (default: 9000)
    #[serde(default = "default_s3_port")]
    pub port: u16,

    /// Bind address (default: "127.0.0.1")
    #[serde(default = "default_bind")]
    pub bind: String,

    /// Root directory for all bucket data
    pub data_root: PathBuf,

    /// S3 access key (used by local clients and remote nodes)
    pub access_key: String,

    /// S3 secret key
    pub secret_key: String,

    /// Optional replication/sync backend
    #[serde(default)]
    pub sync: Option<SyncConfig>,
}

impl StorageConfig {
    pub fn s3_endpoint(&self) -> String {
        format!("http://{}:{}", self.bind, self.port)
    }

    pub fn buckets_root(&self) -> PathBuf {
        self.data_root.clone()
    }
}

// ── defaults ──────────────────────────────────────────────────────────────────

fn default_true() -> bool {
    true
}
fn default_s3_port() -> u16 {
    9000
}
fn default_bind() -> String {
    "127.0.0.1".to_owned()
}

// ── sync / replication ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub backend: SyncBackendKind,
    #[serde(default)]
    pub sftp: Option<SftpConfig>,
    #[serde(default)]
    pub hetzner: Option<HetznerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SyncBackendKind {
    Sftp,
    Hetzner,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftpConfig {
    pub host: String,
    #[serde(default = "default_sftp_port")]
    pub port: u16,
    pub user: String,
    pub key_path: PathBuf,
    pub root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HetznerConfig {
    pub bucket: String,
    pub region: String,
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
}

fn default_sftp_port() -> u16 {
    22
}
