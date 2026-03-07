// Jinja2-compatible template engine – wraps minijinja.
//
// The existing .j2 templates in playbooks/templates/ work unchanged.
// Variable names match the Ansible template context (instance_name,
// project_root, service_domain, vault_*, ...).

use anyhow::Result;
use minijinja::{context, Environment};

use fsn_core::config::VaultConfig;

/// Template rendering context – mirrors the Ansible variable namespace.
pub struct TemplateContext<'a> {
    pub project_name: &'a str,
    pub project_domain: &'a str,
    pub instance_name: &'a str,
    pub service_domain: &'a str,
    pub parent_instance_name: &'a str,
    pub vault: &'a VaultConfig,
}

/// Render a single Jinja2 template string with the given context.
pub fn render(template: &str, ctx: &TemplateContext) -> Result<String> {
    let mut env = Environment::new();

    // Build variable map from context
    let mut vars = minijinja::Value::from_iter([
        ("project_name", minijinja::Value::from(ctx.project_name)),
        ("project_domain", minijinja::Value::from(ctx.project_domain)),
        ("instance_name", minijinja::Value::from(ctx.instance_name)),
        ("service_domain", minijinja::Value::from(ctx.service_domain)),
        ("parent_instance_name", minijinja::Value::from(ctx.parent_instance_name)),
    ]);

    // Inject vault secrets into the template context
    // (only keys – values exposed by minijinja during render, not stored as strings)
    if let minijinja::Value::Map(ref mut map) = vars {
        for key in ctx.vault.keys() {
            if let Some(exposed) = ctx.vault.expose(key) {
                map.insert(
                    minijinja::Value::from(key),
                    minijinja::Value::from(exposed.to_string()),
                );
            }
        }
    }

    let tmpl = env.template_from_str(template)?;
    Ok(tmpl.render(vars)?)
}

/// Render a multi-line template file (e.g. container.quadlet.j2).
pub fn render_file(template_content: &str, ctx: &TemplateContext) -> Result<String> {
    render(template_content, ctx)
}
