// compose.rs — Docker Compose / YAML input parsing.

use std::collections::HashMap;

use serde::Deserialize;

// ── Input ─────────────────────────────────────────────────────────────────────

/// Source of the Docker Compose / YAML definition.
#[derive(Debug, Clone)]
pub enum ComposeInput {
    /// Raw YAML text.
    Text(String),
    /// Path to a YAML file on disk.
    File(std::path::PathBuf),
}

impl ComposeInput {
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    pub fn file(p: impl Into<std::path::PathBuf>) -> Self {
        Self::File(p.into())
    }

    /// Resolve to raw YAML text.
    pub fn resolve(&self) -> Result<String, fsn_error::FsyError> {
        match self {
            Self::Text(s) => Ok(s.clone()),
            Self::File(p) => std::fs::read_to_string(p).map_err(|e| {
                fsn_error::FsyError::internal(format!(
                    "wizard: cannot read {}: {e}",
                    p.display()
                ))
            }),
        }
    }
}

// ── Compose YAML types ────────────────────────────────────────────────────────

/// Top-level Docker Compose file.
#[derive(Debug, Deserialize)]
pub struct ComposeFile {
    pub services: HashMap<String, ComposeServiceDef>,
}

/// A single service entry in a compose file.
#[derive(Debug, Deserialize)]
pub struct ComposeServiceDef {
    pub image: Option<String>,
    pub ports: Option<Vec<serde_json::Value>>,
    pub volumes: Option<Vec<serde_json::Value>>,
    pub environment: Option<serde_json::Value>,
    pub healthcheck: Option<serde_json::Value>,
    pub labels: Option<HashMap<String, String>>,
}

// ── ComposeService ────────────────────────────────────────────────────────────

/// Normalised, wizard-friendly view of one service.
#[derive(Debug, Clone)]
pub struct ComposeService {
    /// Service name (key in the compose file).
    pub name: String,
    /// Container image (e.g. `"nginx:alpine"`).
    pub image: String,
    /// Exposed ports as `"<host>:<container>"` strings.
    pub ports: Vec<String>,
    /// Volume mounts as `"<host>:<container>"` strings.
    pub volumes: Vec<String>,
    /// Environment variables as key → value.
    pub env: HashMap<String, String>,
    /// Whether the service has a healthcheck defined.
    pub has_healthcheck: bool,
    /// Compose labels.
    pub labels: HashMap<String, String>,
}

impl ComposeService {
    /// Parse a `ComposeFile` from raw YAML and return all services.
    pub fn parse_all(yaml: &str) -> Result<Vec<Self>, fsn_error::FsyError> {
        let compose: ComposeFile = serde_yaml::from_str(yaml).map_err(|e| {
            fsn_error::FsyError::parse(format!("wizard: invalid compose YAML: {e}"))
        })?;

        let mut services = Vec::new();
        for (name, def) in compose.services {
            services.push(Self::from_def(name, def));
        }
        Ok(services)
    }

    fn from_def(name: String, def: ComposeServiceDef) -> Self {
        let image = def.image.unwrap_or_default();

        let ports = def.ports.unwrap_or_default().iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();

        let volumes = def.volumes.unwrap_or_default().iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();

        let env = match def.environment {
            Some(serde_json::Value::Object(map)) => map
                .into_iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_owned())))
                .collect(),
            Some(serde_json::Value::Array(arr)) => arr
                .into_iter()
                .filter_map(|v| {
                    let s = v.as_str()?;
                    let (k, v) = s.split_once('=')?;
                    Some((k.to_owned(), v.to_owned()))
                })
                .collect(),
            _ => HashMap::new(),
        };

        let has_healthcheck = def.healthcheck.is_some();
        let labels = def.labels.unwrap_or_default();

        Self { name, image, ports, volumes, env, has_healthcheck, labels }
    }
}
