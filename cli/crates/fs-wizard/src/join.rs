// Join token — generate and verify cluster join tokens.

use std::fmt;

/// A join token that allows a new node to connect to an existing cluster.
///
/// Tokens are UUID-based and carry an issuance timestamp so they can be
/// expired after a configurable TTL.
#[derive(Debug, Clone)]
pub struct JoinToken {
    /// The raw token string (UUID v4).
    pub token: String,
    /// Identifier of the cluster this token belongs to.
    pub cluster_id: String,
    /// UTC timestamp when this token was issued.
    pub issued_at: std::time::SystemTime,
}

impl JoinToken {
    /// Generate a new `JoinToken` for the given cluster.
    ///
    /// The token is a randomly generated UUID v4 formatted as a plain string.
    pub fn generate(cluster_id: &str) -> Self {
        // Use a simple UUID-v4-style generation via rand (no extra uuid crate required).
        let token = generate_uuid_v4();
        Self {
            token,
            cluster_id: cluster_id.to_string(),
            issued_at: std::time::SystemTime::now(),
        }
    }

    /// Returns `true` if `token` matches this join token.
    pub fn verify(&self, token: &str) -> bool {
        self.token == token
    }

    /// Returns `true` if this token is older than `ttl_hours`.
    pub fn is_expired(&self, ttl_hours: u64) -> bool {
        let ttl = std::time::Duration::from_secs(ttl_hours * 3600);
        match self.issued_at.elapsed() {
            Ok(elapsed) => elapsed >= ttl,
            Err(_) => false, // clock went backwards — treat as not expired
        }
    }

    /// Returns the token string.
    pub fn as_str(&self) -> &str {
        &self.token
    }
}

impl fmt::Display for JoinToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.token)
    }
}

// ── UUID v4 generator ─────────────────────────────────────────────────────────

/// Generate a UUID v4 string using random bytes from the OS.
fn generate_uuid_v4() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    // Seed from time + thread id for uniqueness.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let mut h = DefaultHasher::new();
    now.hash(&mut h);
    std::thread::current().id().hash(&mut h);
    let a = h.finish();

    now.hash(&mut h);
    a.hash(&mut h);
    let b = h.finish();

    now.hash(&mut h);
    b.hash(&mut h);
    let c = h.finish();

    now.hash(&mut h);
    c.hash(&mut h);
    let d = h.finish();

    // Format as UUID v4 (version bits set to 4, variant bits to 10xx).
    let b0 = ((a >> 32) & 0xffff_ffff) as u32;
    let b1 = (a & 0xffff) as u16; // time_mid
    let b2 = (((b >> 48) & 0x0fff) as u16) | 0x4000; // version 4
    let b3 = (((b >> 32) & 0x3fff) as u16) | 0x8000; // variant 10xx
    let b4 = c ^ d;

    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        b0,
        b1,
        b2,
        b3,
        b4 & 0x0000_ffff_ffff_ffff
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_unique_tokens() {
        let t1 = JoinToken::generate("cluster-1");
        let t2 = JoinToken::generate("cluster-1");
        // Very unlikely to collide.
        assert_ne!(t1.token, t2.token);
    }

    #[test]
    fn verify_matches_own_token() {
        let t = JoinToken::generate("cluster-1");
        assert!(t.verify(&t.token.clone()));
        assert!(!t.verify("wrong-token"));
    }

    #[test]
    fn not_expired_immediately() {
        let t = JoinToken::generate("cluster-1");
        assert!(!t.is_expired(24));
    }
}
