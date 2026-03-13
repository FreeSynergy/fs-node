# FreeSynergy — Kompletter Refactoring-Plan v3 (Final)

**Stand:** März 2026  
**Autor:** KalEl + Claude  
**Version:** 3.0 — Finaler Plan mit allen Korrekturen und Ergänzungen

---

## 1. Grundprinzipien

### 1.1 Code-Wiederverwendung

Jede Funktionalität wird zuerst als eigenständige Library gebaut. `fsy-*` Libraries wissen nichts von FreeSynergy.Node — sie sind für Wiki.rs, Decidim.rs und jeden anderen nutzbar.

### 1.2 Standards

- **WASM-First** für Plugins (wasmtime + wit-bindgen Component Model)
- **CRDT von Tag 1** (Automerge)
- **ActivityPub von Tag 1** (activitypub_federation)
- **Nur Tera** für Templates
- **Englisch** im Code und in Kommentaren
- **Deutsch** in der Kommunikation hier und in Claude Code

### 1.3 UX-Konsistenz

Desktop, Web und TUI müssen sich **gleich anfühlen**. Gleiche Navigation, gleiche Shortcuts, gleiche Fenster-Metapher.

---

## 2. Repository-Struktur

```
FreeSynergy/Lib            ← Wiederverwendbare Bibliotheken (Cargo Workspace)
FreeSynergy/Node           ← Hauptanwendung (Cargo Workspace, nutzt fsy-*)
FreeSynergy/Node.Store     ← Plugin-Registry (Daten, kein Code)
FreeSynergy/Wiki.Store     ← Plugin-Registry für Wiki.rs (zukünftig)
FreeSynergy/Decidim.Store  ← Plugin-Registry für Decidim.rs (zukünftig)
```

### FreeSynergy/Lib — Bibliotheken

```
fsy-types/              Shared Types (Resource, Meta, TypeSystem, Capability)
fsy-error/              Fehlerbehandlung + Auto-Repair + Repairable-Trait
fsy-config/             TOML laden/speichern mit Validierung + Auto-Repair
fsy-i18n/               Fluent-basierte Schnipsel (actions, nouns, status, errors, ...)
fsy-sync/               CRDT-Sync (Automerge-Wrapper)
fsy-store/              Universeller Store-Client (Download, Registry, Suche)
fsy-plugin-sdk/         WASM Plugin SDK (Traits, wit-bindgen Interfaces)
fsy-plugin-runtime/     WASM Host (wasmtime)
fsy-federation/         OIDC + SCIM + ActivityPub + WebFinger
fsy-auth/               OAuth2 + JWT + Permissions
fsy-bridge-sdk/         Bridge-Interface-Traits
fsy-container/          Container-Abstraktion (Podman via bollard)
fsy-template/           Tera-Wrapper
fsy-health/             Health-Check Framework
fsy-crypto/             age-Encryption, mTLS, Key-Management
fsy-db/                 Datenbank-Abstraktion (SeaORM + rusqlite)
fsy-theme/              Theme-System (CSS-Variablen, TUI-Farben)
fsy-help/               Kontextsensitives Hilfe-System
```

### FreeSynergy/Node — Anwendung

```
crates/
  fsn-core/             Node-spezifische Logik
  fsn-deploy/           Quadlet-Generation, Zentinel
  fsn-host/             Host-Management, SSH, Remote-Install
  fsn-wizard/           Container-Assistent (YAML → Modul)
  fsn-cli/              CLI Binary (clap)
  fsn-app/              Dioxus App (Desktop/Web/Mobile)
```

---

## 3. Datenbank-Empfehlung: SeaORM + rusqlite

### Warum SeaORM?

Nach Analyse aller Optionen ist **SeaORM 2.0 mit rusqlite-Backend** die beste Wahl:

