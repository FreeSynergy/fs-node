// Token storage — persist join tokens to a TOML file.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

// ── StoredToken ───────────────────────────────────────────────────────────────

/// A single join token as stored in the token file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToken {
    /// The raw token string.
    pub token: String,
    /// Human-readable label, e.g. "node2 bootstrap 2026-03-15".
    pub label: String,
    /// RFC 3339 timestamp when this token was created.
    pub created_at: String,
    /// Whether this token has already been consumed by a joining node.
    pub used: bool,
}

// ── TokenFile ─────────────────────────────────────────────────────────────────

/// Root structure of the token storage file (`tokens.toml`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenFile {
    /// Cluster identifier that all tokens in this file belong to.
    pub cluster_id: String,
    /// All issued join tokens.
    #[serde(default)]
    pub join_tokens: Vec<StoredToken>,
}

impl TokenFile {
    /// Load a `TokenFile` from a TOML file at `path`.
    ///
    /// Returns an empty `TokenFile` with the default cluster ID when the file
    /// does not exist yet.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading token file: {}", path.display()))?;
        toml::from_str(&content).with_context(|| format!("parsing token file: {}", path.display()))
    }

    /// Serialize and write this `TokenFile` to `path` (creates or overwrites).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating directory: {}", parent.display()))?;
        }
        let content = toml::to_string_pretty(self).context("serializing token file")?;
        std::fs::write(path, content)
            .with_context(|| format!("writing token file: {}", path.display()))
    }

    /// Add a new token with the given raw token string and label.
    ///
    /// The `created_at` timestamp is set to the current UTC time in RFC 3339 format.
    pub fn add_token(&mut self, token: &str, label: &str) {
        let now = utc_now_rfc3339();
        self.join_tokens.push(StoredToken {
            token: token.to_string(),
            label: label.to_string(),
            created_at: now,
            used: false,
        });
    }

    /// Mark the first token matching `token` as used.
    ///
    /// No-op if no matching token is found.
    pub fn mark_used(&mut self, token: &str) {
        if let Some(t) = self.join_tokens.iter_mut().find(|t| t.token == token) {
            t.used = true;
        }
    }

    /// Returns all tokens that have not been used yet.
    pub fn active_tokens(&self) -> impl Iterator<Item = &StoredToken> {
        self.join_tokens.iter().filter(|t| !t.used)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns the current UTC time formatted as RFC 3339.
fn utc_now_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Manual RFC 3339 formatting (avoids chrono/time dependencies).
    let s = secs;
    let sec = s % 60;
    let min = (s / 60) % 60;
    let hour = (s / 3600) % 24;
    let days = s / 86400;

    // Approximate date from epoch days (good enough for a timestamp label).
    let (year, month, day) = days_to_ymd(days);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, min, sec
    )
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // Algorithm: http://howardhinnant.github.io/date_algorithms.html
    days += 719468;
    let era = days / 146097;
    let doe = days % 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
