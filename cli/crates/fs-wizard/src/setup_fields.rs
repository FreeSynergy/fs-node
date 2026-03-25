// setup_fields.rs — Generate setup field suggestions from module metadata.
//
// Given a detected service class (e.g. "git/forgejo"), returns the common
// setup fields an operator must fill in before deployment.
//
// These mirror the `[[setup.fields]]` entries in the module TOML definitions
// in the store, providing a static preview when the store is not available.

// ── SetupField ────────────────────────────────────────────────────────────────

/// A single required setup field for a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupField {
    /// Variable key (e.g. `"vault_forgejo_admin_password"`).
    pub key: String,
    /// Short human-readable label.
    pub label: String,
    /// Input type hint for the UI.
    pub field_type: SetupFieldType,
    /// Optional extended description shown below the input.
    pub description: Option<String>,
}

/// Input type hint for a setup field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupFieldType {
    /// Plain text — shown in clear text.
    Text,
    /// Secret — masked in UI, stored in vault.
    Secret,
    /// Email address.
    Email,
    /// Domain name (e.g. `git.example.com`).
    Domain,
}

impl SetupField {
    fn text(key: &str, label: &str, description: &str) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            field_type: SetupFieldType::Text,
            description: Some(description.into()),
        }
    }

    fn secret(key: &str, label: &str, description: &str) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            field_type: SetupFieldType::Secret,
            description: Some(description.into()),
        }
    }

    fn email(key: &str, label: &str, description: &str) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            field_type: SetupFieldType::Email,
            description: Some(description.into()),
        }
    }

    fn domain(key: &str, label: &str, description: &str) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            field_type: SetupFieldType::Domain,
            description: Some(description.into()),
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Return the common setup fields for a given service class.
///
/// `class` is the FSN class string, e.g. `"git/forgejo"`, `"mail/stalwart"`.
/// Matches on the primary type prefix (before `/`).
///
/// Returns an empty slice when no fields are known (e.g. database, cache).
pub fn setup_fields_for(class: &str) -> Vec<SetupField> {
    let primary = class.split('/').next().unwrap_or(class);
    match primary {
        "proxy" => proxy_fields(),
        "mail" => mail_fields(),
        "git" => git_fields(),
        "wiki" => wiki_fields(),
        "iam" => iam_fields(),
        "chat" => chat_fields(),
        "collab" => collab_fields(),
        "tasks" => tasks_fields(),
        "tickets" => tickets_fields(),
        "maps" => maps_fields(),
        "monitoring" => monitoring_fields(),
        _ => vec![],
    }
}

// ── Per-type field definitions ────────────────────────────────────────────────

fn proxy_fields() -> Vec<SetupField> {
    vec![SetupField::email(
        "acme_email",
        "ACME email address",
        "Email address used to register TLS certificates via Let's Encrypt.",
    )]
}

fn mail_fields() -> Vec<SetupField> {
    vec![
        SetupField::domain(
            "mail_domain",
            "Primary mail domain",
            "The domain used for outgoing mail, e.g. 'example.com'.",
        ),
        SetupField::secret(
            "vault_mail_admin_password",
            "Mail admin password",
            "Password for the postmaster / admin account.",
        ),
        SetupField::text(
            "smtp_hostname",
            "SMTP hostname",
            "Fully qualified hostname for the mail server, e.g. 'mail.example.com'.",
        ),
    ]
}

fn git_fields() -> Vec<SetupField> {
    vec![
        SetupField::text(
            "git_admin_user",
            "Admin username",
            "Username for the initial admin account created on first start.",
        ),
        SetupField::email(
            "git_admin_email",
            "Admin email",
            "Email address for the initial admin account.",
        ),
        SetupField::secret(
            "vault_git_admin_password",
            "Admin password",
            "Password for the initial admin account.",
        ),
    ]
}

fn wiki_fields() -> Vec<SetupField> {
    vec![
        SetupField::secret(
            "vault_wiki_secret_key",
            "App secret key",
            "Random secret key used for session encryption (generate with `openssl rand -hex 32`).",
        ),
        SetupField::email(
            "wiki_admin_email",
            "Admin email",
            "Email address for the initial admin account.",
        ),
    ]
}

fn iam_fields() -> Vec<SetupField> {
    vec![
        SetupField::secret(
            "vault_kanidm_admin_password",
            "Admin password",
            "Password for the 'admin' account. Set after first start with `kanidm recover-account admin`.",
        ),
        SetupField::secret(
            "vault_kanidm_idm_admin_password",
            "IDM admin password",
            "Password for the 'idm_admin' account.",
        ),
    ]
}

fn chat_fields() -> Vec<SetupField> {
    vec![
        SetupField::secret(
            "vault_matrix_registration_secret",
            "Registration shared secret",
            "Secret used for server-side user registration. Generate with `openssl rand -hex 32`.",
        ),
        SetupField::text(
            "matrix_server_name",
            "Matrix server name",
            "The base server name for Matrix IDs, e.g. 'example.com' (results in @user:example.com).",
        ),
    ]
}

fn collab_fields() -> Vec<SetupField> {
    vec![SetupField::secret(
        "vault_cryptpad_session_key",
        "Session signing key",
        "Secret key for signing user sessions. Generate with `openssl rand -hex 64`.",
    )]
}

fn tasks_fields() -> Vec<SetupField> {
    vec![
        SetupField::secret(
            "vault_vikunja_jwt_secret",
            "JWT secret",
            "Secret for signing JWT tokens. Generate with `openssl rand -hex 32`.",
        ),
        SetupField::email(
            "vikunja_service_jwt_secret",
            "Admin email",
            "Email address for the initial admin account.",
        ),
    ]
}

fn tickets_fields() -> Vec<SetupField> {
    vec![
        SetupField::email(
            "pretix_admin_email",
            "Admin email",
            "Email address for the superuser account created on first start.",
        ),
        SetupField::secret(
            "vault_pretix_secret_key",
            "App secret key",
            "Django secret key for session encryption.",
        ),
    ]
}

fn maps_fields() -> Vec<SetupField> {
    vec![
        SetupField::text(
            "umap_site_name",
            "Site name",
            "Display name for the uMap instance, e.g. 'FreeSynergy Maps'.",
        ),
        SetupField::secret(
            "vault_umap_secret_key",
            "App secret key",
            "Django secret key for session encryption.",
        ),
    ]
}

fn monitoring_fields() -> Vec<SetupField> {
    vec![
        SetupField::text(
            "monitoring_admin_user",
            "Admin username",
            "Username for the initial admin account.",
        ),
        SetupField::secret(
            "vault_monitoring_admin_password",
            "Admin password",
            "Password for the initial admin account.",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_returns_three_fields() {
        let fields = setup_fields_for("git/forgejo");
        assert_eq!(fields.len(), 3);
        assert!(fields.iter().any(|f| f.key == "vault_git_admin_password"));
    }

    #[test]
    fn database_returns_empty() {
        assert!(setup_fields_for("database/postgres").is_empty());
    }

    #[test]
    fn proxy_returns_email_field() {
        let fields = setup_fields_for("proxy/zentinel");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_type, SetupFieldType::Email);
    }

    #[test]
    fn cache_returns_empty() {
        assert!(setup_fields_for("cache/dragonfly").is_empty());
    }
}
