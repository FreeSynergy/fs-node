// Host config – maps to hosts/{hostname}.host.toml
//
// Rules (per RULES.md):
//   - One file per physical/virtual host
//   - Proxy is ALWAYS defined here, never in project.toml
//   - Host files are git-ignored (only example.host.toml is tracked)
//   - ALWAYS required, even for localhost

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::FsnError;

/// Root structure of a host config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    pub host: HostMeta,
    pub proxy: IndexMap<String, ProxyInstance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostMeta {
    pub name: String,
    pub ip: String,

    #[serde(default)]
    pub ipv6: String,

    /// true = no SSH, read-only from deployer (externally managed host)
    #[serde(default)]
    pub external: bool,
}

/// A proxy instance declaration (typically "zentinel").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyInstance {
    pub service_class: String,

    #[serde(default)]
    pub load: ProxyLoad,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProxyLoad {
    #[serde(default)]
    pub plugins: ProxyPlugins,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProxyPlugins {
    /// DNS provider: "hetzner", "cloudflare", "none"
    #[serde(default = "default_dns")]
    pub dns: String,

    /// ACME provider: "letsencrypt", "smallstep-ca", "none"
    #[serde(default = "default_acme")]
    pub acme: String,

    /// ACME contact email
    pub acme_email: Option<String>,
}

fn default_dns() -> String {
    "hetzner".to_string()
}
fn default_acme() -> String {
    "letsencrypt".to_string()
}

impl HostConfig {
    /// Load a host config from a TOML file.
    pub fn load(path: &Path) -> Result<Self, FsnError> {
        let content = std::fs::read_to_string(path).map_err(|_| FsnError::ConfigNotFound {
            path: path.display().to_string(),
        })?;
        toml::from_str(&content).map_err(|e| FsnError::ConfigParse {
            path: path.display().to_string(),
            source: e,
        })
    }
}
