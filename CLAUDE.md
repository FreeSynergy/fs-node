# CLAUDE.md – FreeSynergy.Node

## What is this?

FreeSynergy.Node – a modular, decentralized deployment system based on
Podman Quadlets, managed via a Rust CLI.

## Rules

- Language in files: **English** (comments, YAML keys, variable names)
- Language in chat: **German**
- YAML style: max 160 chars per line, space after colon
- No CHANGELOG.md (removed for token savings)
- OOP everywhere: traits over match blocks, types carry their own behavior
- After every feature: commit directly

## Repository Structure

```
cli/                  → Rust workspace (CLI + deployment engine)
  crates/
    fsn-core/         → Node-specific data types + config parsing
    fsn-deploy/       → Deployment engine (Quadlet generation, Zentinel, reconciliation)
    fsn-dns/          → DNS provider integrations
    fsn-host/         → Host management (SSH, remote install, provisioning)
    fsn-cli/          → CLI binary (clap) — `fsn` command
modules/              → Module definitions (YAML + Templates + Hooks)
hosts/                → Host files (one per server)
projects/             → Project files + branding + sites
```

**UI is in FreeSynergy.Desktop** (separate repo, `fsd` binary). Node is CLI-only.

## Library Dependencies (FreeSynergy.Lib)

All shared libraries live in `../FreeSynergy.Lib/`. Never duplicate their logic in fsn-*.

| Library         | Purpose |
|---|---|
| `fsn-types`     | Resource/Capability traits, Meta, TypeRegistry |
| `fsn-error`     | FsnError, Repairable trait, ValidationIssue |
| `fsn-config`    | TOML loader/saver with backup + auto-repair |
| `fsn-i18n`      | Snippet-based i18n (t(), t_with()) |
| `fsn-theme`     | Theme system (theme.toml → CSS) |
| `fsn-help`      | Context-sensitive help topics |
| `fsn-health`    | Generic health check framework + HealthCheck trait |
| `fsn-container` | Container abstraction (Podman via bollard) |
| `fsn-template`  | Tera template engine wrapper |
| `fsn-plugin-sdk`     | WASM Plugin SDK |
| `fsn-plugin-runtime` | WASM Host runtime |

## Module Conventions

- Path: `modules/{type}/{name}/{name}.toml`
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

## OOP Rules (always follow)

- Behavior belongs to the type itself, NOT external match blocks
- Small objects > big match block
- New categories/types → new Trait/Impl, not new `match` arm

## Debugging

```bash
podman ps -a --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
ls ~/.config/containers/systemd/
podman logs -f kanidm
journalctl --user -u kanidm.service
systemctl --user status kanidm.service
systemctl --user daemon-reload && systemctl --user restart kanidm.service
```

## Branding

- "by KalEl" in header
- Cyan + White for FreeSynergy.Node colors
