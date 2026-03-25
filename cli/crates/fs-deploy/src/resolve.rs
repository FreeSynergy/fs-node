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

use fs_node_core::{
    config::{HostConfig, ProjectConfig, ServiceRegistry, VaultConfig},
    state::desired::{DesiredState, ServiceInstance},
};

use crate::template::{CrossVars, ModuleVars, PluginVars, TemplateContext};

// ── StateResolver ─────────────────────────────────────────────────────────────

pub struct StateResolver<'a> {
    project: &'a ProjectConfig,
    host: &'a HostConfig,
    registry: &'a ServiceRegistry,
    vault: &'a VaultConfig,
}

impl<'a> StateResolver<'a> {
    pub fn new(
        project: &'a ProjectConfig,
        host: &'a HostConfig,
        registry: &'a ServiceRegistry,
        vault: &'a VaultConfig,
    ) -> Self {
        Self {
            project,
            host,
            registry,
            vault,
        }
    }

    /// Build the desired state from the three config layers.
    ///
    /// `data_root` – when `Some`, volumes in module TOMLs are rendered with a
    ///   resolved `{{ project_root }}` and `{{ module_vars.* }}` context.
    ///   Pass `None` in non-deploy contexts (init wizard, web API, sync diff).
    pub fn resolve(&self, data_root: Option<&std::path::Path>) -> Result<DesiredState> {
        // Pre-compute cross-service vars from all service entries.
        // Done once before resolution so every service can reference sibling services.
        let cross_vars = self.project.cross_service_vars();

        // Compute project_root: parent of data_root (e.g. "projects/fs-net/")
        // so {{ project_root }}/data/{{ instance_name }} expands correctly.
        let project_root_buf;
        let project_root: &str = match data_root {
            Some(dr) => {
                project_root_buf = dr
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                &project_root_buf
            }
            None => "",
        };

        let mut instances = Vec::new();
        let mut seen_names = HashMap::new();

        for (instance_name, module_ref) in &self.project.load.services {
            // Uniqueness check (per RULES.md: duplicate instance name = abort)
            if let Some(existing) =
                seen_names.insert(instance_name.clone(), module_ref.service_class.clone())
            {
                bail!(
                    "Duplicate service name '{}' in project '{}' (already defined as {})",
                    instance_name,
                    self.project.project.meta.name,
                    existing
                );
            }

            let instance = self
                .resolve_instance(
                    instance_name,
                    &module_ref.service_class,
                    &module_ref.env,
                    None, // no parent
                    &cross_vars,
                    project_root,
                )
                .with_context(|| format!("Resolving module '{}'", instance_name))?;

            instances.push(instance);
        }

        Ok(DesiredState {
            project_name: self.project.project.meta.name.clone(),
            domain: self.project.project.domain.clone(),
            services: instances,
        })
    }

