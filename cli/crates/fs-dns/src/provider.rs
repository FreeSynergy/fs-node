// DNS provider trait and record types.

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    pub name: String, // e.g. "forgejo.example.com"
    pub record_type: RecordType,
    pub value: String, // IP address or CNAME target
    pub ttl: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecordType {
    A,
    Aaaa,
    Cname,
    Txt,
    /// Mail exchanger record.  `DnsRecord::value` = priority + space + hostname,
    /// e.g. "10 mail.example.com."
    Mx,
    /// Service locator.  `DnsRecord::value` = "priority weight port target",
    /// e.g. "10 1 587 mail.example.com."
    Srv,
}

impl std::fmt::Display for RecordType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordType::A => write!(f, "A"),
            RecordType::Aaaa => write!(f, "AAAA"),
            RecordType::Cname => write!(f, "CNAME"),
            RecordType::Txt => write!(f, "TXT"),
            RecordType::Mx => write!(f, "MX"),
            RecordType::Srv => write!(f, "SRV"),
        }
    }
}

/// Common interface for all DNS providers.
#[async_trait::async_trait]
pub trait DnsProvider: Send + Sync {
    async fn create_record(&self, record: &DnsRecord) -> Result<()>;
    async fn remove_record(&self, record: &DnsRecord) -> Result<()>;
    async fn list_records(&self, domain: &str) -> Result<Vec<DnsRecord>>;

    /// Reconcile: ensure desired records exist, remove stale ones.
    async fn reconcile(&self, desired: &[DnsRecord], domain: &str) -> Result<()> {
        let existing = self.list_records(domain).await?;

        for record in desired {
            let exists = existing
                .iter()
                .any(|r| r.name == record.name && r.record_type == record.record_type);
            if !exists {
                self.create_record(record).await?;
            }
        }

        // Remove records that are in existing but not in desired
        for record in &existing {
            let still_desired = desired
                .iter()
                .any(|r| r.name == record.name && r.record_type == record.record_type);
            if !still_desired {
                self.remove_record(record).await?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn record_type_display() {
        assert_eq!(RecordType::A.to_string(), "A");
        assert_eq!(RecordType::Aaaa.to_string(), "AAAA");
        assert_eq!(RecordType::Cname.to_string(), "CNAME");
        assert_eq!(RecordType::Txt.to_string(), "TXT");
        assert_eq!(RecordType::Mx.to_string(), "MX");
        assert_eq!(RecordType::Srv.to_string(), "SRV");
    }

    #[test]
    fn dns_record_fields() {
        let r = DnsRecord {
            name: "forgejo.example.com".to_string(),
            record_type: RecordType::A,
            value: "1.2.3.4".to_string(),
            ttl: 300,
        };
        assert_eq!(r.name, "forgejo.example.com");
        assert_eq!(r.record_type, RecordType::A);
        assert_eq!(r.value, "1.2.3.4");
        assert_eq!(r.ttl, 300);
    }

    // ── Mock DNS provider for reconcile tests ─────────────────────────────────

    struct MockDns {
        records: Mutex<Vec<DnsRecord>>,
        created: Mutex<Vec<DnsRecord>>,
        removed: Mutex<Vec<DnsRecord>>,
    }

    impl MockDns {
        fn new(initial: Vec<DnsRecord>) -> Self {
            Self {
                records: Mutex::new(initial),
                created: Mutex::new(vec![]),
                removed: Mutex::new(vec![]),
            }
        }
    }

    #[async_trait::async_trait]
    impl DnsProvider for MockDns {
        async fn create_record(&self, record: &DnsRecord) -> anyhow::Result<()> {
            self.created.lock().unwrap().push(record.clone());
            self.records.lock().unwrap().push(record.clone());
            Ok(())
        }
        async fn remove_record(&self, record: &DnsRecord) -> anyhow::Result<()> {
            self.removed.lock().unwrap().push(record.clone());
            self.records
                .lock()
                .unwrap()
                .retain(|r| !(r.name == record.name && r.record_type == record.record_type));
            Ok(())
        }
        async fn list_records(&self, _domain: &str) -> anyhow::Result<Vec<DnsRecord>> {
            Ok(self.records.lock().unwrap().clone())
        }
    }

    fn a_record(name: &str, value: &str) -> DnsRecord {
        DnsRecord {
            name: name.to_string(),
            record_type: RecordType::A,
            value: value.to_string(),
            ttl: 300,
        }
    }

    #[tokio::test]
    async fn reconcile_creates_missing_records() {
        let mock = MockDns::new(vec![]);
        mock.reconcile(&[a_record("git.example.com", "1.2.3.4")], "example.com")
            .await
            .unwrap();
        let created = mock.created.lock().unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].name, "git.example.com");
    }

    #[tokio::test]
    async fn reconcile_removes_stale_records() {
        let mock = MockDns::new(vec![a_record("old.example.com", "1.2.3.4")]);
        mock.reconcile(&[], "example.com").await.unwrap();
        let removed = mock.removed.lock().unwrap();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].name, "old.example.com");
    }

    #[tokio::test]
    async fn reconcile_skips_existing_records() {
        let mock = MockDns::new(vec![a_record("git.example.com", "1.2.3.4")]);
        mock.reconcile(&[a_record("git.example.com", "1.2.3.4")], "example.com")
            .await
            .unwrap();
        let created = mock.created.lock().unwrap();
        assert!(created.is_empty(), "should not re-create existing records");
    }
}
