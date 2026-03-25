//! `fs-builder analyze` — Docker Compose → ContainerResource.
//!
//! # What it does
//!
//! 1. Parses the Docker Compose YAML.
//! 2. Detects the primary service and sub-services (databases, caches, etc.).
//! 3. Infers roles provided by the main service from the image name.
//! 4. Analyzes all environment variables: type, role, confidence, auto-source.
//! 5. Auto-generates network names and suggests S3 backup paths for volumes.
//! 6. Outputs a `ContainerResource` as TOML or JSON.

use anyhow::{bail, Context, Result};
use fs_types::{
    primitives::SemVer,
    resources::{
        container::{
            AutoSource, ContainerResource, ContainerService, ContainerVariable, NetworkDef,
            RoleDep, VarType, VolumeDef,
        },
        meta::{ResourceMeta, ResourceType, Role, ValidationStatus},
    },
    tags::FsTag,
};
use serde::Deserialize;
use std::{collections::HashMap, path::Path};

// ── Docker Compose parsing ────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ComposeFile {
    services: HashMap<String, ComposeServiceDef>,
    #[serde(default)]
    volumes: HashMap<String, Option<serde_json::Value>>,
    #[serde(default)]
    networks: HashMap<String, Option<serde_json::Value>>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ComposeServiceDef {
    image: Option<String>,
    #[serde(default)]
    ports: Vec<serde_json::Value>,
    #[serde(default)]
    volumes: Vec<serde_json::Value>,
    environment: Option<serde_json::Value>,
    healthcheck: Option<serde_json::Value>,
    #[serde(default)]
    networks: Vec<serde_json::Value>,
    depends_on: Option<serde_json::Value>,
}

// ── ComposeAnalyzer ───────────────────────────────────────────────────────────

pub struct ComposeAnalyzer;

impl ComposeAnalyzer {
    /// Read `path`, analyze it and print the result in the given format.
    pub fn run(path: &Path, format: &str) -> Result<()> {
        let yaml = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read {}", path.display()))?;
        let resource = Self::do_analyze(&yaml, path)?;
        match format {
            "json" => println!("{}", serde_json::to_string_pretty(&resource)?),
            _ => println!("{}", toml::to_string_pretty(&resource)?),
        }
        Ok(())
    }

    /// Parse `yaml` and produce a `ContainerResource`.
    fn do_analyze(yaml: &str, source_path: &Path) -> Result<ContainerResource> {
        let compose: ComposeFile =
            serde_yml::from_str(yaml).context("Failed to parse Docker Compose YAML")?;

        if compose.services.is_empty() {
            bail!("No services found in the Compose file.");
        }

        let (primary_name, primary_def) = Self::detect_primary(&compose)?;
        let primary_image = primary_def.image.as_deref().unwrap_or("unknown");
        let (image_name, image_tag) = Self::split_image(primary_image);

        let services = Self::build_services(&compose, primary_name);
        let roles_provided = Self::detect_roles_provided(&image_name);

        let all_service_names: Vec<String> = compose.services.keys().cloned().collect();
        let mut variables: Vec<ContainerVariable> = Vec::new();
        for (svc_name, def) in &compose.services {
            for var_name in Self::extract_env_vars(def) {
                variables.push(Self::analyze_variable(
                    &var_name,
                    svc_name,
                    &all_service_names,
                ));
            }
        }
        variables.dedup_by(|a, b| a.name == b.name);
        variables.sort_by(|a, b| a.name.cmp(&b.name));

        let mut networks: Vec<NetworkDef> = compose
            .networks
            .keys()
            .map(|n| NetworkDef {
                name: n.clone(),
                external: false,
            })
            .collect();
        if networks.is_empty() {
            networks.push(NetworkDef {
                name: format!("{}-backend", primary_name),
                external: false,
            });
        }

        let volumes: Vec<VolumeDef> = compose
            .volumes
            .keys()
            .map(|v| VolumeDef {
                name: v.clone(),
                s3_path: Some(format!("backups/{}/{}", primary_name, v)),
            })
            .collect();

        let roles_required = Self::detect_roles_required(&variables);

        let _stem = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let display_name = Self::capitalize(primary_name);
        let meta = ResourceMeta {
            id: primary_name.replace([' ', '-'], "_").to_lowercase(),
            name: display_name.clone(),
            summary: format!("{display_name} — containerized application for FreeSynergy."),
            description: format!("{display_name} — containerized application for FreeSynergy."),
            description_file: std::path::PathBuf::new(),
            version: image_tag.parse::<SemVer>().unwrap_or(SemVer {
                major: 0,
                minor: 0,
                patch: 1,
                pre: None,
            }),
            author: String::new(),
            license: "MIT".into(),
            icon: std::path::PathBuf::from("icon.svg"),
            tags: roles_provided
                .iter()
                .map(|r| FsTag::new(r.as_str()))
                .collect(),
            resource_type: ResourceType::Container,
            dependencies: vec![],
            signature: None,
            status: ValidationStatus::Incomplete,
            source: None,
            platform: None,
        };

        Ok(ContainerResource {
            meta,
            compose_yaml: yaml.to_owned(),
            services,
            roles_provided,
            roles_required,
            apis: vec![],
            variables,
            networks,
            volumes,
        })
    }