- **Async + Sync**: SeaORM 2.0 hat ein offizielles `sea-orm-sync` Crate mit rusqlite-Backend — perfekt für CLI-Tools wo async Overkill wäre, und das async-Backend für den Server/UI
- **Entity-First Workflow**: Entities definieren → Schema generiert. Passt zu unserem OOP-Ansatz
- **Migrationen eingebaut**: `sea-orm-cli` für Schema-Migrations
- **Multi-DB-fähig**: Startet mit SQLite, kann auf Postgres wechseln wenn nötig (Wiki.rs wird Postgres brauchen)
- **Admin Panel**: SeaORM Pro bietet gratis RBAC-Admin-Panel
- **Wiederverwendbar**: Dieselbe `fsy-db` Library kann in Node (SQLite), Wiki.rs (Postgres) und Decidim.rs (Postgres) eingesetzt werden

### Write-Buffering Engine (wie ownCloud)

Für das Problem mit vielen gleichzeitigen Schreibzugriffen (das Du von ownCloud kennst):

```rust
/// fsy-db: Write-Buffer für SQLite
pub struct WriteBuffer {
    queue: Vec<BufferedWrite>,
    flush_interval: Duration,    // z.B. 100ms
    max_batch_size: usize,       // z.B. 500 Operationen
    db: DatabaseConnection,
}

impl WriteBuffer {
    /// Schreibt nicht sofort, sondern puffert
    pub async fn enqueue(&mut self, write: BufferedWrite) -> Result<()>;
    
    /// Flush: Schreibt alle gepufferten Operationen in einer Transaktion
    pub async fn flush(&mut self) -> Result<FlushResult>;
    
    /// Automatischer Flush per Timer oder Batch-Größe
    pub async fn run_auto_flush(&mut self);
}
```

Das kombiniert SQLite-Vorteile (embedded, keine Infra) mit Batch-Writes (keine Lock-Contention bei vielen Zugriffen).

### Schema in fsy-db (wiederverwendbar)

```rust
// fsy-db bietet Basis-Entities die jedes Projekt erweitern kann
pub mod entities {
    pub mod resource;     // Basis-Resource mit Metadaten
    pub mod permission;   // RBAC-Permissions
    pub mod sync_state;   // CRDT-Sync-Zustand
    pub mod plugin;       // Installierte Plugins
    pub mod audit_log;    // Audit-Trail
}

// Node erweitert mit eigenen Entities
pub mod node_entities {
    pub mod host;
    pub mod project;
    pub mod module;
    pub mod container;
}
```

---

## 4. UI-Architektur

### 4.1 Desktop-Metapher

```
┌──────────────────────────────────────────────────────────┐
│  FreeSynergy.Node                              [_][□][X] │
├────────┬─────────────────────────────────────────────────┤
│        │                                                  │
│  NAV   │              CONTENT AREA                       │
│        │                                                  │
│ ┌────┐ │  ┌──────────────────────────────────────────┐   │
│ │ 🏠 │ │  │  Dashboard / Admin / Store / Help        │   │
│ │Home│ │  │                                          │   │
│ ├────┤ │  │  Programme werden hier als "Fenster"     │   │
│ │ ⚙  │ │  │  geöffnet, die man schließen kann       │   │
│ │Admin│ │  │                                          │   │
│ ├────┤ │  │  [OK] [Abbrechen] [Übernehmen]          │   │
│ │ 📦 │ │  └──────────────────────────────────────────┘   │
│ │Store│ │                                                  │
│ ├────┤ │  ┌──── Zweites Fenster ──────────────┐         │
│ │ ❓ │ │  │  Kann parallel offen sein          │         │
│ │Help│ │  │  [OK] [Abbrechen]                  │         │
│ └────┘ │  └────────────────────────────────────┘         │
│        │                                                  │
├────────┴─────────────────────────────────────────────────┤
│  Status: Online │ 3 Hosts │ 14 Module │ Sync OK │ DE/EN │
└──────────────────────────────────────────────────────────┘
```

### 4.2 Bereiche

