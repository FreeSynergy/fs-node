// Variable analysis — infers type, role and confidence for env vars.
//
// Uses keyword matching on the variable name (uppercase).
// Returns probability estimates, never hard guarantees.

use serde::{Deserialize, Serialize};

use crate::compose::EnvVar;

// ── Types ─────────────────────────────────────────────────────────────────────

/// Base type of an environment variable value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VarType {
    Hostname,
    Url,
    Port,
    Secret,
    Email,
    ConnectionString,
    String,
}

/// Semantic role of a variable (which external service it relates to).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VarRole {
    Database { kind: DbKind },
    Cache { kind: CacheKind },
    Smtp { kind: SmtpKind },
    Iam { kind: IamKind },
    Storage { kind: StorageKind },
    Generic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DbKind {
    Postgres,
    Mysql,
    Mariadb,
    Mongodb,
    Generic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheKind {
    Redis,
    Dragonfly,
    Memcached,
    Valkey,
    Keydb,
    Generic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SmtpKind {
    Sender,
    Receiver,
    Generic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IamKind {
    OidcProvider,
    Ldap,
    Generic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StorageKind {
    S3,
    Generic,
}

// ── AnalyzedVar ───────────────────────────────────────────────────────────────

/// An environment variable enriched with type/role analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzedVar {
    pub name: String,
    pub value: Option<String>,
    pub var_type: VarType,
    pub role: VarRole,
    /// Confidence estimate 0–100.
    pub confidence: u8,
}

impl AnalyzedVar {
    /// Display string for reporting.
    pub fn summary(&self) -> String {
        let type_str = format!("{:?}", self.var_type).to_lowercase();
        let role_str = self.role_label();
        format!(
            "{:<40} → type: {:<18} role: {:<22} ({}%)",
            self.name, type_str, role_str, self.confidence
        )
    }

    fn role_label(&self) -> String {
        match &self.role {
            VarRole::Database { kind } => format!("database.{}", format!("{:?}", kind).to_lowercase()),
            VarRole::Cache    { kind } => format!("cache.{}",    format!("{:?}", kind).to_lowercase()),
            VarRole::Smtp     { kind } => format!("smtp.{}",     format!("{:?}", kind).to_lowercase()),
            VarRole::Iam      { kind } => format!("iam.{}",      format!("{:?}", kind).to_lowercase()),
            VarRole::Storage  { kind } => format!("storage.{}",  format!("{:?}", kind).to_lowercase()),
            VarRole::Generic           => "generic".to_string(),
        }
    }
}

// ── Analyzer ─────────────────────────────────────────────────────────────────

/// Analyzes all environment variables of a service.
pub fn analyze_vars(vars: &[EnvVar]) -> Vec<AnalyzedVar> {
    vars.iter().map(analyze_var).collect()
}

/// Infer type, role and confidence for a single env var.
pub fn analyze_var(var: &EnvVar) -> AnalyzedVar {
    let upper = var.name.to_uppercase();
    let (var_type, type_confidence) = infer_type(&upper);
    let (role, role_confidence)     = infer_role(&upper);

    // Blend confidences: both must be high for overall high confidence.
    let confidence = ((type_confidence as u16 + role_confidence as u16) / 2) as u8;

    AnalyzedVar {
        name:       var.name.clone(),
        value:      var.value.clone(),
        var_type,
        role,
        confidence,
    }
}

// ── Type inference ────────────────────────────────────────────────────────────

fn infer_type(upper: &str) -> (VarType, u8) {
    // Connection strings — check before URL to avoid false URL matches
    if has_any(upper, &["_DB", "_DATABASE", "_DSN", "_JDBC"]) {
        return (VarType::ConnectionString, 85);
    }
    // Secrets
    if has_any(upper, &["_PASSWORD", "_PASSWD", "_PASS", "_SECRET", "_KEY", "_TOKEN", "_API_KEY", "_APIKEY"]) {
        return (VarType::Secret, 90);
    }
    // URLs
    if has_any(upper, &["_URL", "_URI", "_ENDPOINT", "_BASEURL", "_BASE_URL"]) {
        return (VarType::Url, 90);
    }
    // Hostnames
    if has_any(upper, &["_HOST", "_HOSTNAME", "_ADDR", "_ADDRESS", "_SERVER"]) {
        return (VarType::Hostname, 90);
    }
    // Ports
    if upper.ends_with("_PORT") || upper == "PORT" {
        return (VarType::Port, 95);
    }
    // Email
    if has_any(upper, &["_EMAIL", "_MAIL_FROM", "_MAILFROM", "_FROM_EMAIL", "_FROM_ADDRESS"]) {
        return (VarType::Email, 85);
    }
    (VarType::String, 50)
}

// ── Role inference ────────────────────────────────────────────────────────────

fn infer_role(upper: &str) -> (VarRole, u8) {
    // Database roles
    if has_any(upper, &["POSTGRES", "PGSQL", "PG_"]) {
        return (VarRole::Database { kind: DbKind::Postgres }, 90);
    }
    if has_any(upper, &["MARIADB"]) {
        return (VarRole::Database { kind: DbKind::Mariadb }, 90);
    }
    if has_any(upper, &["MYSQL"]) {
        return (VarRole::Database { kind: DbKind::Mysql }, 85);
    }
    if has_any(upper, &["MONGO", "MONGODB"]) {
        return (VarRole::Database { kind: DbKind::Mongodb }, 85);
    }
    // Cache roles
    if has_any(upper, &["DRAGONFLY"]) {
        return (VarRole::Cache { kind: CacheKind::Dragonfly }, 90);
    }
    if has_any(upper, &["REDIS"]) {
        return (VarRole::Cache { kind: CacheKind::Redis }, 90);
    }
    if has_any(upper, &["MEMCACHE", "MEMCACHED"]) {
        return (VarRole::Cache { kind: CacheKind::Memcached }, 85);
    }
    if has_any(upper, &["VALKEY"]) {
        return (VarRole::Cache { kind: CacheKind::Valkey }, 90);
    }
    if has_any(upper, &["KEYDB"]) {
        return (VarRole::Cache { kind: CacheKind::Keydb }, 85);
    }
    // SMTP roles
    if has_any(upper, &["SMTP", "MAIL", "MAILER"]) {
        let kind = if has_any(upper, &["FROM", "SENDER", "OUT"]) {
            SmtpKind::Sender
        } else if has_any(upper, &["TO", "RECIPIENT", "IN"]) {
            SmtpKind::Receiver
        } else {
            SmtpKind::Generic
        };
        return (VarRole::Smtp { kind }, 80);
    }
    // IAM roles
    if has_any(upper, &["LDAP"]) {
        return (VarRole::Iam { kind: IamKind::Ldap }, 90);
    }
    if has_any(upper, &["OAUTH", "OIDC", "SSO", "AUTH_"]) {
        return (VarRole::Iam { kind: IamKind::OidcProvider }, 75);
    }
    // Storage roles
    if has_any(upper, &["S3_", "_S3", "MINIO", "STORAGE_", "_STORAGE", "BUCKET"]) {
        return (VarRole::Storage { kind: StorageKind::S3 }, 80);
    }
    (VarRole::Generic, 50)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn has_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn var(name: &str, value: &str) -> EnvVar {
        EnvVar::new(name, Some(value.to_string()))
    }

    #[test]
    fn secret_detection() {
        let v = analyze_var(&var("POSTGRES_PASSWORD", "secret"));
        assert_eq!(v.var_type, VarType::Secret);
        assert!(v.confidence >= 80);
    }

    #[test]
    fn postgres_role() {
        let v = analyze_var(&var("POSTGRES_HOST", "db"));
        assert_eq!(v.role, VarRole::Database { kind: DbKind::Postgres });
    }

    #[test]
    fn smtp_host() {
        let v = analyze_var(&var("SMTP_HOST", "mail.example.com"));
        assert_eq!(v.var_type, VarType::Hostname);
        assert!(matches!(v.role, VarRole::Smtp { .. }));
    }

    #[test]
    fn redis_url() {
        let v = analyze_var(&var("REDIS_URL", "redis://localhost:6379"));
        assert_eq!(v.var_type, VarType::Url);
        assert_eq!(v.role, VarRole::Cache { kind: CacheKind::Redis });
    }

    #[test]
    fn port_detection() {
        let v = analyze_var(&var("APP_PORT", "8080"));
        assert_eq!(v.var_type, VarType::Port);
    }

    #[test]
    fn generic_fallback() {
        let v = analyze_var(&var("APP_NAME", "myapp"));
        assert_eq!(v.var_type, VarType::String);
        assert_eq!(v.role, VarRole::Generic);
    }
}
