// wizard.rs — Main entry point: Wizard struct.

use fsn_error::FsyError;

use crate::compose::{ComposeInput, ComposeService};
use crate::detect::{self, ServiceTypeHint};
use crate::generate::{self, ModuleToml};

// ── WizardResult ──────────────────────────────────────────────────────────────

/// Result of running the wizard on one compose service.
#[derive(Debug)]
pub struct WizardResult {
    pub service: ComposeService,
    pub hint: ServiceTypeHint,
    pub module: ModuleToml,
}

impl WizardResult {
    /// Render the generated module TOML as a string.
    pub fn to_toml(&self) -> String {
        self.module.to_toml()
    }
}

// ── Wizard ────────────────────────────────────────────────────────────────────

/// Docker Compose → FSN module TOML converter.
///
/// # Example
///
/// ```rust,ignore
/// let wizard = Wizard::new();
/// let results = wizard.convert_all(ComposeInput::text(yaml))?;
/// for r in results {
///     println!("{}", r.to_toml());
/// }
/// ```
pub struct Wizard;

impl Wizard {
    pub fn new() -> Self {
        Self
    }

    /// Convert all services in the compose input.
    pub fn convert_all(&self, input: ComposeInput) -> Result<Vec<WizardResult>, FsyError> {
        let yaml = input.resolve()?;
        let services = ComposeService::parse_all(&yaml)?;

        let results = services
            .into_iter()
            .map(|svc| {
                let hint = detect::detect(&svc);
                let module = generate::generate(&svc, &hint);
                WizardResult { service: svc, hint, module }
            })
            .collect();

        Ok(results)
    }

    /// Convert a single named service from the compose input.
    pub fn convert_service(
        &self,
        input: ComposeInput,
        service_name: &str,
    ) -> Result<WizardResult, FsyError> {
        let results = self.convert_all(input)?;
        results
            .into_iter()
            .find(|r| r.service.name == service_name)
            .ok_or_else(|| {
                FsyError::internal(format!(
                    "wizard: service '{service_name}' not found in compose file"
                ))
            })
    }
}

impl Default for Wizard {
    fn default() -> Self {
        Self::new()
    }
}
