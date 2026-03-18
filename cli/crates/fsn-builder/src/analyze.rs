//! `fsn-builder analyze` — Docker Compose → ContainerAppResource.
//!
//! # What it does
//!
//! 1. Parses the Docker Compose YAML.
//! 2. Detects the primary service and sub-services (databases, caches, etc.).
//! 3. Infers roles provided by the main service from the image name.
//! 4. Analyzes all environment variables: type, role, confidence, auto-source.
//! 5. Auto-generates network names and suggests S3 backup paths for volumes.
//! 6. Outputs a `ContainerAppResource` as TOML or JSON.

use anyhow::{bail, Context, Result};
use fsn_types::resources::{
    container_app::{
        AutoSource, ContainerAppResource, ContainerService, ContainerVariable,
        NetworkDef, RoleDep, VolumeDef, VarType,
    },
    meta::{ResourceMeta, ResourceType, Role, ValidationStatus},
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

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run(path: &Path, format: &str) -> Result<()> {
    let yaml = std::fs::read_to_string(path)
        .with_context(|| format!("Cannot read {}", path.display()))?;
    let resource = analyze_compose(&yaml, path)?;
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&resource)?),
        _      => println!("{}", toml::to_string_pretty(&resource)?),
    }
    Ok(())
}

// ── Analysis pipeline ─────────────────────────────────────────────────────────

pub fn analyze_compose(yaml: &str, source_path: &Path) -> Result<ContainerAppResource> {
    let compose: ComposeFile = serde_yaml::from_str(yaml)
        .context("Failed to parse Docker Compose YAML")?;

    if compose.services.is_empty() {
        bail!("No services found in the Compose file.");
    }

    // ── Detect primary service ──────────────────────────────────────────────
    let (primary_name, primary_def) = detect_primary_service(&compose)?;
    let primary_image = primary_def.image.as_deref().unwrap_or("unknown");
    let (image_name, image_tag) = split_image(primary_image);

    // ── Build ContainerService list ─────────────────────────────────────────
    let services: Vec<ContainerService> = compose.services.iter().map(|(name, def)| {
        let (img, tag) = split_image(def.image.as_deref().unwrap_or(""));
        let is_main = name == primary_name;
        let port = extract_first_port(def);
        let internal = is_infrastructure(&img);
        ContainerService {
            name: name.clone(),
            image: def.image.clone().unwrap_or_default(),
            is_main,
            internal,
            port,
            healthcheck: def.healthcheck.as_ref().map(|_| "defined".to_string()),
            version_tag: tag,
        }
    }).collect();

    // ── Roles provided ──────────────────────────────────────────────────────
    let roles_provided = detect_roles_provided(&image_name);

    // ── Variables ───────────────────────────────────────────────────────────
    let all_service_names: Vec<String> = compose.services.keys().cloned().collect();
    let mut variables: Vec<ContainerVariable> = Vec::new();
    for (svc_name, def) in &compose.services {
        let env_vars = extract_env_vars(def);
        for var_name in env_vars {
            let analyzed = analyze_variable(&var_name, svc_name, &all_service_names);
            variables.push(analyzed);
        }
    }
    variables.dedup_by(|a, b| a.name == b.name);
    variables.sort_by(|a, b| a.name.cmp(&b.name));

    // ── Networks ────────────────────────────────────────────────────────────
    let mut networks: Vec<NetworkDef> = compose.networks.keys()
        .map(|n| NetworkDef { name: n.clone(), external: false })
        .collect();
    if networks.is_empty() {
        // Auto-generate a backend network.
        networks.push(NetworkDef {
            name: format!("{}-backend", primary_name),
            external: false,
        });
    }

    // ── Volumes ─────────────────────────────────────────────────────────────
    let volumes: Vec<VolumeDef> = compose.volumes.keys()
        .map(|v| VolumeDef {
            name: v.clone(),
            s3_path: Some(format!("backups/{}/{}", primary_name, v)),
        })
        .collect();

    // ── Required roles (from env var patterns) ──────────────────────────────
    let roles_required = detect_roles_required(&variables);

    // ── ResourceMeta ────────────────────────────────────────────────────────
    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let meta = ResourceMeta {
        id: primary_name.replace([' ', '-'], "_").to_lowercase(),
        name: capitalize(primary_name),
        description: format!(
            "{} — containerized application for FreeSynergy.",
            capitalize(primary_name)
        ),
        version: image_tag.clone(),
        author: String::new(),
        license: "MIT".into(),
        icon: std::path::PathBuf::from("icon.svg"),
        tags: roles_provided.iter().map(|r| r.as_str().to_owned()).collect(),
        resource_type: ResourceType::ContainerApp,
        dependencies: vec![],
        signature: None,
        status: ValidationStatus::Incomplete,
    };

    Ok(ContainerAppResource {
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

// ── Helpers ───────────────────────────────────────────────────────────────────

fn detect_primary_service<'a>(
    compose: &'a ComposeFile,
) -> Result<(&'a str, &'a ComposeServiceDef)> {
    // Prefer services with an exposed port and not an infra image.
    for (name, def) in &compose.services {
        let img = def.image.as_deref().unwrap_or("");
        if !is_infrastructure(img) && extract_first_port(def).is_some() {
            return Ok((name.as_str(), def));
        }
    }
    // Fall back to the first service.
    compose
        .services
        .iter()
        .next()
        .map(|(n, d)| (n.as_str(), d))
        .ok_or_else(|| anyhow::anyhow!("No services found"))
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
        None             => (image.to_owned(), "latest".to_owned()),
    }
}

fn extract_first_port(def: &ComposeServiceDef) -> Option<u16> {
    def.ports.first().and_then(|v| {
        let s = v.as_str().unwrap_or_default();
        s.split(':').last().and_then(|p| p.parse().ok())
    })
}

fn extract_env_vars(def: &ComposeServiceDef) -> Vec<String> {
    match &def.environment {
        Some(serde_json::Value::Object(m)) => m.keys().cloned().collect(),
        Some(serde_json::Value::Array(a))  => a
            .iter()
            .filter_map(|v| {
                v.as_str()
                    .and_then(|s| s.split_once('=').map(|(k, _)| k.to_owned()))
            })
            .collect(),
        _ => vec![],
    }
}

/// Detect roles provided based on the image name.
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
    if img.contains("ollama") || img.contains("llamacpp") {
        roles.push(Role::new("llm"));
    }

    roles
}