    fn build_services(compose: &ComposeFile, primary_name: &str) -> Vec<ContainerService> {
        compose
            .services
            .iter()
            .map(|(name, def)| {
                let (img, tag) = Self::split_image(def.image.as_deref().unwrap_or(""));
                ContainerService {
                    name: name.clone(),
                    image: def.image.clone().unwrap_or_default(),
                    is_main: name == primary_name,
                    internal: Self::is_infrastructure(&img),
                    port: Self::extract_first_port(def),
                    healthcheck: def.healthcheck.as_ref().map(|_| "defined".to_string()),
                    version_tag: tag,
                }
            })
            .collect()
    }

    fn detect_primary(compose: &ComposeFile) -> Result<(&str, &ComposeServiceDef)> {
        for (name, def) in &compose.services {
            let img = def.image.as_deref().unwrap_or("");
            if !Self::is_infrastructure(img) && Self::extract_first_port(def).is_some() {
                return Ok((name.as_str(), def));
            }
        }
        compose
            .services
            .iter()
            .next()
            .map(|(n, d)| (n.as_str(), d))
            .ok_or_else(|| anyhow::anyhow!("No services found"))
    }

    fn detect_roles_provided(image: &str) -> Vec<Role> {
        let img = image.to_lowercase();
        let mut roles = vec![];
        if img.contains("kanidm") || img.contains("keycloak") || img.contains("authentik") {
            roles.push(Role::new("iam"));
        }
        if img.contains("forgejo") || img.contains("gitea") || img.contains("gitlab") {
            roles.push(Role::new("git"));
        }
        if img.contains("outline") || img.contains("bookstack") || img.contains("wiki") {
            roles.push(Role::new("wiki"));
        }
        if img.contains("stalwart") || img.contains("postfix") || img.contains("maddy") {
            roles.push(Role::new("smtp"));
        }
        if img.contains("element") || img.contains("synapse") || img.contains("tuwunel") {
            roles.push(Role::new("chat"));
        }
        if img.contains("vikunja") || img.contains("plane") {
            roles.push(Role::new("tasks"));
        }
        if img.contains("openobserve") || img.contains("grafana") {
            roles.push(Role::new("monitoring"));
        }
        if img.contains("postgres") || img.contains("mysql") || img.contains("mariadb") {
            roles.push(Role::new("database"));
        }
        if img.contains("redis") || img.contains("dragonfly") || img.contains("valkey") {
            roles.push(Role::new("cache"));
        }
        if img.contains("mistral") || img.contains("ollama") || img.contains("llamacpp") {
            roles.push(Role::new("llm"));
        }
        roles
    }

