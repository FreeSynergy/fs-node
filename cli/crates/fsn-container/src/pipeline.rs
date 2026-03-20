// Container App Manager pipeline — parse → analyze → validate → convert → install.
//
// Ties together all container app manager modules into a single end-to-end workflow.

use std::path::Path;

use anyhow::Result;
use fsn_container::{QuadletManager, ServiceConfig};

use crate::{
    analysis::{analyze_vars, AnalyzedVar},
    compose::{parse_file, ComposeFile},
    converter::convert,
    instance::InstanceName,
    validation::{validate, ValidationReport},
};

// ── AnalyzeResult ─────────────────────────────────────────────────────────────

/// Output of the analysis step — used for reporting before any files are written.
pub struct AnalyzeResult {
    pub compose: ComposeFile,
    pub instance: InstanceName,
    pub validation: ValidationReport,
    /// Per-service variable analysis: service_name → analyzed vars
    pub vars_by_service: indexmap::IndexMap<String, Vec<AnalyzedVar>>,
}

impl AnalyzeResult {
    /// Print a human-readable analysis report to stdout.
    pub fn print_report(&self) {
        println!("── Instance: {} ─────────────────────────────────────────────", self.instance);
        println!();

        // Services
        println!("Services ({}):", self.compose.services.len());
        for (name, svc) in &self.compose.services {
            let image = svc.image.as_deref().unwrap_or("<none>");
            println!("  • {name}  [{image}]");
            let ports: Vec<_> = svc.ports.iter().map(String::as_str).collect();
            if !ports.is_empty() {
                println!("    ports: {}", ports.join(", "));
            }
        }
        println!();

        // Variable analysis per service
        for (svc, vars) in &self.vars_by_service {
            if vars.is_empty() { continue; }
            println!("Variables — {svc}:");
            for v in vars {
                println!("  {}", v.summary());
            }
            println!();
        }

        // Validation
        println!("Validation:");
        self.validation.print_report();
    }
}

// ── Pipeline ──────────────────────────────────────────────────────────────────

/// Analyze a compose file without writing any files.
pub fn analyze(path: &Path, instance_name: Option<&str>) -> Result<AnalyzeResult> {
    let compose = parse_file(path)?;

    let instance = match instance_name {
        Some(n) => InstanceName::from_str(n)?,
        None    => InstanceName::from_compose(&compose)?,
    };

    let validation = validate(&compose);

    let mut vars_by_service = indexmap::IndexMap::new();
    for (name, svc) in &compose.services {
        vars_by_service.insert(name.clone(), analyze_vars(&svc.environment));
    }

    Ok(AnalyzeResult { compose, instance, validation, vars_by_service })
}

/// Full install pipeline: parse → validate → convert → write quadlet files → daemon-reload.
///
/// `dry_run = true` skips writing files and starting services.
pub async fn install(
    path: &Path,
    instance_name: Option<&str>,
    dry_run: bool,
    store_url: Option<&str>,
) -> Result<Vec<ServiceConfig>> {
    let result = analyze(path, instance_name)?;

    result.print_report();

    if !result.validation.is_valid() {
        anyhow::bail!(
            "Validation failed with {} error(s). Fix them before installing.",
            result.validation.error_count()
        );
    }

    let prefix = result.instance.as_str();
    let services = convert(&result.compose, prefix);

    if dry_run {
        println!("\n── Dry-run: would write {} quadlet file(s) ──────────────────", services.len());
        for svc in &services {
            println!("  fsn-{}.container", svc.name);
        }
        return Ok(services);
    }

    // Store enrichment (best-effort, non-blocking)
    if let Some(url) = store_url {
        for svc in &result.compose.services {
            if let Some(image) = &svc.1.image {
                let analyzed = analyze_vars(&svc.1.environment);
                let _enriched = crate::store_client::enrich(url, image, analyzed).await;
                // Enrichment is informational only for now
            }
        }
    }

    // Write quadlet files
    let mgr = QuadletManager::user_default();
    for svc in &services {
        let path = mgr.create_quadlet(svc).await
            .map_err(|e| anyhow::anyhow!("failed to write quadlet for {}: {e}", svc.name))?;
        println!("  ✅ Written: {}", path.display());
    }

    // Reload systemd daemon
    mgr.reload_daemon().await
        .map_err(|e| anyhow::anyhow!("daemon-reload failed: {e}"))?;

    println!("\nInstalled {} service(s) for instance '{prefix}'.", services.len());
    println!("Start with: fsn container-app start {prefix}");

    Ok(services)
}