| Bereich | Funktion |
|---|---|
| **Home/Desktop** | Übersicht aller installierten Programme/Module. Startet sie per Klick. |
| **Admin** | Hosts, Module, Projekte, Plugins verwalten. Wizard hier. |
| **Store** | Plugin-Browser, Download, Updates |
| **Help** | Kontextsensitive Hilfe (immer aufrufbar, F1 / ? / Menü) |

### 4.3 Container-Render-Modi (Metadaten pro Modul)

```toml
[module.ui]
# Welche UI-Modi unterstützt dieses Modul?
supports_web = true       # Hat Web-Interface
supports_tui = false      # Hat TUI-Interface (selten)
supports_desktop = true   # Kann als Desktop-App eingebettet werden
supports_api_only = true  # Nur API, kein UI

# Wie wird es geöffnet?
open_mode = "iframe"      # "iframe" | "external_browser" | "embedded" | "api"
web_url_template = "https://{{ domain }}/{{ service_path }}"
```

### 4.4 Fenster-System

**Alle Einblendungen sind Fenster** mit konsistentem Verhalten:

```rust
pub struct Window {
    pub id: WindowId,
    pub title: LocalizedString,
    pub content: Box<dyn WindowContent>,
    pub closable: bool,            // Immer true
    pub buttons: Vec<WindowButton>, // OK, Cancel, Apply
    pub size: WindowSize,
    pub scrollable: bool,          // Automatisch wenn Inhalt > Fenster
    pub help_topic: Option<String>, // Für kontextsensitive Hilfe
}

pub enum WindowButton {
    Ok,          // Bestätigen + Schließen
    Cancel,      // Abbrechen + Schließen
    Apply,       // Übernehmen (bleibt offen)
    Custom { label_key: String, action: WindowAction },
}
```

### 4.5 Scrolling (auch in TUI)

Jedes Formular und jede Liste ist **automatisch scrollbar** wenn der Inhalt nicht passt:

```rust
pub trait Scrollable {
    fn content_height(&self) -> u32;
    fn viewport_height(&self) -> u32;
    fn scroll_offset(&self) -> u32;
    fn needs_scroll(&self) -> bool {
        self.content_height() > self.viewport_height()
    }
}
```

In der TUI wird das über Dioxus' nativen Scroll-Support oder einen eigenen Scroll-Container gehandhabt. Maus-Scrolling + Tastatur (PgUp/PgDn/Home/End).

### 4.6 Hilfe-System (fsy-help)

```rust
pub struct HelpSystem {
    topics: HashMap<String, HelpTopic>,
    i18n: I18n,
}

pub struct HelpTopic {
    pub id: String,
    pub title_key: String,       // i18n-Key
    pub content_key: String,     // i18n-Key
    pub related: Vec<String>,    // Verwandte Themen
    pub context: HelpContext,    // Wo diese Hilfe angezeigt wird
}

impl HelpSystem {
    /// Kontextsensitive Hilfe: Was ist gerade aktiv?
    pub fn help_for_context(&self, ctx: &str) -> Option<&HelpTopic>;
    
    /// Suche in Hilfetexten
    pub fn search(&self, query: &str) -> Vec<&HelpTopic>;
    
    /// Anzeigen als Fenster
    pub fn show_help_window(&self, topic: &str) -> Window;
}
```

Aufruf: **F1** (Desktop/Web), **?** (TUI), Menü, oder Hilfe-Button in jedem Fenster.

---

## 5. Theme-System (fsy-theme)

### 5.1 Eine Datei regiert alles

Der Benutzer (oder eine KI die die Website baut) liefert **eine einzige Theme-Datei** ab. Diese wird für Dioxus (Desktop/Web) UND TUI interpretiert.

### 5.2 Theme-Format: `theme.toml`

