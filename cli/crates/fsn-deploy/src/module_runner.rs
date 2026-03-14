// Module plugin runner — delegates to fsn-plugin-runtime.
//
// `ModuleRunner` is now a thin alias for `fsn_plugin_runtime::PluginRunner`.
// `ContextBuilder` builds a `fsn_plugin_sdk::PluginContext` from FSN engine types.

pub use fsn_plugin_runtime::ProcessPluginRunner as ModuleRunner;

use fsn_plugin_sdk::{InstanceInfo, PeerRoute, PeerService, PluginContext};

// ── ContextBuilder ────────────────────────────────────────────────────────────

/// Builds a [`PluginContext`] from resolved FSN engine types.
pub struct ContextBuilder;

impl ContextBuilder {
    /// Construct a [`PluginContext`] from a resolved `ServiceInstance` and its peers.
    pub fn build(
        command: &str,
        instance: &fsn_core::state::desired::ServiceInstance,
        project_domain: &str,
        data_root: &str,
        peers: &[&fsn_core::state::desired::ServiceInstance],
    ) -> PluginContext {
        use fsn_core::resource::VarProvider as _;

        let peer_services: Vec<PeerService> = peers.iter().map(|p| {
            let routes: Vec<PeerRoute> = p.class.contract.routes.iter().map(|r| PeerRoute {
                id:   r.id.clone(),
                path: r.path.clone(),
                strip: r.strip,
            }).collect();

            PeerService {
                name:          p.name.clone(),
                class_key:     p.class_key.clone(),
                types:         p.service_types.iter().map(|t| t.to_string()).collect(),
                domain:        p.service_domain.clone(),
                port:          p.class.meta.port,
                upstream_tls:  p.class.contract.upstream_tls,
                routes,
                exported_vars: p.exported_vars(),
            }
        }).collect();

        // Merge all peer exported vars into a flat env map for the context.
        let env: std::collections::HashMap<String, String> = peers.iter()
            .flat_map(|p| p.exported_vars())
            .collect();

        PluginContext {
            protocol: 1,
            command: command.to_string(),
            instance: InstanceInfo {
                name:           instance.name.clone(),
                class_key:      instance.class_key.clone(),
                domain:         instance.service_domain.clone(),
                project:        project_domain.split('.').next().unwrap_or("").to_string(),
                project_domain: project_domain.to_string(),
                data_root:      data_root.to_string(),
                env:            instance.resolved_env.clone(),
            },
            peers: peer_services,
            env,
        }
    }
}
