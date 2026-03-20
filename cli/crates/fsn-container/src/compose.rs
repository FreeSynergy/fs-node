// Compose YAML parser — parses docker-compose / Podman-compose files.
//
// Handles both list and map forms for environment, volumes, ports, depends_on.
// Unknown fields are ignored (permissive parsing).

use std::path::Path;

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

// ── Top-level ─────────────────────────────────────────────────────────────────

/// A parsed docker-compose / Podman-compose file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeFile {
    /// Compose spec version (optional in modern compose).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// All declared services.
    #[serde(default)]
    pub services: IndexMap<String, ComposeService>,

    /// Top-level named volumes.
    #[serde(default)]
    pub volumes: IndexMap<String, Option<serde_yaml::Value>>,

    /// Top-level named networks.
    #[serde(default)]
    pub networks: IndexMap<String, Option<serde_yaml::Value>>,
}

// ── Service ───────────────────────────────────────────────────────────────────

/// One service entry inside `services:`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeService {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,

    /// Env vars — accepts both map and list form.
    #[serde(default, deserialize_with = "deser_env")]
    pub environment: Vec<EnvVar>,

    /// Volume mounts — accepts both string and longform.
    #[serde(default, deserialize_with = "deser_volumes")]
    pub volumes: Vec<String>,

    /// Published ports — accepts both string and longform.
    #[serde(default, deserialize_with = "deser_ports")]
    pub ports: Vec<String>,

    /// Network memberships.
    #[serde(default, deserialize_with = "deser_networks")]
    pub networks: Vec<String>,

    /// Service dependencies.
    #[serde(default, deserialize_with = "deser_depends_on")]
    pub depends_on: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub healthcheck: Option<ComposeHealthcheck>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<serde_yaml::Value>,

    #[serde(default)]
    pub labels: IndexMap<String, String>,
}

// ── EnvVar ────────────────────────────────────────────────────────────────────

/// A single environment variable entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub name: String,
    /// `None` = variable is declared but value comes from host environment.
    pub value: Option<String>,
}

impl EnvVar {
    pub fn new(name: impl Into<String>, value: Option<String>) -> Self {
        Self { name: name.into(), value }
    }

    /// Parse from `"KEY=VALUE"` or `"KEY"` string.
    pub fn from_str(s: &str) -> Self {
        match s.split_once('=') {
            Some((k, v)) => Self::new(k.trim(), Some(v.to_string())),
            None         => Self::new(s.trim(), None),
        }
    }
}

// ── Healthcheck ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeHealthcheck {
    /// `["CMD", "curl", "-f", "http://localhost/health"]` or `["CMD-SHELL", "..."]`
    #[serde(default)]
    pub test: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retries: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_period: Option<String>,
}

// ── Parsing entry point ───────────────────────────────────────────────────────

/// Parse a compose YAML file from disk.
pub fn parse_file(path: &Path) -> Result<ComposeFile> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("reading compose file: {}", path.display()))?;
    parse_str(&content)
}

/// Parse a compose YAML string.
pub fn parse_str(content: &str) -> Result<ComposeFile> {
    serde_yaml::from_str(content)
        .context("parsing compose YAML")
}

// ── Custom deserializers ──────────────────────────────────────────────────────

/// Deserialize `environment:` — accepts map form or list form.
///
/// Map:  `KEY: value`
/// List: `- KEY=VALUE` or `- KEY`
fn deser_env<'de, D>(d: D) -> std::result::Result<Vec<EnvVar>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    let raw: Option<serde_yaml::Value> = Option::deserialize(d)?;
    let Some(raw) = raw else { return Ok(Vec::new()) };

    match raw {
        serde_yaml::Value::Mapping(map) => {
            let mut vars = Vec::new();
            for (k, v) in map {
                let name = yaml_to_string(&k)
                    .map_err(D::Error::custom)?;
                let value = match v {
                    serde_yaml::Value::Null => None,
                    other => Some(yaml_to_string(&other).map_err(D::Error::custom)?),
                };
                vars.push(EnvVar::new(name, value));
            }
            Ok(vars)
        }
        serde_yaml::Value::Sequence(seq) => {
            let mut vars = Vec::new();
            for item in seq {
                let s = yaml_to_string(&item).map_err(D::Error::custom)?;
                vars.push(EnvVar::from_str(&s));
            }
            Ok(vars)
        }
        _ => Err(D::Error::custom("environment must be a mapping or sequence")),
    }
}

/// Deserialize `volumes:` in a service — normalises to `"host:container[:opts]"` strings.
fn deser_volumes<'de, D>(d: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    let raw: Option<serde_yaml::Value> = Option::deserialize(d)?;
    let Some(serde_yaml::Value::Sequence(seq)) = raw else { return Ok(Vec::new()) };

    let mut out = Vec::new();
    for item in seq {
        match item {
            serde_yaml::Value::String(s) => out.push(s),
            serde_yaml::Value::Mapping(map) => {
                // Longform: { type, source, target, read_only }
                let target = map.get("target").and_then(|v| v.as_str()).unwrap_or("");
                let source = map.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let ro     = map.get("read_only").and_then(|v| v.as_bool()).unwrap_or(false);
                let line = if source.is_empty() {
                    target.to_string()
                } else if ro {
                    format!("{source}:{target}:ro")
                } else {
                    format!("{source}:{target}")
                };
                out.push(line);
            }
            other => out.push(yaml_to_string(&other).map_err(D::Error::custom)?),
        }
    }
    Ok(out)
}

