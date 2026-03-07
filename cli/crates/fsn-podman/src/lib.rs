// fsn-podman – Podman container and systemd unit management.
//
// Phase 1: stubs (Ansible handles execution)
// Phase 2: subprocess calls to `podman` and `systemctl`
// Phase 3: Podman REST API + D-Bus for systemd

pub mod podman;
pub mod systemd;

pub use podman::ContainerInfo;
pub use systemd::UnitStatus;
