// Observe actual state – query systemd and podman for what is running.
//
// Phase 1: implemented in fsn-podman.
// This module provides the trait definition and a stub.

use anyhow::Result;
use fsn_core::state::ActualState;

/// Query the current state of all FSN-managed services on this host.
/// Delegates to fsn-podman in Phase 2.
pub fn observe() -> Result<ActualState> {
    // Phase 1: return empty (Ansible handles actual state, not us)
    // Phase 2: call fsn_podman::list_fsn_services()
    Ok(ActualState::default())
}
