// State resolution – build DesiredState from config files.
//
// Algorithm:
//   1. For each module entry in project.yml load.services:
//      a. Look up the module class in ServiceRegistry
//      b. Resolve sub-modules recursively
//      c. Expand Jinja2 vars with expand_template()
//      d. Build ServiceInstance
//   2. Enforce that instance names are unique (duplicate = error)
//   3. Return DesiredState

use std::collections::HashMap;

use anyhow::{bail, Context, Result};

use fsn_core::{
    config::{HostConfig, ServiceRegistry, ProjectConfig, VaultConfig},
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

/// Resolve a single module instance (and its sub-modules recursively).
fn resolve_instance(
    name: &str,
    class_key: &str,
    project: &ProjectConfig,
    host: &HostConfig,
    registry: &ServiceRegistry,
    vault: &VaultConfig,
    parent_name: Option<&str>,
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

    // Build template context for Jinja2 expansion
    let ctx = TemplateContext {
        project_name: &project.project.name,
        project_domain: &project.project.domain,
        instance_name: name,
        service_domain: &service_domain,
        parent_instance_name: parent_name.unwrap_or(name),
        vault,
    };

    // Expand environment variables
    let resolved_env = resolve_env(&class.environment, &ctx)?;

    // Resolve sub-modules recursively
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
