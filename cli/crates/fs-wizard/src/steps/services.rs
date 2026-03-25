// Service selection step — choose which services to deploy.

use super::WizardStep;

/// A service selected for deployment.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectedService {
    /// Service class path, e.g. "git/forgejo".
    pub class: String,
    /// Human-readable display name, e.g. "Forgejo".
    pub name: String,
}

impl SelectedService {
    /// Create a new `SelectedService`.
    pub fn new(class: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            class: class.into(),
            name: name.into(),
        }
    }
}

/// Instance deployment mode for a selected service.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum InstanceMode {
    /// Single standalone instance — no replication.
    #[default]
    Standalone,
    /// One of multiple worker replicas behind a load balancer.
    Worker,
    /// Mirror / read-replica of another instance.
    Mirror,
}

impl InstanceMode {
    /// Short display label.
    pub fn label(&self) -> &str {
        match self {
            Self::Standalone => "Standalone",
            Self::Worker => "Worker (replicated)",
            Self::Mirror => "Mirror (read-replica)",
        }
    }
}

/// Multi-instance deployment configuration for a service.
#[derive(Debug, Clone)]
pub struct MultiInstanceConfig {
    /// Deployment mode for this instance.
    pub mode: InstanceMode,
    /// Number of replicas (only relevant for `Worker` mode).
    pub replicas: u32,
}

impl Default for MultiInstanceConfig {
    fn default() -> Self {
        Self {
            mode: InstanceMode::Standalone,
            replicas: 1,
        }
    }
}

/// Input data for the service selection step.
#[derive(Debug, Clone, Default)]
pub struct ServicesInput {
    /// Services chosen for deployment.
    pub selected: Vec<SelectedService>,
    /// Multi-instance configuration for each selected service (keyed by class).
    pub instance_configs: std::collections::HashMap<String, MultiInstanceConfig>,
}

/// Wizard step that lets the user select services to deploy.
pub struct ServicesStep {
    /// All available service classes offered for selection.
    pub available: Vec<String>,
}

impl ServicesStep {
    /// Create a new `ServicesStep` with the given list of available service classes.
    pub fn new(available: Vec<String>) -> Self {
        Self { available }
    }

    /// Default set of well-known service classes shown in the wizard.
    pub fn default_available() -> Vec<String> {
        vec![
            "proxy/zentinel".into(),
            "iam/kanidm".into(),
            "mail/stalwart".into(),
            "git/forgejo".into(),
            "wiki/outline".into(),
            "chat/tuwunel".into(),
            "collab/cryptpad".into(),
            "tasks/vikunja".into(),
            "tickets/pretix".into(),
            "maps/umap".into(),
            "monitoring/openobserver".into(),
            "database/postgres".into(),
            "cache/dragonfly".into(),
        ]
    }
}

impl WizardStep for ServicesStep {
    type Input = ServicesInput;
    type Output = ServicesInput;

    fn title(&self) -> &str {
        "Service Selection"
    }

    fn validate(&self, input: &Self::Input) -> Vec<String> {
        let mut errors = Vec::new();

        for svc in &input.selected {
            if svc.class.trim().is_empty() {
                errors.push("A selected service has an empty class.".to_string());
            }
            // Validate replica count for Worker mode.
            if let Some(cfg) = input.instance_configs.get(&svc.class) {
                if cfg.mode == InstanceMode::Worker && cfg.replicas == 0 {
                    errors.push(format!(
                        "Service '{}': Worker mode requires at least 1 replica.",
                        svc.class
                    ));
                }
            }
        }

        errors
    }
}
