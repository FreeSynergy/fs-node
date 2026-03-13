# CLAUDE.md – FreeSynergy.Node

## What is this?

FreeSynergy.Node – a modular, decentralized deployment system based on
Podman Quadlets, managed via Ansible, with a Rust CLI + TUI.

## Rules

- Language in files: **English** (comments, YAML keys, variable names)
- Language in chat: **German**
- YAML style: max 160 chars per line, space after colon
- No CHANGELOG.md (removed for token savings)
- OOP everywhere: traits over match blocks, types carry their own behavior
- After every feature: provide a git commit command

## Repository Structure

```
cli/                  → Rust workspace (CLI + TUI + core logic)
  crates/
    fsn-core/         → Node-specific data types + config parsing
    fsn-engine/       → Deployment engine (Zentinel, Quadlet generation)
    fsn-podman/       → Podman interaction
    fsn-dns/          → DNS provider integrations
    fsn-cli/          → CLI binary (clap)
    fsn-tui/          → Terminal UI (ratatui + rat-salsa)
    fsn-web/          → Web UI backend (axum)
    fsn-form/         → Form schema + derive macro
    fsn-form-derive/  → Proc macro for forms
modules/              → Module definitions (YAML + Templates + Hooks)
hosts/                → Host files (one per server)
projects/             → Project files + branding + sites
```

## Library Dependencies (FreeSynergy.Lib)

All shared libraries live in `../FreeSynergy.Lib/`. Never duplicate their logic in fsn-*.

| Library      | Purpose |
|---|---|
| `fsy-types`  | Resource/Capability traits, Meta, TypeRegistry |
| `fsy-error`  | FsyError, Repairable trait, ValidationIssue |
| `fsy-config` | TOML loader/saver with backup + auto-repair |
| `fsy-i18n`   | Snippet-based i18n (t(), t_with()) |
| `fsy-theme`  | Theme system (theme.toml → TUI palette + CSS) |
| `fsy-help`   | Context-sensitive help topics |
| `fsy-health` | Generic health check framework |
| `fsy-core`   | FormAction, SelectionResult |
| `fsy-tui`    | FormNode trait + all TUI node implementations |

## Module Conventions

- Path: `modules/{type}/{name}/{name}.yml`
- Block order: `module` → `vars` → `load` → `container` → `environment`
- `container.healthcheck` is required for every module
- `container.published_ports: []` for all except Zentinel
- `container.networks: []` is set automatically by the deployer

## Project Files

- `{name}.project.yml` = local deployment
- `{name}.{hostname}.yml` = remote deployment
- `vault_` prefix ONLY for real secrets

## Proxy (Zentinel)

- Lives in the host file, NOT the project file
- Static sites served directly by Zentinel
- Branding assets accessible under `/branding/`
- Landing page accessible under root domain

## Healthchecks

Every module has two health check levels:
1. **Quadlet** (`container.healthcheck`): Podman-level, restarts container on failure
2. **Zentinel** (`container.health_path`): Proxy-level, removes upstream from rotation

## TUI Architecture

Component-based with `FormNode` trait (`fsy-tui`). Nodes in `fsy-tui/src/nodes/`:
`TextInputNode`, `SelectInputNode`, `MultiSelectInputNode`, `TextAreaNode`,
`EnvTableNode`, `SectionNode`. Overlay stack (`overlay_stack: Vec<OverlayLayer>`).

Adding a new node: implement `handle_key()` + `render()`, no changes in events.rs needed.

## OOP Rules (always follow)

- Behavior belongs to the type itself, NOT external match blocks
- Small objects > big match block
- New categories/types → new Trait/Impl, not new `match` arm in events.rs

## Debugging

```bash
# All containers
podman ps -a --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

# Quadlet files
ls ~/.config/containers/systemd/

# Container logs
podman logs -f kanidm
journalctl --user -u kanidm.service

# Systemd status
systemctl --user status kanidm.service

# Reload quadlets
systemctl --user daemon-reload
systemctl --user restart kanidm.service
```

## Branding

- "by KalEl" in header
- Cyan + White for FreeSynergy.Node colors