/// Detect required roles from variable names.
fn detect_roles_required(vars: &[ContainerVariable]) -> Vec<RoleDep> {
    let mut required_roles: std::collections::HashSet<String> = std::collections::HashSet::new();
    for var in vars {
        if let Some(role) = &var.role {
            required_roles.insert(role.as_str().to_owned());
        }
    }
    required_roles
        .into_iter()
        .map(|r| RoleDep { role: Role::new(r), optional: false })
        .collect()
}

/// Analyze a single environment variable and infer its type, role, and confidence.
fn analyze_variable(
    name: &str,
    service_name: &str,
    all_services: &[String],
) -> ContainerVariable {
    let upper = name.to_uppercase();

    // Detect type
    let var_type = if upper.contains("SECRET") || upper.contains("PASSWORD") || upper.contains("TOKEN") || upper.contains("KEY") {
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
    } else if upper.starts_with("ENABLE_") || upper.starts_with("DISABLE_") || upper.ends_with("_ENABLED") {
        VarType::Bool
    } else {
        VarType::String
    };

    // Detect role from variable name pattern
    let role = detect_var_role(&upper);

    // Detect auto-source from sibling service name patterns
    let auto_from = detect_auto_source(&upper, all_services);

    // Confidence: high if we detected a clear role, medium otherwise
    let confidence: f32 = if role.is_some() { 0.8 } else { 0.4 };

    ContainerVariable {
        name: name.to_owned(),
        var_type,
        role,
        required: !upper.contains("OPTIONAL"),
        default: None,
        auto_from,
        description: generate_description(name),
        confidence,
    }
}

/// Infer which role supplies this variable.
fn detect_var_role(upper: &str) -> Option<Role> {
    if upper.contains("OIDC") || upper.contains("SSO") || upper.contains("LDAP")
        || upper.contains("KANIDM") || upper.contains("KEYCLOAK") || upper.contains("AUTH_URL")
    {
        return Some(Role::new("iam"));
    }
    if upper.contains("SMTP") || upper.contains("MAIL_") || upper.contains("EMAIL_HOST") {
        return Some(Role::new("smtp"));
    }
    if upper.contains("REDIS") || upper.contains("CACHE_URL") {
        return Some(Role::new("cache"));
    }
    if upper.contains("POSTGRES") || upper.contains("MYSQL") || upper.contains("DATABASE_URL")
        || upper.contains("DB_HOST") || upper.contains("DB_NAME")
    {
        return Some(Role::new("database"));
    }
    if upper.contains("S3_") || upper.contains("MINIO") {
        return Some(Role::new("s3"));
    }
    None
}

/// Detect if a variable can be auto-sourced from a sibling service.
fn detect_auto_source(upper: &str, all_services: &[String]) -> Option<AutoSource> {
    for svc in all_services {
        let svc_upper = svc.to_uppercase();
        if upper.contains(&svc_upper) || upper.starts_with(&format!("{}_", &svc_upper)) {
            let url_template = format!("http://{}:{{{{ port }}}}", svc);
            return Some(AutoSource::InternalService {
                service_name: svc.clone(),
                url_template,
            });
        }
    }
    // Role-based auto-source
    if let Some(role) = detect_var_role(upper) {
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

/// Generate a human-readable description from a variable name.
fn generate_description(name: &str) -> String {
    let words: Vec<String> = name
        .split('_')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
            }
        })
        .collect();
    words.join(" ")
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None    => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
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
        let result = analyze_compose(SAMPLE_COMPOSE, std::path::Path::new("compose.yml")).unwrap();
        assert_eq!(result.meta.resource_type, ResourceType::ContainerApp);
        assert!(!result.services.is_empty());
        let main = result.services.iter().find(|s| s.is_main).unwrap();
        assert_eq!(main.name, "forgejo");
        assert!(result.roles_provided.contains(&Role::new("git")));
        assert!(!result.variables.is_empty());
        assert!(!result.volumes.is_empty());
    }

    #[test]
    fn secret_variables_get_secret_type() {
        let var = analyze_variable("POSTGRES_PASSWORD", "postgres", &[]);
        assert_eq!(var.var_type, VarType::Secret);
    }

    #[test]
    fn database_url_gets_database_role() {
        let var = analyze_variable("DATABASE_URL", "app", &["postgres".to_string()]);
        assert_eq!(var.role, Some(Role::new("database")));
    }

    #[test]
    fn sibling_service_gets_internal_auto_source() {
        let var = analyze_variable("REDIS_HOST", "app", &["redis".to_string()]);
        assert!(matches!(var.auto_from, Some(AutoSource::InternalService { .. })));
    }
}
