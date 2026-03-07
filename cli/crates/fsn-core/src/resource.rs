// Unified Resource trait — the strategic foundation of FreeSynergy.Node.
//
// Every top-level managed object implements this trait:
//   Project, Host, Service, Federation, Bot
//
// This minimal v1 defines only what is needed right now.
// Future phases will add: reconcile(), desired_state(), render().

use anyhow::Result;

/// Lifecycle phase of a managed resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResourcePhase {
    #[default]
    Unknown,
    /// Config present but not yet deployed.
    Pending,
    /// All conditions satisfied, resource is operational.
    Ready,
    /// Running but one or more conditions are degraded.
    Degraded,
    /// Deployment failed or resource is in an error state.
    Failed,
}

impl std::fmt::Display for ResourcePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourcePhase::Unknown  => write!(f, "unknown"),
            ResourcePhase::Pending  => write!(f, "pending"),
            ResourcePhase::Ready    => write!(f, "ready"),
            ResourcePhase::Degraded => write!(f, "degraded"),
            ResourcePhase::Failed   => write!(f, "failed"),
        }
    }
}

/// Core interface for all FSN-managed resources.
///
/// Implementing this trait enables a type to participate in:
/// - TUI/WGUI generic editors
/// - Declarative deploy pipeline
/// - Federation discovery
pub trait Resource {
    /// Machine-readable resource kind: "project", "host", "service", etc.
    fn kind(&self) -> &'static str;

    /// Validate the resource's own invariants.
    /// Returns Err with a human-readable message on failure.
    fn validate(&self) -> Result<()>;

    /// Current lifecycle phase (default: Unknown until observed).
    fn phase(&self) -> ResourcePhase { ResourcePhase::Unknown }
}