```toml
[theme]
name = "FreeSynergy Default"
version = "1.0.0"
author = "KalEl"

[colors]
# Primärfarben (als Hex)
primary = "#2563eb"          # Hauptfarbe (Buttons, Links, Akzente)
primary_hover = "#1d4ed8"
primary_text = "#ffffff"

secondary = "#64748b"
secondary_hover = "#475569"
secondary_text = "#ffffff"

# Hintergrund
bg_base = "#ffffff"          # Haupt-Hintergrund
bg_surface = "#f8fafc"       # Karten, Panels
bg_overlay = "#f1f5f9"       # Overlays, Modals
bg_sidebar = "#1e293b"       # Sidebar

# Text
text_primary = "#0f172a"
text_secondary = "#475569"
text_muted = "#94a3b8"
text_inverse = "#ffffff"     # Text auf dunklem Hintergrund

# Status
success = "#22c55e"
warning = "#f59e0b"
error = "#ef4444"
info = "#3b82f6"

# Borders
border_default = "#e2e8f0"
border_focus = "#2563eb"

[typography]
font_family = "Inter, system-ui, sans-serif"
font_mono = "JetBrains Mono, monospace"
font_size_base = "16px"
font_size_sm = "14px"
font_size_lg = "20px"
font_size_xl = "24px"
font_size_2xl = "30px"
line_height = "1.5"

[spacing]
unit = "4px"                 # Basis-Einheit (alles Vielfache davon)
radius_sm = "4px"
radius_md = "8px"
radius_lg = "12px"

[tui]
# TUI-spezifische Überschreibungen
# Werden automatisch aus [colors] abgeleitet, können aber überschrieben werden
primary_fg = "blue"          # Crossterm-Farbname oder "rgb(37,99,235)"
primary_bg = "default"
sidebar_fg = "white"
sidebar_bg = "dark_gray"
border_style = "rounded"     # "plain" | "rounded" | "double" | "thick"
status_ok = "green"
status_error = "red"
status_warn = "yellow"
```

### 5.3 Wie das an die Website-KI übergeben wird

**Anweisungen für die KI die die Website baut:**

```
CSS-VARIABLEN KONVENTION:
Alle Farben und Abstände MÜSSEN als CSS Custom Properties definiert werden.

Datei: theme.css (wird von FreeSynergy.Node geladen)

Variablen-Namensschema:
  --fsy-color-primary: #2563eb;
  --fsy-color-primary-hover: #1d4ed8;
  --fsy-color-bg-base: #ffffff;
  --fsy-color-bg-surface: #f8fafc;
  --fsy-color-text-primary: #0f172a;
  --fsy-color-success: #22c55e;
  --fsy-color-warning: #f59e0b;
  --fsy-color-error: #ef4444;
  --fsy-font-family: 'Inter', system-ui, sans-serif;
  --fsy-font-mono: 'JetBrains Mono', monospace;
  --fsy-font-size-base: 16px;
  --fsy-spacing-unit: 4px;
  --fsy-radius-md: 8px;

Präfix ist IMMER: --fsy-

DATEI-LIEFERUNG:
Liefere EINE Datei: theme.css
Diese enthält NUR :root { ... } mit den CSS-Variablen.
Kein Layout, keine Komponenten — nur Variablen.
FreeSynergy.Node konvertiert diese automatisch in theme.toml.
```

### 5.4 Konvertierung

```rust
/// fsy-theme: Konvertiert zwischen Formaten
pub struct ThemeEngine {
    theme: Theme,
}

impl ThemeEngine {
    /// Lädt aus theme.toml
    pub fn from_toml(path: &Path) -> Result<Self>;
    
    /// Lädt aus theme.css (CSS Custom Properties extrahieren)
    pub fn from_css(path: &Path) -> Result<Self>;
    
    /// Generiert CSS für Dioxus Web
    pub fn to_css(&self) -> String;
    
    /// Generiert TUI-Farbschema
    pub fn to_tui_palette(&self) -> TuiPalette;
    
    /// Generiert Tailwind-Config
    pub fn to_tailwind_config(&self) -> String;
}
```

### 5.5 Mehrere Themes, wechselbar

```toml
# In den Settings
[appearance]
active_theme = "freesynergy-default"
available_themes = ["freesynergy-default", "freesynergy-dark", "helfa-green"]
```

Themes werden wie Plugins über den Store verteilbar und in den Settings wechselbar.

