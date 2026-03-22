// Template rendering via fs-template (Tera engine).
//
// FSN-specific context (`FsTemplateContext`) holds all the domain fields
// and converts to `fs_template::TemplateContext` for rendering.
// `ProxyServiceSpec` is an FSN-specific data type used by proxy module templates.
//
// OOP design:
//   `InjectIntoContext` trait — each var group carries its own injection logic.
//   `CrossVars`  — lowercased + flat-injected cross-service variables.
//   `ModuleVars` — injected as nested `module_vars.*` object.
//   `PluginVars` — flat-injected proxy plugin variables.

use std::collections::HashMap;

use anyhow::Result;
use serde::Serialize;

use fs_template::{TemplateContext as LibCtx, TemplateEngine};

/// FSN-level alias for callers within this crate.
pub type TemplateContext<'a> = FsTemplateContext<'a>;
use fs_node_core::config::{RouteSpec, VaultConfig};

// ── InjectIntoContext ─────────────────────────────────────────────────────────

/// Trait for variable groups that know how to inject themselves into a
/// [`LibCtx`] template context.
///
/// Each implementor encapsulates its own injection strategy so the caller
/// does not need to know whether variables are lowercased, nested, or flat.
pub trait InjectIntoContext {
    fn inject(&self, ctx: &mut LibCtx) -> Result<()>;
}

// ── CrossVars ─────────────────────────────────────────────────────────────────

/// Cross-service variables (e.g. `MAIL_HOST`, `IAM_URL`).
///
/// Injected as **lowercase** flat top-level template variables so Jinja2
/// templates can use `{{ mail_host }}`, `{{ iam_url }}`, etc.
#[derive(Debug, Clone, Default)]
pub struct CrossVars(pub HashMap<String, String>);

impl InjectIntoContext for CrossVars {
    fn inject(&self, ctx: &mut LibCtx) -> Result<()> {
        let lower: HashMap<String, String> = self.0.iter()
            .map(|(k, v)| (k.to_lowercase(), v.clone()))
            .collect();
        ctx.merge_str_map(&lower);
        Ok(())
    }
}

// ── ModuleVars ────────────────────────────────────────────────────────────────

/// Pre-computed `[vars]` block from the module TOML.
///
/// Injected as a **nested object** under `module_vars` so templates can use
/// `{{ module_vars.config_dir }}`, `{{ module_vars.data_dir }}`, etc.
#[derive(Debug, Clone, Default)]
pub struct ModuleVars(pub HashMap<String, String>);

impl InjectIntoContext for ModuleVars {
    fn inject(&self, ctx: &mut LibCtx) -> Result<()> {
        ctx.set("module_vars", &self.0).map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }
}

// ── PluginVars ────────────────────────────────────────────────────────────────

/// Proxy plugin variables (e.g. `dns_provider`, `acme_email`, `acme_ca_url`).
///
/// Injected as **flat** top-level template variables.
#[derive(Debug, Clone, Default)]
pub struct PluginVars(pub HashMap<String, String>);

impl InjectIntoContext for PluginVars {
    fn inject(&self, ctx: &mut LibCtx) -> Result<()> {
        ctx.merge_str_map(&self.0);
        Ok(())
    }
}

// ── ProxyServiceSpec ──────────────────────────────────────────────────────────

/// One service that needs proxy routing.
///
/// Derived from the service's `ServiceContract` at resolve time.
/// Proxy module templates iterate over `proxy_services` to generate routing config.
#[derive(Debug, Clone, Serialize)]
pub struct ProxyServiceSpec {
    /// Instance name (e.g. `"kanidm"`, `"outline"`).
    pub name: String,
    /// Full service domain (e.g. `"kanidm.example.com"`).
    pub domain: String,
    /// Resolved container name (e.g. `"kanidm"`).
    pub container: String,
    /// Primary internal port.
    pub port: u16,
    /// Routes declared in the service's `[contract]` block.
    pub routes: Vec<RouteSpec>,
    /// Whether the upstream (container) uses TLS internally.
    pub upstream_tls: bool,
    /// Proxy health-check path (from `contract.health_path` or `module.health_path`).
    pub health_path: Option<String>,
}

// ── FsTemplateContext ────────────────────────────────────────────────────────

/// FSN-specific template rendering context.
///
/// Holds all domain-level fields and converts to [`LibCtx`] for rendering.
/// Each variable group is a typed object that carries its own injection logic
/// via [`InjectIntoContext`] — `to_fsn()` just calls `inject()` on each.
pub struct FsTemplateContext<'a> {
    /// Project short name (e.g. `"fs-net"`).
    pub project_name: &'a str,
    /// Primary domain (e.g. `"example.com"`).
    pub project_domain: &'a str,
    /// Instance name (e.g. `"zentinel"`).
    pub instance_name: &'a str,
    /// Fully qualified service domain.
    pub service_domain: &'a str,
    /// Parent instance name (same as `instance_name` for top-level services).
    pub parent_instance_name: &'a str,
    /// Filesystem root of the project (parent of the `data/` directory).
    pub project_root: &'a str,
    /// Vault configuration (for injecting `vault_*` secrets).
    pub vault: &'a VaultConfig,
    /// Cross-service and project-level variables from `VarProvider` exports.
    pub cross_vars: CrossVars,
    /// Pre-computed `[vars]` block from the module TOML.
    pub module_vars: ModuleVars,
    /// Expanded plugin vars (dns_provider, acme_email, acme_ca_url, …).
    pub plugin_vars: PluginVars,
    /// Services that need proxy routing — available as `{{ proxy_services }}`.
    pub proxy_services: Vec<ProxyServiceSpec>,
}

impl<'a> FsTemplateContext<'a> {
    /// Convert to a [`LibCtx`] ready for rendering.
    ///
    /// Each variable group injects itself via [`InjectIntoContext`] — no
    /// injection strategy is hard-coded here.
    pub fn to_fsn(&self) -> Result<LibCtx> {
        let mut ctx = LibCtx::new();

        ctx.set_str("project_name",         self.project_name);
        ctx.set_str("project_domain",        self.project_domain);
        ctx.set_str("instance_name",         self.instance_name);
        ctx.set_str("service_domain",        self.service_domain);
        ctx.set_str("parent_instance_name",  self.parent_instance_name);
        ctx.set_str("project_root",          self.project_root);

        self.module_vars.inject(&mut ctx)?;
        self.cross_vars.inject(&mut ctx)?;
        self.plugin_vars.inject(&mut ctx)?;

        ctx.set("proxy_services", &self.proxy_services)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        for key in self.vault.keys() {
            if let Some(value) = self.vault.expose(key) {
                ctx.set_str(key, value);
            }
        }

        Ok(ctx)
    }
}

// ── Render helpers ────────────────────────────────────────────────────────────

/// Render a single Jinja2/Tera template string with the given FSN context.
pub fn render(template: &str, ctx: &FsTemplateContext) -> Result<String> {
    let engine = TemplateEngine::new();
    let lib_ctx = ctx.to_fsn()?;
    engine.render_str(template, &lib_ctx)
        .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Render a multi-line template file (e.g. `container.quadlet.j2`).
pub fn render_file(template_content: &str, ctx: &FsTemplateContext) -> Result<String> {
    render(template_content, ctx)
}

