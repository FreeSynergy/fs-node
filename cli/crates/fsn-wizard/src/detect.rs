// detect.rs — Service type detection from image name, ports, and volumes.
//
// Maps a ComposeService to one of the FSN service type hints so that
// generate.rs can pick sensible defaults.

use crate::compose::ComposeService;

// ── ServiceTypeHint ───────────────────────────────────────────────────────────

/// Best-guess FSN service class for a compose service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceTypeHint {
    /// Primary FSN class (e.g. "proxy/zentinel").
    pub class: String,
    /// Human-readable reasoning (shown in TUI / CLI output).
    pub reason: String,
    /// Confidence: 0–100.
    pub confidence: u8,
}

impl ServiceTypeHint {
    fn new(class: impl Into<String>, reason: impl Into<String>, confidence: u8) -> Self {
        Self {
            class: class.into(),
            reason: reason.into(),
            confidence,
        }
    }

    fn unknown() -> Self {
        Self::new("unknown", "no recognisable image or port pattern", 0)
    }
}

// ── Detection rules ───────────────────────────────────────────────────────────

/// Detect the service type from a compose service definition.
pub fn detect(svc: &ComposeService) -> ServiceTypeHint {
    let image = svc.image.to_lowercase();
    let image_name = image.split('/').last().unwrap_or(&image);
    let image_base = image_name.split(':').next().unwrap_or(image_name);

    let port_numbers: Vec<u16> = svc
        .ports
        .iter()
        .filter_map(|p| p.split(':').last()?.parse().ok())
        .collect();

    // ── image-name rules (highest priority) ───────────────────────────────────
    match image_base {
        "nginx" | "caddy" | "traefik" | "zentinel" | "haproxy" =>
            return ServiceTypeHint::new("proxy/zentinel", format!("image '{image_base}' is a reverse proxy"), 90),

        "stalwart" | "postfix" | "exim" | "mailu" | "mailserver" =>
            return ServiceTypeHint::new("mail/stalwart", format!("image '{image_base}' is a mail server"), 85),

        "forgejo" | "gitea" | "gogs" | "gitlab-ce" | "gitlab-ee" =>
            return ServiceTypeHint::new("git/forgejo", format!("image '{image_base}' is a git server"), 90),

        "outline" | "bookstack" | "wiki-js" | "wikijs" | "dokuwiki" =>
            return ServiceTypeHint::new("wiki/outline", format!("image '{image_base}' is a wiki"), 85),

        "kanidm" | "keycloak" | "authentik" | "authelia" =>
            return ServiceTypeHint::new("iam/kanidm", format!("image '{image_base}' is an IAM service"), 90),

        "tuwunel" | "synapse" | "element-web" | "matrix-org" | "conduit" =>
            return ServiceTypeHint::new("chat/tuwunel", format!("image '{image_base}' is a Matrix chat server"), 85),

        "cryptpad" | "etherpad" | "hedgedoc" | "excalidraw" =>
            return ServiceTypeHint::new("collab/cryptpad", format!("image '{image_base}' is a collaboration tool"), 80),

        "vikunja" | "planka" | "wekan" | "focalboard" =>
            return ServiceTypeHint::new("tasks/vikunja", format!("image '{image_base}' is a task manager"), 80),

        "pretix" | "eventyay" =>
            return ServiceTypeHint::new("tickets/pretix", format!("image '{image_base}' is an event/ticketing system"), 80),

        "umap" | "openstreetmap" =>
            return ServiceTypeHint::new("maps/umap", format!("image '{image_base}' is a maps service"), 80),

        "openobserve" | "grafana" | "prometheus" | "victoria-metrics" =>
            return ServiceTypeHint::new("monitoring/openobserve", format!("image '{image_base}' is a monitoring system"), 80),

        "postgres" | "postgresql" | "timescaledb" =>
            return ServiceTypeHint::new("database/postgres", format!("image '{image_base}' is a PostgreSQL database"), 90),

        "dragonfly" | "redis" | "keydb" | "valkey" =>
            return ServiceTypeHint::new("cache/dragonfly", format!("image '{image_base}' is a cache/key-value store"), 85),

        _ => {}
    }

    // ── port-based rules (lower priority) ─────────────────────────────────────
    for port in &port_numbers {
        match port {
            80 | 443 | 8080 | 8443 =>
                return ServiceTypeHint::new("proxy/zentinel", format!("port {port} suggests a web/proxy service"), 50),
            25 | 465 | 587 | 993 | 995 | 143 =>
                return ServiceTypeHint::new("mail/stalwart", format!("port {port} is a standard mail port"), 60),
            3000 | 3030 =>
                return ServiceTypeHint::new("git/forgejo", "port 3000 is common for Forgejo/Gitea", 40),
            5432 =>
                return ServiceTypeHint::new("database/postgres", "port 5432 is PostgreSQL", 70),
            6379 =>
                return ServiceTypeHint::new("cache/dragonfly", "port 6379 is Redis/Dragonfly", 70),
            8448 | 8008 =>
                return ServiceTypeHint::new("chat/tuwunel", "port 8448/8008 is Matrix federation/client port", 65),
            _ => {}
        }
    }

    ServiceTypeHint::unknown()
}