    fn detect_roles_required(vars: &[ContainerVariable]) -> Vec<RoleDep> {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        vars.iter()
            .filter_map(|v| v.role.as_ref())
            .filter(|r| seen.insert(r.as_str().to_owned()))
            .map(|r| RoleDep {
                role: r.clone(),
                optional: false,
            })
            .collect()
    }

    pub fn analyze_variable(
        name: &str,
        _service_name: &str,
        all_services: &[String],
    ) -> ContainerVariable {
        let upper = name.to_uppercase();

        let var_type = if upper.contains("SECRET")
            || upper.contains("PASSWORD")
            || upper.contains("TOKEN")
            || upper.contains("KEY")
        {
            VarType::Secret
        } else if upper.contains("_URL") || upper.contains("_URI") {
            VarType::Url
        } else if upper.contains("_HOST") || upper.contains("_HOSTNAME") {
            VarType::Hostname
        } else if upper.contains("_PORT") {
            VarType::Port
        } else if upper.contains("_EMAIL") || upper.contains("_MAIL") {
            VarType::Email
        } else if upper.contains("_PATH") || upper.contains("_DIR") {
            VarType::Path
        } else if upper.starts_with("ENABLE_")
            || upper.starts_with("DISABLE_")
            || upper.ends_with("_ENABLED")
        {
            VarType::Bool
        } else {
            VarType::String
        };

        let role = Self::detect_var_role(&upper);
        let auto_from = Self::detect_auto_source(&upper, all_services);
        let confidence: f32 = if role.is_some() { 0.8 } else { 0.4 };

        ContainerVariable {
            name: name.to_owned(),
            var_type,
            role,
            required: !upper.contains("OPTIONAL"),
            default: None,
            auto_from,
            description: Self::generate_description(name),
            confidence,
        }
    }

    fn detect_var_role(upper: &str) -> Option<Role> {
        if upper.contains("OIDC")
            || upper.contains("SSO")
            || upper.contains("LDAP")
            || upper.contains("KANIDM")
            || upper.contains("KEYCLOAK")
            || upper.contains("AUTH_URL")
        {
            return Some(Role::new("iam"));
        }
        if upper.contains("SMTP") || upper.contains("MAIL_") || upper.contains("EMAIL_HOST") {
            return Some(Role::new("smtp"));
        }
        if upper.contains("REDIS") || upper.contains("CACHE_URL") {
            return Some(Role::new("cache"));
        }
        if upper.contains("POSTGRES")
            || upper.contains("MYSQL")
            || upper.contains("DATABASE_URL")
            || upper.contains("DB_HOST")
            || upper.contains("DB_NAME")
        {
            return Some(Role::new("database"));
        }
        if upper.contains("S3_") || upper.contains("MINIO") {
            return Some(Role::new("s3"));
        }
        None
    }

    fn detect_auto_source(upper: &str, all_services: &[String]) -> Option<AutoSource> {
        for svc in all_services {
            let svc_upper = svc.to_uppercase();
            if upper.contains(&svc_upper) || upper.starts_with(&format!("{}_", &svc_upper)) {
                return Some(AutoSource::InternalService {
                    service_name: svc.clone(),
                    url_template: format!("http://{}:{{{{ port }}}}", svc),
                });
            }
        }
        if let Some(role) = Self::detect_var_role(upper) {
            let field = if upper.contains("_URL") || upper.contains("_URI") {
                "base_url"
            } else if upper.contains("_HOST") {
                "host"
            } else {
                "endpoint"
            };
            return Some(AutoSource::RoleVariable {
                role,
                field: field.to_owned(),
            });
        }
        None
    }

