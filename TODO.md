# FreeSynergy — Master TODO

Stand: 2026-03

---

## Deferred (low priority, kein Blocker)

- [ ] `examples/` Verzeichnis in jeder Lib-Crate
- [ ] Store-i18n: 51 Sprachen prüfen (Inhalte vollständig?)
- [ ] Fehlermeldungen in allen CLI-Commands
- [ ] `fsn-bridge-sdk`: noch kein Bedarf, komplett leer
- [ ] `migration/` Verzeichnis mit Skripten für v1 → v2 Config-Migration
- [ ] JSON-Schema für alle Modul-Manifeste (Validierung + UI-Generierung)
- [ ] `SchemaForm` — generiert Formulare automatisch aus JSON-Schema (nutzt `schemars`)

---

## F — Langzeit-Vision

Nicht für nächste Sprints — aber wichtig festzuhalten, damit Architektur-Entscheidungen heute die richtige Richtung haben.

- [ ] `VISION.md` schreiben: „Kein klassisches OS — dynamisch gerenderte, federierte Service-Views. Jede Interaktion ist ein Intent, geroutet zum besten Provider (lokal oder federated)"
- [ ] **Intent-Routing**: `fsn intent "show mails"` → routed zu bestem verfügbaren Mail-Modul (lokal/federated/remote) — Vorarbeit: ServiceRole-Registry (bereits vorhanden) um Intent-Mapping erweitern
- [ ] **Spatial/Card-based Desktop** als Alternative zum klassischen Window-Manager (Obsidian Canvas / Raycast-ähnlich) — erst nach stabilem Window-Manager evaluieren

---

## Reihenfolge (empfohlen)

1. **F** Vision dokumentieren, Intent-Routing evaluieren