---

## 6. i18n — Schnipsel-System

### Kleine, wiederverwendbare Bausteine

```
locales/{lang}/
  actions.ftl       → save, delete, edit, search, confirm, cancel, ...
  nouns.ftl         → module, server, project, host, plugin, store, ...
  status.ftl        → online, offline, error, loading, syncing, ...
  errors.ftl        → file-not-found, invalid-config, connection-failed, ...
  phrases.ftl       → select-item, confirm-delete, welcome-to, ...
  time.ftl          → ago, minutes, hours, days, just-now, ...
  validation.ftl    → required-field, invalid-email, too-short, ...
  help.ftl          → help-dashboard, help-wizard, help-store, ...
```

Zusammengesetzt im Code:
```rust
// t("action-save") → "Save" / "Speichern"
// t_phrase("phrase-confirm-delete", [("item", t("noun-module"))]) 
//   → "Delete module?" / "Modul löschen?"
```

---

## 7. Error-Handling + Auto-Repair

Siehe Plan v2 — unverändert. Zusammenfassung:
- **Repairable-Trait** auf allen Konfig-Typen
- **AutoRepaired** → Toast-Notification
- **NeedsUserDecision** → Dialog mit Optionen
- **Unrecoverable** → Fehler anzeigen, nicht öffnen
- Backup immer anbieten bevor repariert wird

---

## 8. Container-Assistent (fsn-wizard)

Siehe Plan v2 — unverändert. Zusammenfassung:
- YAML/Docker-Compose eingeben (Text, URL, Datei)
- Automatische Typ-Erkennung (Image-Name, Ports, Volumes)
- Modul-Generation mit Standard-Werten
- Erklärungen was fehlt (APIs, Abhängigkeiten)
- Benutzer wählt Aufgaben-Typ (Purpose)

---

## 9. Typ-System + Schnittstellen

Siehe Plan v2 — unverändert. Zusammenfassung:
- **Capability-Trait**: Was kann ein Service? (APIs, Events, Formate)
- **Requirement-Trait**: Was braucht ein Service?
- **TypeRegistry**: Validiert Abhängigkeiten, findet kompatible Bridges
- Pro Typ: TOML-Definition im Store mit APIs, Events, Bridge-Kompatibilität

---

## 10. CRDT + Sync + Federation + Store + Bridges + Permissions

Alle Details aus Plan v2 bleiben bestehen. Hier nur die Entscheidungen:

| Thema | Entscheidung |
|---|---|
| CRDT | **Automerge** (3 wenn stabil, sonst 0.5 stable). Beitrag zum Projekt möglich. |
| Plugin-Interface | **wit-bindgen** (WASM Component Model Standard) |
| ActivityPub | **activitypub_federation** (Lemmy, Axum-kompatibel) |
| Datenbank | **SeaORM 2.0** + rusqlite (sync) + sqlx (async/Postgres) |
| Templates | **Tera** (einziger Template-Engine) |

---

## 11. Meine Verbesserungsvorschläge

Hier sind Dinge, die ich als wichtig erachte und die noch nicht angesprochen wurden:

### 11.1 Versionierung & Changelog

Jede `fsy-*` Library bekommt **eigene SemVer-Versionierung**. CHANGELOG.md pro Crate, nicht nur global. Nutze `cargo-release` für koordinierte Releases.

### 11.2 Feature Flags überall

Jede Library sollte granulare Feature-Flags haben:

```toml
[features]
default = ["sqlite"]
sqlite = ["sea-orm/rusqlite"]
postgres = ["sea-orm/sqlx-postgres"]
sync = ["automerge"]
federation = ["activitypub_federation", "openidconnect"]
wasm-plugins = ["wasmtime"]
```

Das hält die Compile-Zeiten kurz und erlaubt es, nur das einzubinden was gebraucht wird. Wiki.rs braucht vielleicht `federation` + `postgres` aber kein `wasm-plugins`.

### 11.3 CI/CD von Anfang an

