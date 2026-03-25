// fs-dns – DNS record management.
// Replaces playbooks/tasks/dns-create-record.yml and dns-remove-record.yml.

pub mod hetzner;
pub mod provider;

pub use provider::{DnsProvider, DnsRecord, RecordType};

/// Create the right provider from a name string and API token.
pub fn make_provider(name: &str, token: &str) -> anyhow::Result<Box<dyn DnsProvider>> {
    match name {
        "hetzner" => Ok(Box::new(hetzner::HetznerDns::new(token))),
        "cloudflare" => anyhow::bail!("Cloudflare DNS not yet implemented"),
        "none" => Ok(Box::new(NoopDns)),
        other => anyhow::bail!("Unknown DNS provider: {}", other),
    }
}

/// No-op provider for when DNS automation is disabled.
struct NoopDns;

#[async_trait::async_trait]
impl DnsProvider for NoopDns {
    async fn create_record(&self, _: &DnsRecord) -> anyhow::Result<()> {
        Ok(())
    }
    async fn remove_record(&self, _: &DnsRecord) -> anyhow::Result<()> {
        Ok(())
    }
    async fn list_records(&self, _: &str) -> anyhow::Result<Vec<DnsRecord>> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_provider_none_returns_noop() {
        assert!(make_provider("none", "").is_ok());
    }

    #[test]
    fn make_provider_hetzner_succeeds() {
        assert!(make_provider("hetzner", "test-token").is_ok());
    }

    #[test]
    fn make_provider_cloudflare_returns_err() {
        assert!(make_provider("cloudflare", "token").is_err());
    }

    #[test]
    fn make_provider_unknown_returns_err() {
        assert!(make_provider("doesnotexist", "token").is_err());
    }

    #[tokio::test]
    async fn noop_dns_all_methods_succeed() {
        let provider = make_provider("none", "").unwrap();
        let record = DnsRecord {
            name: "test.example.com".to_string(),
            record_type: RecordType::A,
            value: "1.2.3.4".to_string(),
            ttl: 300,
        };
        provider.create_record(&record).await.unwrap();
        provider.remove_record(&record).await.unwrap();
        let records = provider.list_records("example.com").await.unwrap();
        assert!(records.is_empty());
    }
}
