// Project-specific form — uses #[derive(Form)] to declare the schema once.
//
// The schema drives form generation automatically:
//   ProjectFormData::schema() → FormSchema (static, generated at compile time)
//   schema_form::build_nodes(schema, prefill, display_fns, dynamics) → Vec<Box<dyn FormNode>>
//
// To add a new field: add it here with #[form(...)] attributes.
// No changes needed in events.rs, new_project.rs, or anywhere else.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use fsn_form::Form;

use crate::app::{ProjectHandle, ResourceForm, ResourceKind, PROJECT_TABS};
use crate::schema_form;
use crate::ui::form_node::FormNode;

// ── Form data struct ──────────────────────────────────────────────────────────

/// Form schema for creating and editing a Project.
///
/// Each field maps to a `FieldMeta` in the generated `FormSchema`.
/// The actual domain struct (`ProjectMeta`) stays clean — no UI concerns.
#[derive(Form)]
pub struct ProjectFormData {
    #[form(label = "form.project.name", required, tab = 0, hint = "form.project.name.hint", max_len = 255)]
    pub name: String,

    #[form(label = "form.project.domain", required, tab = 0, hint = "form.project.domain.hint")]
    pub domain: String,

    #[form(label = "form.project.description", tab = 0, hint = "form.project.description.hint")]
    pub description: String,

    #[form(label = "form.project.email", required, tab = 0, widget = "email", hint = "form.project.email.hint")]
    pub contact_email: String,

    #[form(label = "form.options.language", widget = "select", tab = 1,
           options = "de,en,fr,es,it,pt", default = "de")]
    pub language: String,

    #[form(label = "form.project.path", required, tab = 1, hint = "form.project.path.hint")]
    pub install_dir: String,

    #[form(label = "form.options.version", tab = 1, default = "0.1.0")]
    pub version: String,
}

// ── Display helpers ───────────────────────────────────────────────────────────

pub fn lang_display(code: &str) -> &'static str {
    match code {
        "de" => "Deutsch",
        "en" => "English",
        "fr" => "Français",
        "es" => "Español",
        "it" => "Italiano",
        "pt" => "Português",
        _    => "—",
    }
}

const DISPLAY_FNS: &[(&str, fn(&str) -> &'static str)] = &[
    ("language", lang_display),
];

// ── Smart-defaults hook ───────────────────────────────────────────────────────

/// Derives domain from name and contact_email from domain automatically.
pub fn project_on_change(nodes: &mut Vec<Box<dyn FormNode>>, key: &'static str) {
    match key {
        "name" => {
            let name_val = nodes.iter().find(|n| n.key() == "name")
                .map(|n| n.value().to_string()).unwrap_or_default();
            let slug = crate::app::slugify(&name_val);

            let domain_dirty = nodes.iter().find(|n| n.key() == "domain")
                .map(|n| n.is_dirty()).unwrap_or(false);
            if !domain_dirty {
                if let Some(n) = nodes.iter_mut().find(|n| n.key() == "domain") {
                    n.set_value(&slug);
                }
            }
            sync_email_from_domain(nodes);
        }
        "domain" => sync_email_from_domain(nodes),
        _ => {}
    }
}

fn sync_email_from_domain(nodes: &mut Vec<Box<dyn FormNode>>) {
    let domain = nodes.iter().find(|n| n.key() == "domain")
        .map(|n| n.value().to_string()).unwrap_or_default();
    if domain.is_empty() { return; }
    let email_dirty = nodes.iter().find(|n| n.key() == "contact_email")
        .map(|n| n.is_dirty()).unwrap_or(false);
    if !email_dirty {
        if let Some(n) = nodes.iter_mut().find(|n| n.key() == "contact_email") {
            n.set_value(&format!("admin@{}", domain));
        }
    }
}

// ── Form builders ─────────────────────────────────────────────────────────────

pub fn new_project_form() -> ResourceForm {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".into());
    let dynamics: &[(&str, String)] = &[
        ("install_dir", format!("{}/fsn", home)),
    ];
    let nodes = schema_form::build_nodes(
        ProjectFormData::schema(),
        &HashMap::new(),
        DISPLAY_FNS,
        dynamics,
    );
    ResourceForm::new(ResourceKind::Project, PROJECT_TABS, nodes, None, project_on_change)
}

pub fn edit_project_form(handle: &ProjectHandle) -> ResourceForm {
    let p    = &handle.config.project;
    let desc = p.description.as_deref().unwrap_or("").to_string();
    let prefill: HashMap<&str, &str> = [
        ("name",          p.name.as_str()),
        ("domain",        p.domain.as_str()),
        ("description",   desc.as_str()),
        ("contact_email", handle.email()),
        ("language",      p.language.as_str()),
        ("install_dir",   handle.install_dir()),
        ("version",       p.version.as_str()),
    ].into_iter().filter(|(_, v)| !v.is_empty()).collect();

    let nodes = schema_form::build_nodes(
        ProjectFormData::schema(),
        &prefill,
        DISPLAY_FNS,
        &[],
    );
    ResourceForm::new(ResourceKind::Project, PROJECT_TABS, nodes, Some(handle.slug.clone()), project_on_change)
}

// ── Submit ────────────────────────────────────────────────────────────────────

pub fn submit_project_form(form: &ResourceForm, root: &Path) -> Result<()> {
    let is_edit = form.edit_id.is_some();
    let slug = form.edit_id.clone()
        .unwrap_or_else(|| crate::app::slugify(&form.field_value("name")));

    let project_dir = root.join("projects").join(&slug);
    std::fs::create_dir_all(&project_dir)?;

    let toml_path = project_dir.join(format!("{}.project.toml", slug));
    if !is_edit && toml_path.exists() { return Ok(()); }

    let name    = form.field_value("name");
    let domain  = form.field_value("domain");
    let desc    = form.field_value("description");
    let email   = form.field_value("contact_email");
    let lang    = form.field_value("language");
    let path    = form.field_value("install_dir");
    let version = form.field_value("version");

    let content = format!(
        "[project]\nname        = \"{name}\"\ndomain      = \"{domain}\"\ndescription = \"{desc}\"\nemail       = \"{email}\"\nlanguage    = \"{lang}\"\ninstall_dir = \"{path}\"\nversion     = \"{version}\"\n"
    );
    std::fs::write(&toml_path, content)?;
    Ok(())
}