    /// Resolve a single module instance (and its sub-modules recursively).
    fn resolve_instance(
        &self,
        name: &str,
        class_key: &str,
        instance_env: &indexmap::IndexMap<String, String>,
        parent_name: Option<&str>,
        cross_vars: &HashMap<String, String>,
        project_root: &str,
    ) -> Result<ServiceInstance> {
        let class = self
            .registry
            .get(class_key)
            .with_context(|| format!("Module class '{}' not found in registry", class_key))?
            .clone();

        let service_domain = format!("{}.{}", name, self.project.project.domain);
        let alias_domains: Vec<String> = class
            .meta
            .alias
            .iter()
            .map(|a| format!("{}.{}", a, self.project.project.domain))
            .collect();

        // Pre-compute [vars] block: render each var template with just the basic vars
        // (no module_vars self-reference). This gives us e.g. config_dir = "/projects/fs-net/data/zentinel".
        let module_vars = Self::precompute_module_vars(
            &class.vars,
            project_root,
            name,
            &self.project.project.meta.name,
            &self.project.project.domain,
        );

        // Collect plugin vars for proxy modules (dns_provider, acme_email, acme_ca_url, …).
        // For all other module types this is an empty map.
        let plugin_vars = PluginVars(if class_key.starts_with("proxy/") {
            self.host.plugin_vars(self.registry)
        } else {
            HashMap::new()
        });

        // Collect proxy service specs for proxy modules.
        // Proxy templates iterate over `proxy_services` to generate per-service routing config.
        let proxy_services = if class_key.starts_with("proxy/") {
            self.collect_proxy_services(project_root)
        } else {
            Vec::new()
        };

        // Build template context for Jinja2 expansion (includes cross-service vars)
        let ctx = TemplateContext {
            project_name: &self.project.project.meta.name,
            project_domain: &self.project.project.domain,
            instance_name: name,
            service_domain: &service_domain,
            parent_instance_name: parent_name.unwrap_or(name),
            project_root,
            vault: self.vault,
            cross_vars: CrossVars(cross_vars.clone()),
            module_vars: ModuleVars(module_vars),
            plugin_vars,
            proxy_services,
        };

        // Expand environment variables (module defaults + instance overrides)
        let mut resolved_env = Self::resolve_env(&class.environment, &ctx)?;
        // Instance-level env overrides take precedence over module defaults
        for (k, v) in instance_env {
            resolved_env.insert(k.clone(), v.clone());
        }

        // Expand volume mount strings ({{ module_vars.config_dir }}/data:/data:Z → real path)
        let container_volumes = class
            .container
            .as_ref()
            .map(|c| c.volumes.as_slice())
            .unwrap_or(&[]);
        let resolved_volumes = Self::resolve_volumes(container_volumes, &ctx)?;

        // Expand native app args ({{ module_vars.config_dir }}/... → real path)
        let raw_args = class
            .service
            .as_ref()
            .map(|s| s.args.as_slice())
            .unwrap_or(&[]);
        let resolved_args = raw_args
            .iter()
            .map(|a| {
                crate::template::render(a, &ctx).with_context(|| format!("Expanding arg '{}'", a))
            })
            .collect::<Result<Vec<String>>>()?;

        // Resolve sub-modules recursively (same cross_vars for the whole project)
        let mut sub_services = Vec::new();
        for (sub_name_tpl, sub_ref) in &class.load.sub_services {
            let sub_name = format!("{}-{}", name, sub_name_tpl);
            let sub = self
                .resolve_instance(
                    &sub_name,
                    &sub_ref.service_class,
                    &indexmap::IndexMap::new(),
                    Some(name),
                    cross_vars,
                    project_root,
                )
                .with_context(|| format!("Resolving sub-module '{}'", sub_name))?;
            sub_services.push(sub);
        }

        // Merge capability set: type defaults + plugin-declared extras.
        let mut capabilities: Vec<fs_node_core::config::Capability> = class
            .meta
            .service_types
            .iter()
            .flat_map(|t| t.capabilities())
            .collect();
        for cap in &class.meta.capabilities {
            if !capabilities.contains(cap) {
                capabilities.push(cap.clone());
            }
        }

        Ok(ServiceInstance {
            name: name.to_string(),
            class_key: class_key.to_string(),
            service_types: class.meta.service_types.clone(),
            version: class.meta.version.clone(),
            capabilities,
            class,
            resolved_env,
            resolved_volumes,
            resolved_args,
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

    /// Expand Jinja2 templates in volume mount strings.
    fn resolve_volumes(raw_volumes: &[String], ctx: &TemplateContext) -> Result<Vec<String>> {
        raw_volumes
            .iter()
            .map(|tpl| {
                crate::template::render(tpl, ctx)
                    .with_context(|| format!("Expanding volume '{}'", tpl))
            })
            .collect()
    }

    /// Collect proxy service specs from all services that have declared routes.
    ///
    /// Called when resolving a proxy module instance — provides `proxy_services`
    /// in the Jinja2 template context so proxy templates can iterate:
    ///   `{% for svc in proxy_services %}{{ svc.domain }} { ... }{% endfor %}`
    ///
    /// Services without `[contract.routes]` (e.g. databases, caches, the proxy itself)
    /// are excluded automatically.
    fn collect_proxy_services(
        &self,
        _project_root: &str,
    ) -> Vec<crate::template::ProxyServiceSpec> {
        let mut specs = Vec::new();

        for (instance_name, entry) in &self.project.load.services {
            let Some(class) = self.registry.get(&entry.service_class) else {
                continue;
            };

            // Skip services with no declared routes — nothing to proxy.
            if class.contract.routes.is_empty() {
                continue;
            }

            // Skip services that are purely internal infrastructure (Database, Cache).
            if class.meta.is_internal_only() {
                continue;
            }

            let subdomain = entry.subdomain.as_deref().unwrap_or(instance_name.as_str());
            let domain = format!("{}.{}", subdomain, self.project.project.domain);

            // Resolve container name (or use instance_name for native apps).
            let container = class
                .container
                .as_ref()
                .map(|c| {
                    c.name
                        .replace("{{ instance_name }}", instance_name)
                        .replace("{{ parent_instance_name }}", instance_name)
                })
                .unwrap_or_else(|| instance_name.to_string());

            let health_path = class
                .contract
                .health_path
                .clone()
                .or_else(|| class.meta.health_path.clone());

            specs.push(crate::template::ProxyServiceSpec {
                name: instance_name.clone(),
                domain,
                container,
                port: class.meta.port,
                routes: class.contract.routes.clone(),
                upstream_tls: class.contract.upstream_tls,
                health_path,
            });
        }

        specs
    }

    /// Pre-compute the [vars] block from a module class.
    ///
    /// Each var value is itself a Tera template (may reference `project_root`,
    /// `instance_name`, `project_name`, `project_domain`). We render them with a
    /// minimal context (no module_vars self-reference) to get concrete paths/strings.
    fn precompute_module_vars(
        vars: &indexmap::IndexMap<String, toml::Value>,
        project_root: &str,
        instance_name: &str,
        project_name: &str,
        project_domain: &str,
    ) -> HashMap<String, String> {
        use fs_template::{TemplateContext, TemplateEngine};

        let engine = TemplateEngine::new();
        let mut out = HashMap::new();

        // Build a minimal base context with the four root variables.
        let mut base = TemplateContext::new();
        base.set_str("project_root", project_root);
        base.set_str("instance_name", instance_name);
        base.set_str("project_name", project_name);
        base.set_str("project_domain", project_domain);

        for (key, val) in vars {
            let template_str = match val {
                toml::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let rendered = engine
                .render_str(&template_str, &base)
                .unwrap_or_else(|_| template_str.clone());
            out.insert(key.clone(), rendered);
        }
        out
    }
}

// ── Public shims ──────────────────────────────────────────────────────────────

pub fn resolve_desired(
    project: &ProjectConfig,
    host: &HostConfig,
    registry: &ServiceRegistry,
    vault: &VaultConfig,
    data_root: Option<&std::path::Path>,
) -> Result<DesiredState> {
    StateResolver::new(project, host, registry, vault).resolve(data_root)
}
