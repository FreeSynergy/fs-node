// Converter — ComposeFile → Vec<ServiceConfig> (fsn-container types).
//
// Translates parsed compose services into the ServiceConfig format that
// QuadletManager uses to generate .container unit files.
// Uses fsn-container's existing quadlet generation — no socket, no bollard.

use fsn_container::{HealthCheck, PortBinding, ServiceConfig, Volume};

use crate::compose::{ComposeFile, ComposeService};

// ── Public API ────────────────────────────────────────────────────────────────

/// Convert all services in a compose file into `ServiceConfig` objects.
///
/// `instance_prefix` is prepended to every service name when it differs from
/// the bare service name (multi-service compose files get namespaced).
pub fn convert(compose: &ComposeFile, instance_prefix: &str) -> Vec<ServiceConfig> {
    compose
        .services
        .iter()
        .map(|(name, svc)| convert_service(name, svc, instance_prefix, compose))
        .collect()
}

// ── Internal ──────────────────────────────────────────────────────────────────

fn convert_service(
    name: &str,
    svc: &ComposeService,
    instance_prefix: &str,
    compose: &ComposeFile,
) -> ServiceConfig {
    let instance_name = instance_name(name, instance_prefix);
    let image = svc.image.clone().unwrap_or_else(|| format!("{name}:latest"));

    let mut config = ServiceConfig::new(&instance_name, image);
    config.description = Some(format!("Container App Manager-managed service: {instance_name}"));

    // Environment
    for env in &svc.environment {
        if let Some(value) = &env.value {
            config.environment.insert(env.name.clone(), value.clone());
        }
        // Variables without a value are omitted (come from host env, not supported in quadlets)
    }

    // Volumes
    config.volumes = svc.volumes.iter().map(|s| parse_volume(s)).collect();

    // Ports
    config.ports = svc.ports.iter().filter_map(|s| parse_port(s)).collect();

    // Network — use first declared network; fall back to compose file default or "fsn"
    config.network = pick_network(svc, compose);

    // Healthcheck
    if let Some(hc) = &svc.healthcheck {
        if !hc.test.is_empty() {
            // Strip "CMD" / "CMD-SHELL" prefix if present
            let test = strip_cmd_prefix(&hc.test);
            config.healthcheck = Some(HealthCheck {
                test,
                interval:     hc.interval.clone().unwrap_or_else(|| "30s".to_string()),
                timeout:      hc.timeout.clone().unwrap_or_else(|| "10s".to_string()),
                retries:      hc.retries.unwrap_or(3),
                start_period: hc.start_period.clone().unwrap_or_else(|| "5s".to_string()),
            });
        }
    }

    config
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build the instance name: `prefix` when prefix ≠ name, else just `name`.
fn instance_name(svc_name: &str, prefix: &str) -> String {
    if prefix.is_empty() || prefix == svc_name {
        svc_name.to_string()
    } else {
        format!("{prefix}-{svc_name}")
    }
}

/// Parse a volume string `"source:target[:opts]"` into a `Volume`.
fn parse_volume(s: &str) -> Volume {
    let parts: Vec<&str> = s.splitn(3, ':').collect();
    match parts.as_slice() {
        [source, target, opts] => Volume {
            host: source.to_string(),
            container: target.to_string(),
            options: Some(opts.to_string()),
        },
        [source, target] => Volume {
            host: source.to_string(),
            container: target.to_string(),
            options: None,
        },
        [target] => Volume {
            host: target.to_string(),
            container: target.to_string(),
            options: None,
        },
        _ => Volume {
            host: s.to_string(),
            container: s.to_string(),
            options: None,
        },
    }
}

/// Parse a port string `"host:container[/proto]"` into a `PortBinding`.
fn parse_port(s: &str) -> Option<PortBinding> {
    // Strip protocol suffix: "8080:80/tcp" → "8080:80", proto = "tcp"
    let (mapping, proto) = match s.rsplit_once('/') {
        Some((m, p)) => (m, p.to_string()),
        None         => (s, "tcp".to_string()),
    };

    let (host_str, container_str) = mapping.split_once(':')?;

    let host_port:      u16 = host_str.trim().parse().ok()?;
    let container_port: u16 = container_str.trim().parse().ok()?;

    Some(PortBinding { host_port, container_port, protocol: proto })
}

/// Choose the primary network for a service.
/// Prefers the first network in the service's list; falls back to "fsn".
fn pick_network(svc: &ComposeService, _compose: &ComposeFile) -> String {
    svc.networks.first().cloned().unwrap_or_else(|| "fsn".to_string())
}

/// Strip Docker `CMD` / `CMD-SHELL` prefix from healthcheck test arrays.
fn strip_cmd_prefix(test: &[String]) -> Vec<String> {
    match test.first().map(String::as_str) {
        Some("CMD") | Some("CMD-SHELL") => test[1..].to_vec(),
        _ => test.to_vec(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::parse_str;

    const COMPOSE: &str = r#"
services:
  app:
    image: myapp:1.0
    environment:
      - APP_PORT=8080
      - DATABASE_URL=postgres://db/mydb
    ports:
      - "8080:8080"
    volumes:
      - app-data:/data
    networks:
      - backend
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost/health"]
      interval: 30s
      timeout: 10s
      retries: 3
  db:
    image: postgres:16
    environment:
      POSTGRES_DB: mydb
      POSTGRES_PASSWORD: secret
    volumes:
      - pg-data:/var/lib/postgresql/data
volumes:
  app-data:
  pg-data:
networks:
  backend:
"#;

    #[test]
    fn converts_image() {
        let f = parse_str(COMPOSE).unwrap();
        let configs = convert(&f, "");
        let app = configs.iter().find(|c| c.name == "app").unwrap();
        assert_eq!(app.image, "myapp:1.0");
    }

    #[test]
    fn converts_env() {
        let f = parse_str(COMPOSE).unwrap();
        let configs = convert(&f, "");
        let app = configs.iter().find(|c| c.name == "app").unwrap();
        assert_eq!(app.environment.get("APP_PORT"), Some(&"8080".to_string()));
    }

    #[test]
    fn converts_port() {
        let f = parse_str(COMPOSE).unwrap();
        let configs = convert(&f, "");
        let app = configs.iter().find(|c| c.name == "app").unwrap();
        assert!(app.ports.iter().any(|p| p.host_port == 8080 && p.container_port == 8080));
    }

    #[test]
    fn instance_prefix_applied() {
        let f = parse_str(COMPOSE).unwrap();
        let configs = convert(&f, "prod");
        assert!(configs.iter().any(|c| c.name == "prod-app"));
        assert!(configs.iter().any(|c| c.name == "prod-db"));
    }

    #[test]
    fn single_service_no_prefix_duplication() {
        let f = parse_str(COMPOSE).unwrap();
        let configs = convert(&f, "app");
        // When prefix equals service name, no double prefix
        let app = configs.iter().find(|c| c.name == "app").unwrap();
        assert_eq!(app.name, "app");
    }

    #[test]
    fn healthcheck_cmd_prefix_stripped() {
        let f = parse_str(COMPOSE).unwrap();
        let configs = convert(&f, "");
        let app = configs.iter().find(|c| c.name == "app").unwrap();
        let hc = app.healthcheck.as_ref().unwrap();
        assert_eq!(hc.test.first().map(String::as_str), Some("curl"));
    }
}
