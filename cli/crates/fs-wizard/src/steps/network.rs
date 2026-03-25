// Network setup step — collect hostname, domain, and IP address.

use super::WizardStep;

/// Input data for the network setup step.
#[derive(Debug, Clone, Default)]
pub struct NetworkInput {
    /// Short hostname for this node (e.g. "node1").
    pub hostname: String,
    /// Primary domain for services (e.g. "example.com").
    pub domain: String,
    /// Public IP address of this node (IPv4 or IPv6).
    pub ip: String,
}

/// Wizard step that collects network configuration.
pub struct NetworkStep;

impl NetworkStep {
    /// Create a new `NetworkStep`.
    pub fn new() -> Self {
        Self
    }

    /// Static title (useful when you don't have a step instance).
    pub fn title() -> &'static str {
        "Network Setup"
    }

    /// Validate a `NetworkInput` without a step instance.
    pub fn validate(input: &NetworkInput) -> Vec<String> {
        let mut errors = Vec::new();

        if input.hostname.trim().is_empty() {
            errors.push("Hostname is required.".to_string());
        } else if input.hostname.contains(' ') {
            errors.push("Hostname must not contain spaces.".to_string());
        }

        if input.domain.trim().is_empty() {
            errors.push("Domain is required.".to_string());
        } else if !input.domain.contains('.') {
            errors.push("Domain must contain at least one dot (e.g. example.com).".to_string());
        }

        if input.ip.trim().is_empty() {
            errors.push("IP address is required.".to_string());
        } else if !is_valid_ip(&input.ip) {
            errors.push(format!(
                "'{}' is not a valid IPv4 or IPv6 address.",
                input.ip
            ));
        }

        errors
    }
}

impl Default for NetworkStep {
    fn default() -> Self {
        Self::new()
    }
}

impl WizardStep for NetworkStep {
    type Input = NetworkInput;
    type Output = NetworkInput;

    fn title(&self) -> &str {
        "Network Setup"
    }

    fn validate(&self, input: &Self::Input) -> Vec<String> {
        NetworkStep::validate(input)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns `true` if `s` is a syntactically valid IPv4 or IPv6 address.
fn is_valid_ip(s: &str) -> bool {
    s.parse::<std::net::IpAddr>().is_ok()
}
