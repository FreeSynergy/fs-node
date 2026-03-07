// fsn-engine – State reconciliation and code generation.
//
// The engine is the heart of FSN: it reads config, computes state,
// generates Quadlet/env files, and enforces constraints.
// It has no network I/O (DNS is in fsn-dns) and no process spawning
// (systemd/podman is in fsn-podman).

pub mod constraints;
pub mod diff;
pub mod generate;
pub mod observe;
pub mod resolve;
pub mod template;
