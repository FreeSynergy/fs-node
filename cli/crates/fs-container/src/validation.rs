// Dry-run validation — checks a ComposeFile for common problems.
//
// Returns a list of ValidationIssues. An empty list means the file is valid.
// Does NOT make network calls (offline-safe).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::compose::ComposeFile;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueLevel {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub level: IssueLevel,
    /// Service name, if the issue relates to a specific service.
    pub service: Option<String>,
    pub message: String,
}

impl ValidationIssue {
    fn error(service: Option<&str>, msg: impl Into<String>) -> Self {
        Self {
            level: IssueLevel::Error,
            service: service.map(str::to_string),
            message: msg.into(),
        }
    }

    fn warning(service: Option<&str>, msg: impl Into<String>) -> Self {
        Self {
            level: IssueLevel::Warning,
            service: service.map(str::to_string),
            message: msg.into(),
        }
    }

    fn info(service: Option<&str>, msg: impl Into<String>) -> Self {
        Self {
            level: IssueLevel::Info,
            service: service.map(str::to_string),
            message: msg.into(),
        }
    }
}

/// Result of a dry-run validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        !self.issues.iter().any(|i| i.level == IssueLevel::Error)
    }

    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.level == IssueLevel::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.level == IssueLevel::Warning)
            .count()
    }

    /// Print a human-readable summary to stdout.
    pub fn print_report(&self) {
        if self.issues.is_empty() {
            println!("✅ No issues found.");
            return;
        }
        for issue in &self.issues {
            let icon = match issue.level {
                IssueLevel::Error => "❌",
                IssueLevel::Warning => "⚠️ ",
                IssueLevel::Info => "ℹ️ ",
            };
            let svc = issue.service.as_deref().unwrap_or("global");
            println!("{icon} [{svc}] {}", issue.message);
        }
        println!();
        println!(
            "Validation: {} error(s), {} warning(s)",
            self.error_count(),
            self.warning_count()
        );
    }
}

// ── Validator ─────────────────────────────────────────────────────────────────

/// Run all checks against `compose` and return a validation report.
pub fn validate(compose: &ComposeFile) -> ValidationReport {
    let mut issues = Vec::new();

    check_has_services(compose, &mut issues);
    check_images(compose, &mut issues);
    check_healthchecks(compose, &mut issues);
    check_networks(compose, &mut issues);
    check_port_conflicts(compose, &mut issues);
    check_volume_references(compose, &mut issues);
    check_depends_on_refs(compose, &mut issues);

    ValidationReport { issues }
}

// ── Individual checks ─────────────────────────────────────────────────────────

fn check_has_services(compose: &ComposeFile, issues: &mut Vec<ValidationIssue>) {
    if compose.services.is_empty() {
        issues.push(ValidationIssue::error(
            None,
            "No services defined in compose file",
        ));
    }
}

fn check_images(compose: &ComposeFile, issues: &mut Vec<ValidationIssue>) {
    for (name, svc) in &compose.services {
        if svc.image.is_none() {
            issues.push(ValidationIssue::error(
                Some(name),
                "No image specified (build: is not supported by the container app manager)",
            ));
        }
    }
}

fn check_healthchecks(compose: &ComposeFile, issues: &mut Vec<ValidationIssue>) {
    for (name, svc) in &compose.services {
        match &svc.healthcheck {
            None => {
                issues.push(ValidationIssue::warning(
                    Some(name),
                    "No healthcheck defined — container app manager cannot monitor service health",
                ));
            }
            Some(hc) if hc.test.is_empty() => {
                issues.push(ValidationIssue::warning(
                    Some(name),
                    "Healthcheck has no test command",
                ));
            }
            _ => {}
        }
    }
}

