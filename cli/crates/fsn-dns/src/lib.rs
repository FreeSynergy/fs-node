// fsn-dns – DNS record management.
// Replaces playbooks/tasks/dns-create-record.yml and dns-remove-record.yml.

pub mod hetzner;
pub mod provider;

pub use provider::{DnsProvider, DnsRecord, RecordType};

/// Create the right provider from a name string and API token.
pub fn make_provider(name: &str, token: &str) -> anyhow::Result<Box<dyn DnsProvider>> {
    match name {
        "hetzner"    => Ok(Box::new(hetzner::HetznerDns::new(token))),
        "cloudflare" => anyhow::bail!("Cloudflare DNS not yet implemented"),
        "none"       => Ok(Box::new(NoopDns)),
        other        => anyhow::bail!("Unknown DNS provider: {}", other),
    }
}

/// No-op provider for when DNS automation is disabled.
struct NoopDns;

#[async_trait::async_trait]
impl DnsProvider for NoopDns {
    async fn create_record(&self, _: &DnsRecord) -> anyhow::Result<()> { Ok(()) }
    async fn remove_record(&self, _: &DnsRecord) -> anyhow::Result<()> { Ok(()) }
    async fn list_records(&self, _: &str) -> anyhow::Result<Vec<DnsRecord>> { Ok(vec![]) }
}
