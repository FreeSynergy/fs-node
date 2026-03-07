// State resolution – build DesiredState from config files.
//
// Algorithm:
//   1. For each module entry in project.yml load.modules:
//      a. Look up the module class in ModuleRegistry
//      b. Resolve sub-modules recursively
//      c. Expand Jinja2 vars with expand_template()
//      d. Build ModuleInstance
//   2. Enforce that instance names are unique (duplicate = error)
//   3. Return DesiredState

use std::collections::HashMap;

use anyhow::{bail, Context, Result};

use fsn_core::{
    config::{HostConfig, ModuleRegistry, ProjectConfig, VaultConfig},
    state::desired::{DesiredState, ModuleInstance},
};

use crate::template::TemplateContext;

/// Build the desired state from the three config layers.
pub fn resolve_desired(
    project: &ProjectConfig,
    host: &HostConfig,
    registry: &ModuleRegistry,
    vault: &VaultConfig,
) -> Result<DesiredState> {
    let mut instances = Vec::new();
    let mut seen_names = HashMap::new();

    for (instance_name, module_ref) in &project.load.modules {
        // Uniqueness check (per RULES.md: duplicate instance name = abort)
        if let Some(existing) = seen_names.insert(instance_name.clone(), module_ref.module_class.clone()) {
            bail!(
                "Duplicate service name '{}' in project '{}' (already defined as {})",
                instance_name,
                project.project.name,
                existing
            );
        }

        let instance = resolve_instance(
            instance_name,
            &module_ref.module_class,
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
        modules: instances,
    })
}

/// Resolve a single module instance (and its sub-modules recursively).
fn resolve_instance(
    name: &str,
    class_key: &str,
    project: &ProjectConfig,
    host: &HostConfig,
    registry: &ModuleRegistry,
    vault: &VaultConfig,
    parent_name: Option<&str>,
) -> Result<ModuleInstance> {
    let class = registry
        .get(class_key)
        .with_context(|| format!("Module class '{}' not found in registry", class_key))?
        .clone();

    let service_domain = format!("{}.{}", name, project.project.domain);
    let alias_domains: Vec<String> = class
        .module
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
    let mut sub_modules = Vec::new();
    for (sub_name_tpl, sub_ref) in &class.load.modules {
        let sub_name = format!("{}-{}", name, sub_name_tpl);
        let sub = resolve_instance(
            &sub_name,
            &sub_ref.module_class,
            project,
            host,
            registry,
            vault,
            Some(name),
        )
        .with_context(|| format!("Resolving sub-module '{}'", sub_name))?;
        sub_modules.push(sub);
    }

    Ok(ModuleInstance {
        name: name.to_string(),
        class_key: class_key.to_string(),
        version: class.module.version.clone(),
        class,
        resolved_env,
        service_domain,
        alias_domains,
        sub_modules,
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