fn check_networks(compose: &ComposeFile, issues: &mut Vec<ValidationIssue>) {
    // Networks declared but never used
    for net_name in compose.networks.keys() {
        let used = compose
            .services
            .values()
            .any(|svc| svc.networks.contains(net_name));
        if !used {
            issues.push(ValidationIssue::warning(
                None,
                format!("Network '{net_name}' is declared but not used by any service"),
            ));
        }
    }
    // Services referencing undeclared networks
    for (svc_name, svc) in &compose.services {
        for net in &svc.networks {
            if !compose.networks.contains_key(net.as_str()) {
                issues.push(ValidationIssue::info(
                    Some(svc_name),
                    format!("Service uses network '{net}' which is not declared at top level (will be auto-created)"),
                ));
            }
        }
    }
}

fn check_port_conflicts(compose: &ComposeFile, issues: &mut Vec<ValidationIssue>) {
    // Collect all host ports and detect duplicates
    let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for (svc_name, svc) in &compose.services {
        for port_str in &svc.ports {
            // port_str is "host:container[/proto]" or "container[/proto]"
            let host_part = if let Some((host, _)) = port_str.split_once(':') {
                host.to_string()
            } else {
                // No host port mapping — container port only
                continue;
            };
            if let Some(previous) = seen.get(&host_part) {
                issues.push(ValidationIssue::error(
                    Some(svc_name),
                    format!("Port {host_part} is already used by service '{previous}'"),
                ));
            } else {
                seen.insert(host_part, svc_name.clone());
            }
        }
    }
}

fn check_volume_references(compose: &ComposeFile, issues: &mut Vec<ValidationIssue>) {
    // Named volumes referenced in services but not declared at top level
    for (svc_name, svc) in &compose.services {
        for vol_str in &svc.volumes {
            let source = vol_str.split(':').next().unwrap_or("");
            // Skip bind mounts (start with / or . or ~)
            if source.starts_with('/') || source.starts_with('.') || source.starts_with('~') {
                continue;
            }
            if !source.is_empty() && !compose.volumes.contains_key(source) {
                issues.push(ValidationIssue::info(
                    Some(svc_name),
                    format!("Named volume '{source}' is not declared at top level (will be auto-created)"),
                ));
            }
        }
    }
}

fn check_depends_on_refs(compose: &ComposeFile, issues: &mut Vec<ValidationIssue>) {
    let service_names: HashSet<&str> = compose.services.keys().map(String::as_str).collect();
    for (svc_name, svc) in &compose.services {
        for dep in &svc.depends_on {
            if !service_names.contains(dep.as_str()) {
                issues.push(ValidationIssue::error(
                    Some(svc_name),
                    format!("depends_on references unknown service '{dep}'"),
                ));
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::parse_str;

    #[test]
    fn valid_compose_passes() {
        let yaml = r#"
services:
  app:
    image: myapp:latest
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost/health"]
"#;
        let f = parse_str(yaml).unwrap();
        let r = validate(&f);
        assert_eq!(r.error_count(), 0);
    }

    #[test]
    fn missing_image_is_error() {
        let yaml = r#"
services:
  app:
    environment:
      - KEY=value
"#;
        let f = parse_str(yaml).unwrap();
        let r = validate(&f);
        assert!(r
            .issues
            .iter()
            .any(|i| { i.level == IssueLevel::Error && i.service.as_deref() == Some("app") }));
    }

    #[test]
    fn no_healthcheck_is_warning() {
        let yaml = r#"
services:
  app:
    image: myapp:latest
"#;
        let f = parse_str(yaml).unwrap();
        let r = validate(&f);
        assert!(r.issues.iter().any(|i| i.level == IssueLevel::Warning));
    }

    #[test]
    fn port_conflict_detected() {
        let yaml = r#"
services:
  a:
    image: a:latest
    ports:
      - "8080:80"
  b:
    image: b:latest
    ports:
      - "8080:90"
"#;
        let f = parse_str(yaml).unwrap();
        let r = validate(&f);
        assert!(r
            .issues
            .iter()
            .any(|i| i.level == IssueLevel::Error && i.message.contains("8080")));
    }
}