/// Deserialize `ports:` — normalises to `"host:container[/proto]"` strings.
fn deser_ports<'de, D>(d: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Option<serde_yaml::Value> = Option::deserialize(d)?;
    let Some(serde_yaml::Value::Sequence(seq)) = raw else { return Ok(Vec::new()) };

    let mut out = Vec::new();
    for item in seq {
        match item {
            serde_yaml::Value::String(s) => out.push(s),
            serde_yaml::Value::Number(n) => out.push(n.to_string()),
            serde_yaml::Value::Mapping(map) => {
                // Longform: { target, published, protocol }
                let target    = map.get("target").and_then(|v| v.as_u64()).unwrap_or(0);
                let published = map.get("published")
                    .map(|v| match v {
                        serde_yaml::Value::Number(n) => n.to_string(),
                        serde_yaml::Value::String(s) => s.clone(),
                        _ => String::new(),
                    })
                    .unwrap_or_default();
                let proto = map.get("protocol").and_then(|v| v.as_str()).unwrap_or("tcp");
                let line = if published.is_empty() {
                    format!("{target}/{proto}")
                } else {
                    format!("{published}:{target}/{proto}")
                };
                out.push(line);
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Deserialize `networks:` in a service — both list and map form.
fn deser_networks<'de, D>(d: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Option<serde_yaml::Value> = Option::deserialize(d)?;
    let Some(raw) = raw else { return Ok(Vec::new()) };

    match raw {
        serde_yaml::Value::Sequence(seq) => {
            Ok(seq.into_iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect())
        }
        serde_yaml::Value::Mapping(map) => {
            Ok(map.keys()
                .filter_map(|k| k.as_str().map(str::to_string))
                .collect())
        }
        _ => Ok(Vec::new()),
    }
}

/// Deserialize `depends_on:` — both list and map form.
fn deser_depends_on<'de, D>(d: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Option<serde_yaml::Value> = Option::deserialize(d)?;
    let Some(raw) = raw else { return Ok(Vec::new()) };

    match raw {
        serde_yaml::Value::Sequence(seq) => {
            Ok(seq.into_iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect())
        }
        serde_yaml::Value::Mapping(map) => {
            Ok(map.keys()
                .filter_map(|k| k.as_str().map(str::to_string))
                .collect())
        }
        _ => Ok(Vec::new()),
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn yaml_to_string(v: &serde_yaml::Value) -> Result<String, String> {
    match v {
        serde_yaml::Value::String(s)  => Ok(s.clone()),
        serde_yaml::Value::Number(n)  => Ok(n.to_string()),
        serde_yaml::Value::Bool(b)    => Ok(b.to_string()),
        serde_yaml::Value::Null       => Ok(String::new()),
        other => Err(format!("unexpected YAML value: {:?}", other)),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE: &str = r#"
version: "3.8"
services:
  app:
    image: myapp:latest
    environment:
      - DATABASE_URL=postgres://db/mydb
      - SECRET_KEY=mysecret
    ports:
      - "8080:8080"
    volumes:
      - app-data:/data
    networks:
      - backend
    depends_on:
      - db
  db:
    image: postgres:16
    environment:
      POSTGRES_DB: mydb
      POSTGRES_USER: user
      POSTGRES_PASSWORD: secret
    volumes:
      - pg-data:/var/lib/postgresql/data
volumes:
  app-data:
  pg-data:
networks:
  backend:
"#;

    #[test]
    fn parse_services() {
        let f = parse_str(SIMPLE).unwrap();
        assert_eq!(f.services.len(), 2);
        assert!(f.services.contains_key("app"));
        assert!(f.services.contains_key("db"));
    }

    #[test]
    fn parse_env_list_form() {
        let f = parse_str(SIMPLE).unwrap();
        let env = &f.services["app"].environment;
        assert!(env.iter().any(|e| e.name == "DATABASE_URL"));
        assert!(env.iter().any(|e| e.name == "SECRET_KEY"));
    }

    #[test]
    fn parse_env_map_form() {
        let f = parse_str(SIMPLE).unwrap();
        let env = &f.services["db"].environment;
        assert!(env.iter().any(|e| e.name == "POSTGRES_DB" && e.value.as_deref() == Some("mydb")));
        assert!(env.iter().any(|e| e.name == "POSTGRES_PASSWORD"));
    }

    #[test]
    fn parse_volumes_and_networks() {
        let f = parse_str(SIMPLE).unwrap();
        assert!(f.services["app"].volumes.iter().any(|v| v.contains("app-data")));
        assert!(f.services["app"].networks.contains(&"backend".to_string()));
        assert_eq!(f.volumes.len(), 2);
        assert_eq!(f.networks.len(), 1);
    }

    #[test]
    fn parse_ports() {
        let f = parse_str(SIMPLE).unwrap();
        assert!(f.services["app"].ports.contains(&"8080:8080".to_string()));
    }
}
