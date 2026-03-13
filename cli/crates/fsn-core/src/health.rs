// Health validation — cross-resource consistency checks.
//
// Pattern: Validator (cross-resource) + Strategy (per-resource rule set).
//
// Types (HealthLevel, HealthStatus, HealthIssue, HealthRules) come from
// fsy-health. This module only contains the FSN-specific check functions.
//
// Required vs. optional (per spec):
//   Project required:  host, proxy (via host), mail service, wiki service
//   Project optional:  monitoring service (→ Warning), git service (→ Warning)
//   Host required:     proxy configured, project assigned
//   Service required:  project assigned, host assigned

use crate::config::host::HostConfig;
use crate::config::project::{ProjectConfig, ServiceInstanceConfig};

pub use fsy_health::{HealthCheck, HealthIssue, HealthLevel, HealthRules, HealthStatus};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract the broad type prefix from a service_class path.
/// E.g. `"mail/stalwart"` → `"mail"`, `"proxy/zentinel"` → `"proxy"`.
fn class_type(service_class: &str) -> &str {
    service_class.split('/').next().unwrap_or("")
}

/// Check whether a project's `load.services` contains at least one service
/// whose class path starts with the given type prefix.
fn project_has_type(project: &ProjectConfig, type_prefix: &str) -> bool {
    project.load.services.values()
        .any(|e| class_type(&e.service_class) == type_prefix)
}

// ── Project health ─────────────────────────────────────────────────────────────

/// Check the health of a project.
///
/// # Arguments
/// * `project`       — the project config to check.
/// * `host_projects` — list of project slugs referenced by known hosts.
pub fn check_project(project: &ProjectConfig, host_projects: &[&str]) -> HealthStatus {
    let has_host = host_projects.iter().any(|&p| p == project.project.meta.name.as_str());

    HealthRules::new()
        .require(has_host,                             "health.project.no_host")
        .require(project_has_type(project, "mail"),    "health.project.no_mail")
        .require(project_has_type(project, "wiki"),    "health.project.no_wiki")
        .warn(
            project_has_type(project, "observability")
                || project_has_type(project, "monitoring"),
            "health.project.no_monitoring",
        )
        .warn(project_has_type(project, "git"),        "health.project.no_git")
        .build()
}

// ── Host health ────────────────────────────────────────────────────────────────

/// Check the health of a host.
pub fn check_host(host: &HostConfig) -> HealthStatus {
    let has_project = host.host.project.as_deref()
        .map(|p| !p.is_empty())
        .unwrap_or(false);

    HealthRules::new()
        .require(!host.proxy.is_empty(), "health.host.no_proxy")
        .require(has_project,            "health.host.no_project")
        .build()
}

// ── Service health ─────────────────────────────────────────────────────────────

/// Check the health of a standalone service instance.
pub fn check_service(svc: &ServiceInstanceConfig) -> HealthStatus {
    let has_host = svc.service.host.as_deref()
        .map(|h| !h.is_empty())
        .unwrap_or(false);

    HealthRules::new()
        .require(!svc.service.project.is_empty(), "health.service.no_project")
        .require(has_host,                        "health.service.no_host")
        .build()
}
