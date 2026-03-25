// fs-s3 – FreeSynergy.Node embedded S3 storage server.
//
// Modules:
//   config     – StorageConfig (TOML deserialization)
//   buckets    – BucketKind + directory initialization (D3)
//   server     – S3Server backed by s3s + s3s-fs (D1)
//   backend    – opendal sync backends: local / SFTP / Hetzner (D2)
//   profile    – NodeProfile read/write in the `profiles/` bucket (D4)
//   federation – FederatedS3Client: inter-node S3 communication (D5)

pub mod backend;
pub mod buckets;
pub mod config;
pub mod federation;
pub mod profile;
pub mod server;

// Convenient re-exports for callers
pub use buckets::{ensure_buckets, BucketInfo, BucketKind};
pub use config::StorageConfig;
pub use federation::FederatedS3Client;
pub use profile::{NodeProfile, ProfileStore};
pub use server::S3Server;