    fn generate_description(name: &str) -> String {
        name.split('_')
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => {
                        c.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn is_infrastructure(image: &str) -> bool {
        let img = image.to_lowercase();
        img.contains("postgres")
            || img.contains("mysql")
            || img.contains("mariadb")
            || img.contains("redis")
            || img.contains("dragonfly")
            || img.contains("memcached")
            || img.contains("valkey")
            || img.contains("minio")
            || img.contains("nginx")
            || img.contains("caddy")
            || img.contains("traefik")
    }

    fn split_image(image: &str) -> (String, String) {
        match image.rsplit_once(':') {
            Some((img, tag)) => (img.to_owned(), tag.to_owned()),
            None => (image.to_owned(), "latest".to_owned()),
        }
    }

    fn extract_first_port(def: &ComposeServiceDef) -> Option<u16> {
        def.ports.first().and_then(|v| {
            let s = v.as_str().unwrap_or_default();
            s.split(':').next_back().and_then(|p| p.parse().ok())
        })
    }

    fn extract_env_vars(def: &ComposeServiceDef) -> Vec<String> {
        match &def.environment {
            Some(serde_json::Value::Object(m)) => m.keys().cloned().collect(),
            Some(serde_json::Value::Array(a)) => a
                .iter()
                .filter_map(|v| {
                    v.as_str()
                        .and_then(|s| s.split_once('=').map(|(k, _)| k.to_owned()))
                })
                .collect(),
            _ => vec![],
        }
    }

    fn capitalize(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }
}

// ── Public free-fn shims (used by main.rs and tests) ─────────────────────────

pub fn run(path: &Path, format: &str) -> Result<()> {
    ComposeAnalyzer::run(path, format)
}

#[allow(dead_code)]
pub fn analyze_compose(yaml: &str, source_path: &Path) -> Result<ContainerResource> {
    ComposeAnalyzer::do_analyze(yaml, source_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_COMPOSE: &str = r#"
version: "3"
services:
  forgejo:
    image: codeberg.org/forgejo/forgejo:7
    ports:
      - "3000:3000"
    environment:
      FORGEJO__database__DB_TYPE: postgres
      FORGEJO__database__HOST: postgres:5432
      FORGEJO__database__NAME: forgejo
      FORGEJO__database__PASSWD: secret
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/-/health"]
    volumes:
      - forgejo-data:/data
  postgres:
    image: postgres:16
    environment:
      POSTGRES_DB: forgejo
      POSTGRES_USER: forgejo
      POSTGRES_PASSWORD: secret
    volumes:
      - postgres-data:/var/lib/postgresql/data
volumes:
  forgejo-data:
  postgres-data:
"#;

    #[test]
    fn analyze_forgejo_compose() {
        let result =
            ComposeAnalyzer::do_analyze(SAMPLE_COMPOSE, std::path::Path::new("compose.yml"))
                .unwrap();
        assert_eq!(result.meta.resource_type, ResourceType::Container);
        assert!(!result.services.is_empty());
        let main = result.services.iter().find(|s| s.is_main).unwrap();
        assert_eq!(main.name, "forgejo");
        assert!(result.roles_provided.contains(&Role::new("git")));
        assert!(!result.variables.is_empty());
        assert!(!result.volumes.is_empty());
    }

    #[test]
    fn secret_variables_get_secret_type() {
        let var = ComposeAnalyzer::analyze_variable("POSTGRES_PASSWORD", "postgres", &[]);
        assert_eq!(var.var_type, VarType::Secret);
    }

    #[test]
    fn database_url_gets_database_role() {
        let var =
            ComposeAnalyzer::analyze_variable("DATABASE_URL", "app", &["postgres".to_string()]);
        assert_eq!(var.role, Some(Role::new("database")));
    }

    #[test]
    fn sibling_service_gets_internal_auto_source() {
        let var = ComposeAnalyzer::analyze_variable("REDIS_HOST", "app", &["redis".to_string()]);
        assert!(matches!(
            var.auto_from,
            Some(AutoSource::InternalService { .. })
        ));
    }
}
