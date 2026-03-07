// Desired state – what SHOULD be running according to project + host config.

use std::collections::HashMap;

use crate::config::module::ModuleClass;

/// The fully resolved desired state for a project on a host.
#[derive(Debug, Clone)]
pub struct DesiredState {
    pub project_name: String,
    pub domain: String,
    pub modules: Vec<ModuleInstance>,
}

/// A resolved module instance – the module class with all Jinja2 vars expanded.
#[derive(Debug, Clone)]
pub struct ModuleInstance {
    /// Instance name (e.g. "forgejo") – unique per project.
    pub name: String,

    /// Module class key (e.g. "git/forgejo").
    pub class_key: String,

    /// The class template this instance was resolved from.
    pub class: ModuleClass,

    /// Jinja2-expanded environment variables (ready for Quadlet/env file).
    pub resolved_env: HashMap<String, String>,

    /// The full subdomain this service listens on (e.g. "forgejo.example.com").
    pub service_domain: String,

    /// Alias subdomains (CNAME targets).
    pub alias_domains: Vec<String>,

    /// Sub-modules owned by this instance (e.g. postgres, dragonfly).
    pub sub_modules: Vec<ModuleInstance>,

    /// Version from the class definition (used to detect updates).
    pub version: String,
}
