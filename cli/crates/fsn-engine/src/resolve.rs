// State resolution – build DesiredState from config files.
//
// Algorithm:
//   1. Pre-compute cross-service vars (MAIL_HOST, IAM_URL, …) from project entries.
//   2. For each module entry in project.yml load.services:
//      a. Look up the module class in ServiceRegistry
//      b. Resolve sub-modules recursively
//      c. Expand Jinja2 vars with expand_template() (includes cross-service vars)
//      d. Build ServiceInstance
//   3. Enforce that instance names are unique (duplicate = error)
//   4. Return DesiredState

use std::collections::HashMap;

use anyhow::{bail, Context, Result};

use fsn_core::{
    config::{HostConfig, ServiceRegistry, ProjectConfig, VaultConfig},
    resource::ProjectResource,
    state::desired::{DesiredState, ServiceInstance},
};

use crate::template::TemplateContext;

/// Build the desired state from the three config layers.
pub fn resolve_desired(
    project: &ProjectConfig,
    host: &HostConfig,
    registry: &ServiceRegistry,
    vault: &VaultConfig,
) -> Result<DesiredState> {
    // Pre-compute cross-service vars from all service entries.
    // Done once before resolution so every service can reference sibling services.
    let cross_vars = collect_cross_service_vars(project);

    let mut instances = Vec::new();
    let mut seen_names = HashMap::new();

    for (instance_name, module_ref) in &project.load.services {
        // Uniqueness check (per RULES.md: duplicate instance name = abort)
        if let Some(existing) = seen_names.insert(instance_name.clone(), module_ref.service_class.clone()) {
            bail!(
                "Duplicate service name '{}' in project '{}' (already defined as {})",
                instance_name,
                project.project.name,
                existing
            );
        }

        let instance = resolve_instance(
            instance_name,
            &module_ref.service_class,
            project,
            host,
            registry,
            vault,
            None, // no parent
            &cross_vars,
        )
        .with_context(|| format!("Resolving module '{}'", instance_name))?;

        instances.push(instance);
    }

    Ok(DesiredState {
        project_name: project.project.name.clone(),
        domain: project.project.domain.clone(),
        services: instances,
    })
}

/// Map a service class key prefix to its exported variable prefix.
/// e.g. "mail/stalwart" → Some("MAIL"), "git/forgejo" → Some("GIT")
fn service_class_prefix(class_key: &str) -> Option<&'static str> {
    match class_key.split('/').next()? {
        "mail"       => Some("MAIL"),
        "iam"        => Some("IAM"),
        "git"        => Some("GIT"),
        "chat"       => Some("CHAT"),
        "wiki"       => Some("WIKI"),
        "tasks"      => Some("TASKS"),
        "collab"     => Some("COLLAB"),
        "monitoring" => Some("MONITORING"),
        "tickets"    => Some("TICKETS"),
        "maps"       => Some("MAPS"),
        _            => None,
    }
}

/// Pre-compute cross-service variables from the project load entries.
///
/// Derived from instance names + project domain before ServiceClass loading,
/// so no chicken-and-egg problem. Each service can reference sibling services
/// via `{{ mail_host }}`, `{{ iam_url }}`, etc. in its Jinja2 environment block.
pub fn collect_cross_service_vars(project: &ProjectConfig) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    // Project-level vars
    vars.insert("PROJECT_NAME".into(),   project.project.name.clone());
    vars.insert("PROJECT_DOMAIN".into(), project.project.domain.clone());
    if let Some(email) = project.contact_email() {
        vars.insert("PROJECT_EMAIL".into(), email.to_string());
    }

    // Cross-service vars (MAIL_HOST, IAM_URL, GIT_DOMAIN, etc.)
    for (instance_name, entry) in &project.load.services {
        if let Some(prefix) = service_class_prefix(&entry.service_class) {
            let subdomain = entry.subdomain.as_deref().unwrap_or(instance_name.as_str());
            let domain = format!("{}.{}", subdomain, project.project.domain);
            vars.insert(format!("{prefix}_HOST"),   instance_name.clone());
            vars.insert(format!("{prefix}_DOMAIN"), domain.clone());
            vars.insert(format!("{prefix}_URL"),    format!("https://{domain}"));
            if let Some(port) = entry.port {
                vars.insert(format!("{prefix}_PORT"), port.to_string());
            }
        }
    }

    vars
}

/// Resolve a single module instance (and its sub-modules recursively).
fn resolve_instance(
    name: &str,
    class_key: &str,
    project: &ProjectConfig,
    host: &HostConfig,
    registry: &ServiceRegistry,
    vault: &VaultConfig,
    parent_name: Option<&str>,
    cross_vars: &HashMap<String, String>,
) -> Result<ServiceInstance> {
    let class = registry
        .get(class_key)
        .with_context(|| format!("Module class '{}' not found in registry", class_key))?
        .clone();

    let service_domain = format!("{}.{}", name, project.project.domain);
    let alias_domains: Vec<String> = class
        .meta
        .alias
        .iter()
        .map(|a| format!("{}.{}", a, project.project.domain))
        .collect();

    // Build template context for Jinja2 expansion (includes cross-service vars)
    let ctx = TemplateContext {
        project_name: &project.project.name,
        project_domain: &project.project.domain,
        instance_name: name,
        service_domain: &service_domain,
        parent_instance_name: parent_name.unwrap_or(name),
        vault,
        cross_vars: cross_vars.clone(),
    };

    // Expand environment variables
    let resolved_env = resolve_env(&class.environment, &ctx)?;

    // Resolve sub-modules recursively (same cross_vars for the whole project)
    let mut sub_services = Vec::new();
    for (sub_name_tpl, sub_ref) in &class.load.sub_services {
        let sub_name = format!("{}-{}", name, sub_name_tpl);
        let sub = resolve_instance(
            &sub_name,
            &sub_ref.service_class,
            project,
            host,
            registry,
            vault,
            Some(name),
            cross_vars,
        )
        .with_context(|| format!("Resolving sub-module '{}'", sub_name))?;
        sub_services.push(sub);
    }

    Ok(ServiceInstance {
        name: name.to_string(),
        class_key: class_key.to_string(),
        service_type: class.meta.service_type.clone(),
        version: class.meta.version.clone(),
        class,
        resolved_env,
        service_domain,
        alias_domains,
        sub_services,
    })
}

/// Expand all Jinja2 strings in the environment block.
fn resolve_env(
    raw_env: &indexmap::IndexMap<String, String>,
    ctx: &TemplateContext,
) -> Result<HashMap<String, String>> {
    let mut out = HashMap::new();
    for (key, template) in raw_env {
        let value = crate::template::render(template, ctx)
            .with_context(|| format!("Expanding env var '{}'", key))?;
        out.insert(key.clone(), value);
    }
    Ok(out)
}
