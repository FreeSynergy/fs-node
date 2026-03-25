// Capability matcher — resolve service capabilities to concrete URLs.

use std::collections::HashMap;

// ── CapabilityBinding ─────────────────────────────────────────────────────────

/// Maps a single capability (e.g. "iam", "mail") to a deployed service and its URL.
#[derive(Debug, Clone)]
pub struct CapabilityBinding {
    /// Capability identifier, e.g. "iam", "mail", "git".
    pub capability: String,
    /// Name of the service fulfilling this capability, e.g. "kanidm".
    pub service_name: String,
    /// Base URL where the service is reachable, e.g. "https://auth.example.com".
    pub url: String,
}

impl CapabilityBinding {
    /// Create a new `CapabilityBinding`.
    pub fn new(
        capability: impl Into<String>,
        service_name: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        Self {
            capability: capability.into(),
            service_name: service_name.into(),
            url: url.into(),
        }
    }
}

// ── CapabilityMatcher ─────────────────────────────────────────────────────────

/// Matches running services to required role variables.
///
/// Used during deployment to auto-fill role-variable placeholders in service
/// configuration templates (e.g. `{{iam_url}}`, `{{mail_host}}`).
#[derive(Debug, Clone, Default)]
pub struct CapabilityMatcher {
    /// All known capability-to-service bindings.
    pub bindings: Vec<CapabilityBinding>,
}

impl CapabilityMatcher {
    /// Create an empty `CapabilityMatcher`.
    pub fn new() -> Self {
        Self { bindings: vec![] }
    }

    /// Register a binding from `capability` to a service at `url`.
    pub fn add_binding(&mut self, capability: &str, service_name: &str, url: &str) {
        self.bindings
            .push(CapabilityBinding::new(capability, service_name, url));
    }

    /// Return the first binding that satisfies `capability`, if any.
    ///
    /// When multiple services fulfill the same capability the first registered
    /// one wins. Override ordering by inserting preferred bindings first.
    pub fn resolve(&self, capability: &str) -> Option<&CapabilityBinding> {
        self.bindings.iter().find(|b| b.capability == capability)
    }

    /// Auto-fill role variables for a service from its list of required capabilities.
    ///
    /// For each capability in `required_capabilities`, the matcher looks up the
    /// corresponding binding and inserts two entries into the returned map:
    ///
    /// - `{capability}_url`          → the service URL
    /// - `{capability}_service`      → the service name
    ///
    /// Capabilities that have no registered binding are silently skipped.
    pub fn auto_fill(&self, required_capabilities: &[&str]) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        for cap in required_capabilities {
            if let Some(binding) = self.resolve(cap) {
                vars.insert(format!("{}_url", cap), binding.url.clone());
                vars.insert(format!("{}_service", cap), binding.service_name.clone());
            }
        }
        vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_finds_first_match() {
        let mut m = CapabilityMatcher::new();
        m.add_binding("iam", "kanidm", "https://auth.example.com");
        m.add_binding("mail", "stalwart", "https://mail.example.com");

        let b = m.resolve("iam").unwrap();
        assert_eq!(b.service_name, "kanidm");
        assert_eq!(b.url, "https://auth.example.com");

        assert!(m.resolve("git").is_none());
    }

    #[test]
    fn auto_fill_returns_url_and_service_vars() {
        let mut m = CapabilityMatcher::new();
        m.add_binding("iam", "kanidm", "https://auth.example.com");
        m.add_binding("mail", "stalwart", "https://mail.example.com");

        let vars = m.auto_fill(&["iam", "mail", "git"]);

        assert_eq!(
            vars.get("iam_url"),
            Some(&"https://auth.example.com".to_string())
        );
        assert_eq!(vars.get("iam_service"), Some(&"kanidm".to_string()));
        assert_eq!(
            vars.get("mail_url"),
            Some(&"https://mail.example.com".to_string())
        );
        assert_eq!(vars.get("mail_service"), Some(&"stalwart".to_string()));
        // "git" has no binding — should be absent
        assert!(!vars.contains_key("git_url"));
    }
}
