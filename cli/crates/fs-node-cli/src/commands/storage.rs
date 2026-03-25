// `fsn storage` – S3 storage management commands.
//
// Subcommands:
//   status                – show bucket sizes and backend info
//   init                  – initialize bucket structure
//   profile show          – print the local node's public profile
//   profile set           – update display name / description / public URL
//   profile avatar <file> – upload a new avatar image
//   sync pull <url>       – pull from a remote node (federation)
//   sync push <url>       – push to a remote node (federation)

use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use fs_s3::{
    buckets::BucketInfo, BucketKind, FederatedS3Client, NodeProfile, ProfileStore, StorageConfig,
};

use crate::cli::{ProfileCommand, StorageSyncCommand};

// ── default config helper ─────────────────────────────────────────────────────

fn default_config(root: &Path) -> StorageConfig {
    StorageConfig {
        enabled: true,
        port: 9000,
        bind: "127.0.0.1".to_owned(),
        data_root: root.join("storage"),
        access_key: "fs_local".to_owned(),
        secret_key: "changeme_secret_key".to_owned(),
        sync: None,
    }
}

// ── status ────────────────────────────────────────────────────────────────────

pub async fn status(root: &Path) -> Result<()> {
    let config = default_config(root);

    println!("FSN S3 Storage Status");
    println!("  Endpoint : http://{}:{}", config.bind, config.port);
    println!("  Data root: {}", config.data_root.display());
    println!();

    for kind in BucketKind::all() {
        let info = BucketInfo::collect(*kind, &config.buckets_root());
        let public = if info.is_public { " (public)" } else { "" };
        println!(
            "  {:10}  {:>6} objects  {:>9}{}",
            info.kind.name(),
            info.object_count,
            human_bytes(info.size_bytes),
            public,
        );
    }
    Ok(())
}

// ── init ──────────────────────────────────────────────────────────────────────

pub async fn init(root: &Path) -> Result<()> {
    let config = default_config(root);
    fs_s3::ensure_buckets(&config.buckets_root()).await?;
    println!(
        "Storage buckets initialized at {}",
        config.data_root.display()
    );
    for kind in BucketKind::all() {
        println!("  ✓  {}", kind.name());
    }
    Ok(())
}

// ── profile ───────────────────────────────────────────────────────────────────

pub async fn profile(root: &Path, cmd: ProfileCommand) -> Result<()> {
    let config = default_config(root);
    let store = ProfileStore::new(&config);

    match cmd {
        ProfileCommand::Show => match store.get_profile_opt("local").await {
            Some(p) => {
                println!("Node ID     : {}", p.node_id);
                println!("Name        : {}", p.display_name);
                if let Some(d) = &p.description {
                    println!("Description : {d}");
                }
                if let Some(u) = &p.public_url {
                    println!("Public URL  : {u}");
                }
                if let Some(h) = &p.avatar_hash {
                    println!("Avatar hash : {h}");
                }
            }
            None => println!("No profile set. Run `fsn storage profile set --name <name>`."),
        },

        ProfileCommand::Set {
            name,
            description,
            public_url,
        } => {
            let mut p = store
                .get_profile_opt("local")
                .await
                .unwrap_or_else(|| NodeProfile::new("local", &name));

            p.display_name = name;
            p.updated_at = Utc::now().timestamp();
            if let Some(d) = description {
                p.description = Some(d);
            }
            if let Some(u) = public_url {
                p.public_url = Some(u);
            }

            fs_s3::ensure_buckets(&config.buckets_root()).await?;
            store.put_profile(&p).await?;
            println!("Profile updated.");
        }

        ProfileCommand::Avatar { file } => {
            let ext = file
                .extension()
                .and_then(|e| e.to_str())
                .context("avatar file must have an extension (png/jpg/webp)")?
                .to_lowercase();

            anyhow::ensure!(
                matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp"),
                "unsupported avatar format '{ext}' — use png, jpg, or webp"
            );

            let data = tokio::fs::read(&file)
                .await
                .with_context(|| format!("read avatar file {}", file.display()))?;

            fs_s3::ensure_buckets(&config.buckets_root()).await?;
            let hash = store.put_avatar("local", &data, &ext).await?;
            println!("Avatar uploaded ({} bytes, hash: {hash})", data.len());
        }
    }
    Ok(())
}

// ── sync ──────────────────────────────────────────────────────────────────────

pub async fn sync(root: &Path, cmd: StorageSyncCommand) -> Result<()> {
    let config = default_config(root);

    match cmd {
        StorageSyncCommand::Pull {
            remote_url,
            bucket,
            access_key,
            secret_key,
        } => {
            let bucket_kind = parse_bucket(&bucket)?;
            let client = FederatedS3Client::new(&remote_url, &access_key, &secret_key);
            let local_dest = bucket_kind.path(&config.buckets_root());

            println!(
                "Pulling {bucket} from {remote_url} → {}",
                local_dest.display()
            );
            fs_s3::ensure_buckets(&config.buckets_root()).await?;

            let stats = client.pull_bucket(bucket_kind, &local_dest).await?;
            println!(
                "Done: {} files, {}",
                stats.files_downloaded,
                human_bytes(stats.bytes_transferred)
            );
        }

        StorageSyncCommand::Push {
            remote_url,
            bucket,
            access_key,
            secret_key,
        } => {
            let bucket_kind = parse_bucket(&bucket)?;
            let client = FederatedS3Client::new(&remote_url, &access_key, &secret_key);
            let local_src = bucket_kind.path(&config.buckets_root());

            println!("Pushing {bucket} to {remote_url}");
            let stats = client.push_bucket(bucket_kind, &local_src).await?;
            println!(
                "Done: {} files, {}",
                stats.files_uploaded,
                human_bytes(stats.bytes_transferred)
            );
        }

        StorageSyncCommand::FetchProfile {
            remote_url,
            node_id,
            access_key,
            secret_key,
        } => {
            let client = FederatedS3Client::new(&remote_url, &access_key, &secret_key);
            let p = client.fetch_profile(&node_id).await?;
            println!("Remote profile for '{node_id}':");
            println!("  Name        : {}", p.display_name);
            if let Some(d) = &p.description {
                println!("  Description : {d}");
            }
            if let Some(u) = &p.public_url {
                println!("  Public URL  : {u}");
            }
        }
    }
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn parse_bucket(name: &str) -> Result<BucketKind> {
    match name {
        "profiles" => Ok(BucketKind::Profiles),
        "backups" => Ok(BucketKind::Backups),
        "media" => Ok(BucketKind::Media),
        "packages" => Ok(BucketKind::Packages),
        "shared" => Ok(BucketKind::Shared),
        other => anyhow::bail!(
            "unknown bucket '{other}' — valid: profiles, backups, media, packages, shared"
        ),
    }
}

fn human_bytes(b: u64) -> String {
    const K: u64 = 1024;
    const M: u64 = K * 1024;
    const G: u64 = M * 1024;
    if b >= G {
        format!("{:.1} GiB", b as f64 / G as f64)
    } else if b >= M {
        format!("{:.1} MiB", b as f64 / M as f64)
    } else if b >= K {
        format!("{:.1} KiB", b as f64 / K as f64)
    } else {
        format!("{b} B")
    }
}
