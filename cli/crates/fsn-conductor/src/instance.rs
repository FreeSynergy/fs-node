// Instance name handling (F7).
//
// An instance name identifies a deployed compose stack uniquely on the host.
// Rules:
//  - Default: the name of the main service (first service in the compose file)
//  - Custom: user-supplied via --name flag
//  - Must be lowercase alphanumeric + hyphens only
//  - Max 48 chars
//  - Prefix is prepended to all sub-service names: "{instance}-{svc}"

use anyhow::{bail, Result};

use crate::compose::ComposeFile;

// ── InstanceName ──────────────────────────────────────────────────────────────

/// A validated instance name for a conductor-managed compose stack.
#[derive(Debug, Clone)]
pub struct InstanceName(String);

impl InstanceName {
    /// Create from user-supplied string, validating format.
    pub fn from_str(s: &str) -> Result<Self> {
        let s = s.trim().to_lowercase();
        validate_name(&s)?;
        Ok(Self(s))
    }

    /// Derive the default instance name from a compose file.
    ///
    /// Uses the first service name (= "main service") as the instance name.
    pub fn from_compose(compose: &ComposeFile) -> Result<Self> {
        let name = compose
            .services
            .keys()
            .next()
            .ok_or_else(|| anyhow::anyhow!("compose file has no services"))?;
        let normalised = normalise(name);
        validate_name(&normalised)?;
        Ok(Self(normalised))
    }

    /// The instance name string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Build the prefixed name for a sub-service.
    ///
    /// If `svc_name` equals the instance name itself (single-service compose),
    /// no prefix is added. Otherwise returns `"{instance}-{svc_name}"`.
    pub fn service_name(&self, svc_name: &str) -> String {
        let svc = normalise(svc_name);
        if svc == self.0 {
            svc
        } else {
            format!("{}-{svc}", self.0)
        }
    }
}

impl std::fmt::Display for InstanceName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Validation ────────────────────────────────────────────────────────────────

fn validate_name(s: &str) -> Result<()> {
    if s.is_empty() {
        bail!("instance name must not be empty");
    }
    if s.len() > 48 {
        bail!("instance name too long (max 48 chars): {s}");
    }
    if !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        bail!("instance name must contain only [a-z0-9-]: {s}");
    }
    if s.starts_with('-') || s.ends_with('-') {
        bail!("instance name must not start or end with '-': {s}");
    }
    Ok(())
}

/// Normalise a service name to a valid instance name slug.
fn normalise(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::parse_str;

    #[test]
    fn valid_name() {
        assert!(InstanceName::from_str("kanidm").is_ok());
        assert!(InstanceName::from_str("my-app-42").is_ok());
    }

    #[test]
    fn rejects_uppercase() {
        // from_str lowercases, so it should pass
        assert!(InstanceName::from_str("Kanidm").is_ok());
    }

    #[test]
    fn rejects_leading_hyphen() {
        assert!(InstanceName::from_str("-bad").is_err());
    }

    #[test]
    fn rejects_too_long() {
        let long = "a".repeat(49);
        assert!(InstanceName::from_str(&long).is_err());
    }

    #[test]
    fn from_compose_uses_first_service() {
        let yaml = r#"
services:
  kanidm:
    image: kanidm/server:latest
  db:
    image: postgres:16
"#;
        let f = parse_str(yaml).unwrap();
        let name = InstanceName::from_compose(&f).unwrap();
        assert_eq!(name.as_str(), "kanidm");
    }

    #[test]
    fn service_name_no_double_prefix() {
        let n = InstanceName::from_str("kanidm").unwrap();
        assert_eq!(n.service_name("kanidm"), "kanidm");
    }

    #[test]
    fn service_name_prefixed() {
        let n = InstanceName::from_str("kanidm").unwrap();
        assert_eq!(n.service_name("db"), "kanidm-db");
    }
}
