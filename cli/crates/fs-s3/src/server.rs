// Embedded S3-compatible server (D1).
//
// Uses `s3s` (S3 API middleware) + `s3s_fs` (local-filesystem backend).
// Runs in its own tokio task alongside the main axum HTTP server.
//
// The server listens on `config.bind:config.port` (default 127.0.0.1:9000).
// On startup, all bucket directories are created (idempotent).

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::buckets;
use crate::config::StorageConfig;

// ── S3Server ──────────────────────────────────────────────────────────────────

pub struct S3Server {
    config: Arc<StorageConfig>,
}

impl S3Server {
    pub fn new(config: StorageConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Ensure bucket directories exist and start the S3 server in a background task.
    pub async fn start(&self) -> Result<JoinHandle<()>> {
        buckets::ensure_buckets(&self.config.buckets_root())
            .await
            .context("failed to initialize S3 bucket directories")?;

        let config = Arc::clone(&self.config);
        let handle = tokio::spawn(async move {
            if let Err(e) = serve(config).await {
                error!("S3 server error: {e:#}");
            }
        });

        info!(
            "S3 server starting on {}:{}",
            self.config.bind, self.config.port
        );
        Ok(handle)
    }
}

// ── inner server loop ─────────────────────────────────────────────────────────

#[allow(clippy::cognitive_complexity)]
async fn serve(config: Arc<StorageConfig>) -> Result<()> {
    let root = config.buckets_root();

    // Local filesystem backend
    let fs = s3s_fs::FileSystem::new(&root)
        .map_err(|e| anyhow::anyhow!("failed to open S3 filesystem backend: {e:?}"))?;

    // Auth: single access-key / secret-key pair
    let auth =
        s3s::auth::SimpleAuth::from_single(config.access_key.as_str(), config.secret_key.as_str());

    // Build the s3s service
    let mut builder = s3s::service::S3ServiceBuilder::new(fs);
    builder.set_auth(auth);
    let service = builder.build();

    let addr: SocketAddr = format!("{}:{}", config.bind, config.port)
        .parse()
        .context("invalid S3 bind address")?;

    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("cannot bind S3 server to {addr}"))?;

    info!("S3 API ready at http://{addr}");

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                error!("S3 accept error: {e}");
                continue;
            }
        };

        tracing::trace!("S3 connection from {peer}");

        let svc = service.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            if let Err(e) = http1::Builder::new().serve_connection(io, svc).await {
                tracing::debug!("S3 connection closed: {e}");
            }
        });
    }
}
