// Zentinel KDL config generation.
//
// Generates the FSN-managed section of the Zentinel proxy config.
// Strategy: the entire upstreams{} + routes{} block is regenerated on every
// deploy and written between FSN-MANAGED markers — manually-edited parts
// (listeners, server settings) above/below the markers are never touched.
//
// Real Zentinel KDL format (docs.zentinelproxy.io/v/25.12/configuration/):
//
//   upstreams {
//     upstream "name" {
//       targets {
//         target {
//           address "host:port"
//         }
//       }
//     }
//   }
//   routes {
//     route "name" {
//       matches {
//         host "domain.example.com"
//       }
//       upstream "name"
//     }
//   }
//
// Zentinel is Pingora-based — NOT Caddy. The KDL syntax is its own.

use fsn_core::{
    config::service::ServiceType,
    state::desired::{DesiredState, ServiceInstance},
};

const MARKER_START: &str = "# === FSN-MANAGED-START ===";
const MARKER_END:   &str = "# === FSN-MANAGED-END ===";

// ── Public API ────────────────────────────────────────────────────────────────

/// Replace (or insert) the FSN-managed section in an existing Zentinel config.
/// Everything outside the markers is preserved verbatim.
pub fn upsert_managed_section(config: &str, desired: &DesiredState) -> String {
    let managed = generate_managed_section(desired);
    match (config.find(MARKER_START), config.find(MARKER_END)) {
        (Some(s), Some(e)) => {
            let end_of_block = e + MARKER_END.len();
            format!("{}{}{}", &config[..s], managed, &config[end_of_block..])
        }
        _ => format!("{}\n{}\n", config.trim_end(), managed),
    }
}

/// Generate the complete Zentinel config file from scratch (initial install).
/// Writes a minimal server + listeners block plus the FSN-managed section.
pub fn generate_full_config(desired: &DesiredState) -> String {
    let managed = generate_managed_section(desired);
    format!(
        "# Zentinel proxy configuration\n\
         # Lines outside the FSN-MANAGED block can be edited freely.\n\
         \n\
         listeners {{\n\
         \x20   listener \"http\" {{\n\
         \x20       address \"0.0.0.0:80\"\n\
         \x20   }}\n\
         \x20   listener \"https\" {{\n\
         \x20       address \"0.0.0.0:443\"\n\
         \x20   }}\n\
         }}\n\
         \n\
         {managed}\n"
    )
}

/// Remove a single service from the managed section.
/// Pass the filtered `DesiredState` (without the removed service) to regenerate.
pub fn upsert_without(config: &str, desired: &DesiredState) -> String {
    upsert_managed_section(config, desired)
}

// ── Core generation ───────────────────────────────────────────────────────────

/// Build the complete FSN-managed KDL block (upstreams + routes).
fn generate_managed_section(desired: &DesiredState) -> String {
    let instances = collect_proxy_instances(desired);

    let mut upstreams = String::new();
    let mut routes    = String::new();

    for inst in &instances {
        upstreams.push_str(&upstream_block(inst));
        routes.push_str(&route_blocks(inst));
    }

    format!(
        "{MARKER_START}\nupstreams {{\n{upstreams}}}\nroutes {{\n{routes}}}\n{MARKER_END}\n"
    )
}

/// Generate one `upstream "name" { targets { target { address "…:port" } } }` block.
fn upstream_block(inst: &ServiceInstance) -> String {
    let name = &inst.name;
    let port = inst.class.meta.port;
    // Containers reach each other by container name on the internal network.
    format!(
        "    upstream \"{name}\" {{\n\
         \x20       targets {{\n\
         \x20           target {{\n\
         \x20               address \"{name}:{port}\"\n\
         \x20           }}\n\
         \x20       }}\n\
         \x20   }}\n"
    )
}

/// Generate `route` blocks for all domains (primary + aliases) of a service.
fn route_blocks(inst: &ServiceInstance) -> String {
    let name = &inst.name;
    let mut all_domains = vec![inst.service_domain.clone()];
    all_domains.extend(inst.alias_domains.clone());

    let mut out = String::new();
    for domain in &all_domains {
        // Use domain as route name with dots replaced (KDL names must be unique).
        let route_name = domain.replace('.', "-");
        out.push_str(&format!(
            "    route \"{route_name}\" {{\n\
             \x20       matches {{\n\
             \x20           host \"{domain}\"\n\
             \x20       }}\n\
             \x20       upstream \"{name}\"\n\
             \x20   }}\n"
        ));
    }
    out
}

// ── Helper ────────────────────────────────────────────────────────────────────

/// Collect all instances (incl. sub-services) that need an HTTP proxy route.
/// Excludes internal services (Database, Cache) and the proxy itself.
fn collect_proxy_instances(desired: &DesiredState) -> Vec<ServiceInstance> {
    let mut out = Vec::new();
    for inst in &desired.services {
        push_proxy_instance(inst, &mut out);
    }
    out
}

fn push_proxy_instance(inst: &ServiceInstance, out: &mut Vec<ServiceInstance>) {
    // Include services that are user-facing and not the proxy itself.
    if !inst.class.meta.is_internal_only() && !inst.class.meta.has_type(&ServiceType::Proxy) {
        out.push(inst.clone());
    }
    for sub in &inst.sub_services {
        push_proxy_instance(sub, out);
    }
}
