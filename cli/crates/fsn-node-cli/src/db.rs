// Database lifecycle management for the FSN CLI.
//
// Initializes all Node-side SQLite databases at startup:
//   fsn.db       — audit log + core migrations (fsn-db Migrator)
//   fsn-core.db  — hosts, projects, invitations, federation
//   fsn-bus.db   — event log, subscriptions, routing rules, standing orders

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use anyhow::{Context, Result};
use fsn_node_core::audit::AuditEntry;
use fsn_db::{BufferedWrite, DbBackend, DbConnection, Migrator, WriteBuffer};
use tracing::warn;

static DB: OnceLock<Arc<DbConnection>> = OnceLock::new();
static WRITE_BUF: OnceLock<Arc<WriteBuffer>> = OnceLock::new();

/// Path to an FSN SQLite database under `~/.local/share/fsn/`.
pub fn db_path(filename: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".local/share/fsn").join(filename)
}

/// Initialize all Node databases: connect, run migrations, set up write buffer.
///
/// Call once at startup. Non-fatal — the CLI continues without persistence
/// if DB init fails (e.g. permission error, missing SQLite).
pub async fn init() -> Result<()> {
    let path = db_path("fsn.db");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating DB directory {}", parent.display()))?;
    }

    let conn = DbConnection::connect(DbBackend::Sqlite {
        path: path.to_string_lossy().into_owned(),
    })
    .await
    .map_err(|e| anyhow::anyhow!("DB connect: {e}"))?;

    Migrator::run(conn.inner())
        .await
        .map_err(|e| anyhow::anyhow!("DB migrations: {e}"))?;

    let buf = WriteBuffer::with_defaults(conn.inner().clone());
    WRITE_BUF.set(Arc::new(buf)).ok();
    DB.set(Arc::new(conn)).ok();

    // Initialize the two additional Node databases.
    if let Err(e) = init_core_db().await { warn!("fsn-core.db init failed: {e}"); }
    if let Err(e) = init_bus_db().await  { warn!("fsn-bus.db init failed: {e}");  }

    Ok(())
}

/// Spawn the write-buffer auto-flush loop as a background tokio task.
///
/// Call after `init()` succeeds. The task runs until the process exits.
pub fn spawn_flush_loop() {
    if let Some(buf) = WRITE_BUF.get() {
        let buf = buf.clone();
        tokio::spawn(async move { buf.run_auto_flush().await });
    }
}

/// Write an audit entry to the database via the write buffer.
///
/// Fire and forget — silently does nothing when the DB was not initialized.
pub async fn write_audit_entry(entry: &AuditEntry) {
    let Some(buf) = WRITE_BUF.get() else { return };

    // Escape single quotes for inline SQL (values are internal strings, not user input)
    let actor  = entry.actor.replace('\'', "''");
    let action = entry.action.replace('\'', "''");
    let kind   = entry.resource_kind.replace('\'', "''");
    let payload = match &entry.detail {
        Some(d) => format!("'{}'", d.replace('\'', "''")),
        None    => "NULL".to_string(),
    };

    let sql = format!(
        "INSERT INTO audit_logs (actor, action, resource_kind, payload, outcome, created_at) \
         VALUES ('{actor}', '{action}', '{kind}', {payload}, 'ok', {})",
        entry.timestamp,
    );

    if let Err(e) = buf.enqueue(BufferedWrite { sql, values: vec![] }).await {
        warn!("audit write failed: {e}");
    }
}

/// Return the active database connection, if initialized.
pub fn get_conn() -> Option<std::sync::Arc<DbConnection>> {
    DB.get().cloned()
}

/// Flush all pending writes to disk.
///
/// Call before process exit to ensure the last audit entries are persisted.
pub async fn flush() {
    if let Some(buf) = WRITE_BUF.get() {
        if let Err(e) = buf.flush().await {
            warn!("final DB flush failed: {e}");
        }
    }
}

// ── Additional Node databases ─────────────────────────────────────────────────

const CORE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS hosts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL UNIQUE,
    domain      TEXT    NOT NULL,
    ip_address  TEXT,
    ssh_port    INTEGER NOT NULL DEFAULT 22,
    status      TEXT    NOT NULL DEFAULT 'unknown',
    project_id  INTEGER,
    notes       TEXT,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS projects (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL UNIQUE,
    domain      TEXT,
    status      TEXT    NOT NULL DEFAULT 'draft',
    description TEXT,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS invitations (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    token           TEXT    NOT NULL UNIQUE,
    project_id      INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    role            TEXT    NOT NULL DEFAULT 'member',
    encrypted_toml  TEXT,
    port            INTEGER,
    expires_at      TEXT,
    used_at         TEXT,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS federation_peers (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL,
    domain      TEXT    NOT NULL UNIQUE,
    auth_broker TEXT,
    status      TEXT    NOT NULL DEFAULT 'pending',
    joined_at   TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS federation_rights (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    peer_id   INTEGER NOT NULL REFERENCES federation_peers(id) ON DELETE CASCADE,
    direction TEXT    NOT NULL,
    right     TEXT    NOT NULL,
    scope     TEXT    NOT NULL DEFAULT '*'
)
"#;

const BUS_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS event_log (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id     TEXT    NOT NULL UNIQUE,
    topic        TEXT    NOT NULL,
    source_role  TEXT    NOT NULL,
    source_inst  TEXT,
    payload_json TEXT    NOT NULL DEFAULT '{}',
    delivery     TEXT    NOT NULL DEFAULT 'fire-and-forget',
    storage      TEXT    NOT NULL DEFAULT 'no-store',
    acked_at     TEXT,
    created_at   TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_event_log_topic  ON event_log (topic);
CREATE INDEX IF NOT EXISTS idx_event_log_source ON event_log (source_role);
CREATE TABLE IF NOT EXISTS subscriptions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    subscriber_role TEXT    NOT NULL,
    topic_filter    TEXT    NOT NULL,
    inst_tag        TEXT,
    granted_read    INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_subs_role ON subscriptions (subscriber_role);
CREATE TABLE IF NOT EXISTS routing_rules (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT    NOT NULL,
    topic_pattern TEXT    NOT NULL,
    source_role   TEXT,
    delivery      TEXT    NOT NULL DEFAULT 'fire-and-forget',
    storage       TEXT    NOT NULL DEFAULT 'no-store',
    priority      INTEGER NOT NULL DEFAULT 0,
    enabled       INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE IF NOT EXISTS standing_orders (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT    NOT NULL,
    trigger_role  TEXT    NOT NULL,
    topic         TEXT    NOT NULL,
    payload_json  TEXT    NOT NULL DEFAULT '{}',
    enabled       INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now'))
)
"#;

async fn init_core_db() -> anyhow::Result<()> {
    open_and_apply_schema("fsn-core.db", CORE_SCHEMA).await
}

async fn init_bus_db() -> anyhow::Result<()> {
    open_and_apply_schema("fsn-bus.db", BUS_SCHEMA).await
}

async fn open_and_apply_schema(filename: &str, schema: &str) -> anyhow::Result<()> {
    let path = db_path(filename);
    let conn = DbConnection::connect(DbBackend::Sqlite {
        path: path.to_string_lossy().into_owned(),
    })
    .await
    .map_err(|e| anyhow::anyhow!("{filename} connect: {e}"))?;

    conn.apply_schema(schema)
        .await
        .map_err(|e| anyhow::anyhow!("{filename} schema: {e}"))
}
