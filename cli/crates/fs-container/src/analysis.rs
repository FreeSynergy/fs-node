// Variable analysis — infers type, role and confidence for env vars.
//
// Uses keyword matching on the variable name (uppercase).
// Returns probability estimates, never hard guarantees.

use std::fmt;
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

impl VarType {
    /// Infer the variable type from an uppercase variable name.
    pub fn infer_from(upper: &str) -> (VarType, u8) {
        // Connection strings — check before URL to avoid false URL matches
        if has_any(upper, &["_DB", "_DATABASE", "_DSN", "_JDBC"]) {
            return (VarType::ConnectionString, 85);
        }
        if has_any(upper, &["_PASSWORD", "_PASSWD", "_PASS", "_SECRET", "_KEY", "_TOKEN", "_API_KEY", "_APIKEY"]) {
            return (VarType::Secret, 90);
        }
        if has_any(upper, &["_URL", "_URI", "_ENDPOINT", "_BASEURL", "_BASE_URL"]) {
            return (VarType::Url, 90);
        }
        if has_any(upper, &["_HOST", "_HOSTNAME", "_ADDR", "_ADDRESS", "_SERVER"]) {
            return (VarType::Hostname, 90);
        }
        if upper.ends_with("_PORT") || upper == "PORT" {
            return (VarType::Port, 95);
        }
        if has_any(upper, &["_EMAIL", "_MAIL_FROM", "_MAILFROM", "_FROM_EMAIL", "_FROM_ADDRESS"]) {
            return (VarType::Email, 85);
        }
        (VarType::String, 50)
    }
}

impl fmt::Display for VarType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            VarType::Hostname         => "hostname",
            VarType::Url              => "url",
            VarType::Port             => "port",
            VarType::Secret           => "secret",
            VarType::Email            => "email",
            VarType::ConnectionString => "connection-string",
            VarType::String           => "string",
        })
    }
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

impl VarRole {
    /// Infer the semantic role from an uppercase variable name.
    pub fn infer_from(upper: &str) -> (VarRole, u8) {
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
        if has_any(upper, &["LDAP"]) {
            return (VarRole::Iam { kind: IamKind::Ldap }, 90);
        }
        if has_any(upper, &["OAUTH", "OIDC", "SSO", "AUTH_"]) {
            return (VarRole::Iam { kind: IamKind::OidcProvider }, 75);
        }
        if has_any(upper, &["S3_", "_S3", "MINIO", "STORAGE_", "_STORAGE", "BUCKET"]) {
            return (VarRole::Storage { kind: StorageKind::S3 }, 80);
        }
        (VarRole::Generic, 50)
    }
}

impl fmt::Display for VarRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VarRole::Database { kind } => write!(f, "database.{kind}"),
            VarRole::Cache    { kind } => write!(f, "cache.{kind}"),
            VarRole::Smtp     { kind } => write!(f, "smtp.{kind}"),
            VarRole::Iam      { kind } => write!(f, "iam.{kind}"),
            VarRole::Storage  { kind } => write!(f, "storage.{kind}"),
            VarRole::Generic           => f.write_str("generic"),
        }
    }
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

impl fmt::Display for DbKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DbKind::Postgres => "postgres",
            DbKind::Mysql    => "mysql",
            DbKind::Mariadb  => "mariadb",
            DbKind::Mongodb  => "mongodb",
            DbKind::Generic  => "generic",
        })
    }
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

impl fmt::Display for CacheKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CacheKind::Redis      => "redis",
            CacheKind::Dragonfly  => "dragonfly",
            CacheKind::Memcached  => "memcached",
            CacheKind::Valkey     => "valkey",
            CacheKind::Keydb      => "keydb",
            CacheKind::Generic    => "generic",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SmtpKind {
    Sender,
    Receiver,
    Generic,
}

impl fmt::Display for SmtpKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SmtpKind::Sender   => "sender",
            SmtpKind::Receiver => "receiver",
            SmtpKind::Generic  => "generic",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IamKind {
    OidcProvider,
    Ldap,
    Generic,
}

impl fmt::Display for IamKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            IamKind::OidcProvider => "oidc-provider",
            IamKind::Ldap         => "ldap",
            IamKind::Generic      => "generic",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StorageKind {
    S3,
    Generic,
}

impl fmt::Display for StorageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            StorageKind::S3      => "s3",
            StorageKind::Generic => "generic",
        })
    }
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
        format!(
            "{:<40} → type: {:<18} role: {:<22} ({}%)",
            self.name, self.var_type, self.role, self.confidence
        )
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
    let (var_type, type_confidence) = VarType::infer_from(&upper);
    let (role, role_confidence)     = VarRole::infer_from(&upper);

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
