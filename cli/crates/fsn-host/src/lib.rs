//! Host management — SSH connections, remote install, and server provisioning.
//!
//! # Planned features
//! - SSH session management (russh)
//! - Remote Podman/Quadlet deployment
//! - Server provisioning (install Podman, configure linger, unprivileged ports)
//! - Host health polling

/// A remote host that FSN manages.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RemoteHost {
    pub name: String,
    pub address: String,
    pub ssh_port: u16,
    pub ssh_user: String,
    pub ssh_key_path: Option<String>,
}

impl Default for RemoteHost {
    fn default() -> Self {
        Self {
            name: String::new(),
            address: String::new(),
            ssh_port: 22,
            ssh_user: "root".into(),
            ssh_key_path: None,
        }
    }
}

// TODO Phase 6: implement SSH session, remote command execution, file transfer
