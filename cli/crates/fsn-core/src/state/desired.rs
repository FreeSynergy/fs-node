// Desired state – what SHOULD be running according to project + host config.

use std::collections::HashMap;

use crate::config::service::{ServiceClass, ServiceType};
use crate::resource::VarProvider;

/// The fully resolved desired state for a project on a host.
#[derive(Debug, Clone)]
pub struct DesiredState {
    pub project_name: String,
    pub domain: String,
    /// Top-level service instances (sub-services nested inside).
    pub services: Vec<ServiceInstance>,
}

/// A resolved service instance – the class with all Jinja2 vars expanded.
#[derive(Debug, Clone)]
pub struct ServiceInstance {
    /// Instance name (e.g. "forgejo") – unique per project.
    pub name: String,

    /// Service class key (e.g. "git/forgejo").
    pub class_key: String,

    /// The class template this instance was resolved from.
    pub class: ServiceClass,

    /// Functional type (convenience copy from class.meta.service_type).
    pub service_type: ServiceType,

    /// Jinja2-expanded environment variables (ready for Quadlet .env file).
    pub resolved_env: HashMap<String, String>,

    /// The full subdomain this service listens on (e.g. "forgejo.example.com").
    pub service_domain: String,

    /// Alias subdomains (CNAME targets).
    pub alias_domains: Vec<String>,

    /// Sub-services owned by this instance (e.g. postgres, dragonfly).
    pub sub_services: Vec<ServiceInstance>,

    /// Version from the class definition (used to detect updates).
    pub version: String,
}

// ── VarProvider impl ──────────────────────────────────────────────────────────

impl VarProvider for ServiceInstance {
    /// Exports cross-service variables based on the service type.
    ///
    /// Internal services (Database, Cache, Proxy, Bot) return an empty map —
    /// they are not consumed directly by user-facing services via template vars.
    fn exported_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        let prefix = match self.service_type {
            ServiceType::Mail       => "MAIL",
            ServiceType::Iam        => "IAM",
            ServiceType::Git        => "GIT",
            ServiceType::Chat       => "CHAT",
            ServiceType::Wiki       => "WIKI",
            ServiceType::Tasks      => "TASKS",
            ServiceType::Collab     => "COLLAB",
            ServiceType::Monitoring => "MONITORING",
            ServiceType::Tickets    => "TICKETS",
            ServiceType::Maps       => "MAPS",
            _ => return vars,
        };
        vars.insert(format!("{prefix}_HOST"),   self.name.clone());
        vars.insert(format!("{prefix}_DOMAIN"), self.service_domain.clone());
        vars.insert(format!("{prefix}_URL"),    format!("https://{}", self.service_domain));
        vars.insert(format!("{prefix}_PORT"),   self.class.meta.port.to_string());
        vars
    }
}
