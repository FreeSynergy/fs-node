// IAM selection step — choose an identity provider.

use super::WizardStep;

/// Which identity / access management provider to deploy.
#[derive(Debug, Clone, PartialEq)]
pub enum IamChoice {
    /// Kanidm — recommended, modern Rust-native IdP.
    Kanidm,
    /// Keycloak — Java-based, feature-rich enterprise IdP.
    Keycloak,
    /// Authentik — Python-based, flexible IdP.
    Authentik,
    /// LLDAP — lightweight LDAP server.
    Lldap,
    /// External — use an existing IdP at the given URL.
    External { url: String },
    /// No IAM — skip identity provider setup.
    None,
}

impl IamChoice {
    /// Short display label used in menus and summaries.
    pub fn label(&self) -> &str {
        match self {
            Self::Kanidm => "Kanidm ⭐ (recommended)",
            Self::Keycloak => "Keycloak",
            Self::Authentik => "Authentik",
            Self::Lldap => "LLDAP",
            Self::External { .. } => "External IdP",
            Self::None => "No IAM",
        }
    }

    /// Returns `true` if this choice is the recommended default.
    pub fn is_recommended(&self) -> bool {
        *self == Self::Kanidm
    }

    /// Returns the service class ID used in the store (if any).
    pub fn service_class(&self) -> Option<&'static str> {
        match self {
            Self::Kanidm => Some("iam/kanidm"),
            Self::Keycloak => Some("iam/keycloak"),
            Self::Authentik => Some("iam/authentik"),
            Self::Lldap => Some("iam/lldap"),
            _ => None,
        }
    }
}

/// Input data for the IAM selection step.
#[derive(Debug, Clone)]
pub struct IamInput {
    /// The chosen IAM provider.
    pub choice: IamChoice,
}

impl Default for IamInput {
    fn default() -> Self {
        Self {
            choice: IamChoice::Kanidm,
        }
    }
}

/// Wizard step that selects an identity provider.
pub struct IamStep;

impl IamStep {
    /// Create a new `IamStep`.
    pub fn new() -> Self {
        Self
    }

    /// Returns all available `IamChoice` variants (excluding the `External` variant).
    /// The External variant must be constructed with a URL by the UI.
    pub fn choices() -> Vec<IamChoice> {
        vec![
            IamChoice::Kanidm,
            IamChoice::Keycloak,
            IamChoice::Authentik,
            IamChoice::Lldap,
            IamChoice::External { url: String::new() },
            IamChoice::None,
        ]
    }
}

impl Default for IamStep {
    fn default() -> Self {
        Self::new()
    }
}

impl WizardStep for IamStep {
    type Input = IamInput;
    type Output = IamInput;

    fn title(&self) -> &str {
        "Identity & Access Management"
    }

    fn validate(&self, input: &Self::Input) -> Vec<String> {
        let mut errors = Vec::new();
        if let IamChoice::External { url } = &input.choice {
            if url.trim().is_empty() {
                errors.push("External IdP URL is required.".to_string());
            } else if !url.starts_with("https://") && !url.starts_with("http://") {
                errors.push("External IdP URL must start with http:// or https://".to_string());
            }
        }
        errors
    }
}
