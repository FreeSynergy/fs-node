// generate.rs — Generate FSN module TOML from a detected ComposeService.

use std::collections::HashMap;

use crate::compose::ComposeService;
use crate::detect::ServiceTypeHint;

// ── ModuleToml ────────────────────────────────────────────────────────────────

/// Generated FSN module definition.
#[derive(Debug, Clone)]
pub struct ModuleToml {
    pub name: String,
    pub class: String,
    pub image: String,
    pub description: String,
    pub ports: Vec<String>,
    pub volumes: Vec<String>,
    pub env: HashMap<String, String>,
    pub health_path: Option<String>,
}

impl ModuleToml {
    /// Serialise to TOML text in FSN module format.
    pub fn to_toml(&self) -> String {
        let mut out = String::new();

        out.push_str("[module]\n");
        out.push_str(&format!("name        = {:?}\n", self.name));
        out.push_str(&format!("class       = {:?}\n", self.class));
        out.push_str(&format!("description = {:?}\n", self.description));
        out.push('\n');

        out.push_str("[container]\n");
        out.push_str(&format!("image = {:?}\n", self.image));

        if let Some(hp) = &self.health_path {
            out.push_str(&format!("health_path = {:?}\n", hp));
        }

        // healthcheck block
        out.push('\n');
        out.push_str("[container.healthcheck]\n");
        out.push_str("test     = [\"CMD\", \"curl\", \"-f\", \"http://localhost/health\"]\n");
        out.push_str("interval = \"30s\"\n");
        out.push_str("timeout  = \"10s\"\n");
        out.push_str("retries  = 3\n");

        if !self.ports.is_empty() {
            out.push('\n');
            out.push_str("[container.published_ports]\n");
            for p in &self.ports {
                out.push_str(&format!("# {p}\n"));
            }
        }

        if !self.volumes.is_empty() {
            out.push('\n');
            out.push_str("# volumes:\n");
            for v in &self.volumes {
                out.push_str(&format!("#   {v}\n"));
            }
        }

        if !self.env.is_empty() {
            out.push('\n');
            out.push_str("[environment]\n");
            let mut keys: Vec<_> = self.env.keys().collect();
            keys.sort();
            for k in keys {
                let v = &self.env[k];
                out.push_str(&format!("{k} = {:?}\n", v));
            }
        }

        out
    }
}

// ── Generator ─────────────────────────────────────────────────────────────────

/// Generate a `ModuleToml` from a `ComposeService` and its detected type hint.
pub fn generate(svc: &ComposeService, hint: &ServiceTypeHint) -> ModuleToml {
    let class = if hint.class == "unknown" {
        // Default fallback
        "proxy/zentinel".to_owned()
    } else {
        hint.class.clone()
    };

    let health_path = guess_health_path(&class);

    ModuleToml {
        name: svc.name.clone(),
        class,
        image: svc.image.clone(),
        description: format!("Auto-generated from Docker Compose service '{}'", svc.name),
        ports: svc.ports.clone(),
        volumes: svc.volumes.clone(),
        env: svc.env.clone(),
        health_path,
    }
}

fn guess_health_path(class: &str) -> Option<String> {
    match class.split('/').next().unwrap_or("") {
        "proxy" => Some("/health".to_owned()),
        "mail" => None,
        "git" => Some("/health".to_owned()),
        "wiki" => Some("/healthcheck".to_owned()),
        "iam" => Some("/status".to_owned()),
        "chat" => Some("/health".to_owned()),
        "monitoring" => Some("/-/health".to_owned()),
        _ => Some("/health".to_owned()),
    }
}