- **GitHub Actions**: Build, Test, Clippy, Rustfmt auf jedem Push
- **cargo-deny**: License-Check, Advisory-DB-Check
- **Dependabot**: Automatische Dependency-Updates
- **Nightly Fuzzing**: cargo-fuzz auf fsy-config, fsy-sync, fsy-template (alles was User-Input parst)

### 11.4 Dokumentation

- **Jede fsy-* Crate**: README.md + `#[doc]` auf allen pub Items
- **docs.rs** automatisch (bei Publish auf crates.io)
- **Architektur-Docs**: `docs/ARCHITECTURE.md` pro Repo
- **Beispiele**: `examples/` Verzeichnis in jeder Library

### 11.5 Error-Recovery für Netzwerk

Nicht nur Dateien reparieren — auch Netzwerk-Fehler graceful behandeln:

```rust
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff: BackoffStrategy,
    pub on_failure: FailureAction,  // Cache nutzen, Offline-Modus, Benutzer fragen
}
```

Wenn der Store nicht erreichbar ist → lokalen Cache nutzen. Wenn ein Host offline ist → markieren, nicht crashen. Immer weiterlaufen.

### 11.6 Offline-First

Da es passive Clients gibt (lokal/mobil): **Alles muss offline funktionieren**. Store-Katalog wird gecacht, Konfigurationen sind lokal, CRDT-Sync passiert wenn Verbindung da ist. Kein Feature darf eine Netzwerkverbindung voraussetzen außer explizit netzwerk-basierten Aktionen.

### 11.7 Audit-Log

Jede Änderung an Konfigurationen, Hosts, Permissions, Plugins wird geloggt:

```rust
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub actor: Subject,
    pub action: AuditAction,
    pub target: ResourceRef,
    pub details: Value,
    pub source_host: HostRef,
}
```

Wird per CRDT synchronisiert → verteiltes, konsistentes Audit-Log.

### 11.8 Graceful Degradation

Wenn ein Plugin nicht lädt → Rest funktioniert trotzdem. Wenn CRDT-Sync fehlschlägt → lokaler Zustand bleibt nutzbar. Wenn ein Host offline ist → andere Hosts arbeiten weiter. Nichts darf das ganze System lahmlegen.

### 11.9 Config-Schema als JSON-Schema

Plugin-Metadaten und Modul-Konfigurationen sollten ein **JSON-Schema** mitbringen (auch wenn das Format TOML ist). Das ermöglicht:
- Automatische Validierung
- UI-Generierung (Forms aus Schema generieren)
- Dokumentation (Schema → Docs)
- Andere Tools können die Schemas nutzen

### 11.10 Migration von v1

Die bestehenden 135 Commits in Node sind nicht verloren:
- Modul-Definitionen → migrieren in Node.Store Format
- Deployment-Logik → migrieren in fsn-deploy
- i18n-Strings → migrieren in fsy-i18n Schnipsel-Format
- Ein `migration/` Verzeichnis mit Skripten die alte Configs konvertieren

---

## 12. Umsetzungsplan

### Phase 0: Setup (3-5 Tage)

- [ ] `FreeSynergy/Lib` erstellen, CI einrichten
- [ ] `FreeSynergy/UI` archivieren
- [ ] `FreeSynergy/Node` Branch `v2` erstellen
- [ ] CLAUDE.md + CHANGELOG.md Workflow

### Phase 1: Fundament (2-3 Wochen)

fsy-types, fsy-error, fsy-config, fsy-i18n, fsy-theme, fsy-help, fsy-db

### Phase 2: CRDT + Sync (2 Wochen)

fsy-sync (Automerge)

### Phase 3: Store + Plugins (3 Wochen)

fsy-store, fsy-plugin-sdk, fsy-plugin-runtime

### Phase 4: Auth + Federation (3-4 Wochen)

fsy-auth, fsy-federation, fsy-crypto

### Phase 5: Container + Templates (2 Wochen)

fsy-container, fsy-template, fsy-health

### Phase 6: Node Application (4-6 Wochen)

fsn-core, fsn-deploy, fsn-host, fsn-wizard, fsn-cli, fsn-app (Dioxus)

### Phase 7: Bridges (ongoing)

fsy-bridge-sdk + erste WASM-Bridge-Plugins

---

## 13. Vollständiger Bibliotheken-Stack

### Kern

| Crate | Version | Zweck |
|---|---|---|
| `dioxus` | 0.7.x | UI: TUI + Desktop + Web + Mobile |
| `serde` + `toml` + `serde_json` | 1 / 0.8 / 1 | Serialisierung |
| `sea-orm` | 2.0 | ORM (async: sqlx, sync: rusqlite) |
| `sea-orm-sync` | 2.0 | Sync SQLite für CLI |
| `automerge` | 0.5+ / 3.x | CRDT |
| `tera` | 1 | Templates |
| `fluent` | 0.16 | i18n |
| `activitypub_federation` | 0.7 | ActivityPub |

### Netzwerk

| Crate | Zweck |
|---|---|
| `reqwest` (rustls) | HTTP-Client |
| `axum` (via Dioxus) | HTTP-Server |
| `tokio-tungstenite` | WebSocket |
| `russh` | SSH |
| `rustls` + `rcgen` | TLS + Zertifikate |
| `tonic` | gRPC |

### Auth

| Crate | Zweck |
|---|---|
| `openidconnect` | OIDC |
| `oauth2` | OAuth2 |
| `jsonwebtoken` | JWT |
| `age` | Secrets |

### Plugins

| Crate | Zweck |
|---|---|
| `wasmtime` + `wasmtime-wasi` | WASM Runtime (Standard) |
| `wit-bindgen` | Component Model Interfaces |
| `libloading` + `abi_stable` | Native (nur Ausnahmen) |

### Container

| Crate | Zweck |
|---|---|
| `bollard` | Podman/Docker API |
| `serde_yaml` | YAML-Parse |
| `tokio-cron-scheduler` | Scheduling |
| `backon` | Retry mit Backoff |

### Qualität

| Crate | Zweck |
|---|---|
| `thiserror` + `anyhow` | Errors |
| `tracing` + `tracing-subscriber` | Logging |
| `opentelemetry` + `opentelemetry-otlp` | Observability |
| `rstest` + `insta` + `mockall` | Testing |
| `cargo-fuzz` | Fuzzing |
| `testcontainers` | Integration Tests |
| `schemars` | JSON-Schema Generation |
| `cargo-deny` | License/Advisory Check |

---

## 14. Zusammenfassung aller Entscheidungen

| Frage | Entscheidung |
|---|---|
| UI-Framework | **Dioxus 0.7.x** |
| Datenbank | **SeaORM 2.0** (rusqlite sync + sqlx async) |
| CRDT | **Automerge** (von Tag 1) |
| Plugins | **WASM-First** (wit-bindgen, wasmtime) |
| Templates | **Nur Tera** |
| Federation | **OIDC + SCIM + ActivityPub** (von Tag 1) |
| ActivityPub Crate | **activitypub_federation** |
| Theme-System | **Eine Datei** (theme.toml oder theme.css → konvertierbar) |
| Fenster | **Alle Einblendungen sind Fenster** (OK/Cancel/Apply) |
| Hilfe | **Immer aufrufbar** (F1, ?, Menü) |
| Scrolling | **Automatisch** wenn Inhalt > Viewport |
| Sprache im Code | **Englisch** |
| Sprache hier | **Deutsch** |
| Lib-Veröffentlichung | **crates.io** (wenn APIs stabil) |
| Repo-Struktur | **Lib-Monorepo + Node-Monorepo + Store-Repos** |
| Wiki.rs/Decidim.rs | **Demnächst** — fsy-* Libraries müssen stabil sein |

---

## Nächster Schritt

Phase 0: Repos aufsetzen. Dann Phase 1: fsy-types + fsy-error + fsy-config + fsy-i18n. Das Fundament.
