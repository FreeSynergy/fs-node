# FreeSynergy.Node

**Run your own infrastructure. Trust no one. Cooperate freely.**

FreeSynergy.Node is a modular, decentralized deployment system for self-hosted services.
It uses [Podman Quadlets](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html)
and a native Rust CLI (`fsn`) to deploy a full-featured platform on any Linux server —
without Docker, without root, and without a central controller.

---

## Why This Exists

Most self-hosted platforms assume you trust a single organization, a single cloud provider,
or a single piece of software to hold everything together.

FreeSynergy.Node is built around a different principle: **decentralization with voluntary cooperation**.

- Everyone runs their own instance, on their own hardware.
- Nobody gives their data to anyone else.
- Cooperation with other nodes is possible — but always opt-in, transparent, and revokable.
- You decide who you work with. You decide what you share.

This is not just a technical decision. It is the reason the whole system is designed the way it is.

---

## What It Does

FreeSynergy.Node reads your configuration (which services you want, on which hosts),
generates [Quadlet](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html)
unit files, and deploys them as rootless `systemd` services via Podman.

It handles:

- **Service deployment** — pull images, generate configs, start containers
- **Reverse proxy** — automatic route collection via [Zentinel](https://zentric.dev/) (Pingora/Rust)
- **TLS certificates** — automatic via Let's Encrypt (ACME), managed by Zentinel
- **DNS management** — create and clean up DNS records automatically (Hetzner DNS today, Cloudflare planned)
- **DNS reconciliation** — stale records from renamed services are removed on the next deploy
- **Network isolation** — only the proxy has external access; all other containers are internal
- **Secrets management** — encrypted vault (`vault.age`) — never committed, never logged
- **Setup wizard** — interactive `fsn init` collects all per-service requirements
- **Multi-host projects** — one project can span multiple servers
- **Federation** — (designed, implementation in progress) mutual OIDC trust between autonomous nodes

---

## Available Services

Services are organized by typed slot. Each slot can hold exactly one service implementation
(or none). The `database` and `cache` slots are internal and can be shared across services.

| Type | Service | Description |
|---|---|---|
| `proxy` | **zentinel** | Reverse proxy + TLS + DNS (Pingora/Rust) |
| `iam` | **kanidm** | Identity provider (OIDC, OAuth2, WebAuthn) |
| `mail` | **stalwart** | Mail server (SMTP, IMAP, JMAP) |
| `git` | **forgejo** | Git hosting + CI/CD |
| `wiki` | **outline** | Team wiki and knowledge base |
| `collab` | **cryptpad** | End-to-end encrypted collaborative documents |
| `chat` | **tuwunel** | Matrix homeserver |
| `tasks` | **vikunja** | Project management and task tracker |
| `tickets` | **pretix** | Event ticketing |
| `maps` | **umap** | Self-hosted OpenStreetMap instance |
| `monitoring` | **openobserver** | Metrics, logs, traces |
| `database` | **postgres** | PostgreSQL (internal — created per service) |
| `cache` | **dragonfly** | Redis-compatible cache (internal) |

---

## How It Works

### Three layers of configuration

```
modules/        Service class definitions (TOML templates) — git-tracked, reusable
hosts/          One file per server (infrastructure layer) — git-ignored
projects/       What runs where — partially git-tracked
```

**Service classes** (`modules/{type}/{name}/{name}.toml`) define what a service needs:
container image, ports, environment variables, health checks, and setup requirements.
They are generic and reusable across projects.

**Host files** define the physical server: IP, proxy settings, DNS provider.
Each server gets exactly one host file. Host files are git-ignored — they contain
infrastructure-specific details that differ per deployment.

**Project files** define which services run for your specific project:

```toml
# projects/myproject/myproject.project.toml
[project]
name    = "myproject"
domain  = "example.com"
version = "0.1.0"

[project.contact]
acme_email = "admin@example.com"

# Typed slots — each slot holds exactly one service
[services]
iam  = "kanidm"
mail = "stalwart"
git  = "forgejo"
wiki = "outline"

# All active services (includes sub-services like postgres)
[load.services]
zentinel.service_class = "proxy/zentinel"
kanidm.service_class   = "iam/kanidm"
stalwart.service_class = "mail/stalwart"
forgejo.service_class  = "git/forgejo"
outline.service_class  = "wiki/outline"
```

### Deployment flow

```
fsn-install.sh
  → installs Podman, builds fsn binary (Rust/cargo)

fsn server setup          (as root, once per server)
  → checks Podman ≥ 5.0
  → creates deploy user, enables linger
  → configures unprivileged ports

fsn init                  (interactive wizard)
  → project skeleton (project.toml + host.toml)
  → service selection (which services to activate)
  → per-service requirements (reads [[setup.fields]] from each module)
  → writes vault.age (encrypted secrets)

fsn deploy
  → resolves service dependencies
  → generates Quadlet unit files (.container + .env + .network)
  → deploys via systemd / Podman
  → generates Zentinel KDL config (routes + upstreams)
  → creates/reconciles DNS records
  → runs per-service post-deploy hooks
```

### CLI Commands

```bash
fsn init              # Interactive setup wizard
fsn deploy            # Deploy all services
fsn deploy --service forgejo  # Deploy single service
fsn status            # Show running services
fsn update            # Pull latest images + redeploy
fsn restart <name>    # Restart a service
fsn remove <name>     # Stop and remove a service
fsn logs <name>       # Stream container logs
fsn server setup      # One-time server bootstrap (run as root)
```

---

## Security Model

- All containers run **rootless** (no root, no `--privileged`)
- Only Zentinel binds to external ports (80/443 + TCP 25/143/993 for mail)
- All other containers communicate via internal Podman networks
- No published ports except the proxy
- Secrets stored in `projects/{name}/vault.age` — encrypted with `age`, git-ignored
- Host files are git-ignored — infrastructure details stay local
- Container images pinned to specific tags (no `latest` in production)

---

## Project Status

**v0.1.0-dev — Rust CLI replaces Ansible**

| Component | Status |
|---|---|
| Service definitions (14 services, TOML format) | Done |
| Project / host file schema | Done |
| Rust CLI (`fsn`) | Done |
| Setup wizard (`fsn init`) | Done |
| Quadlet generation | Done |
| Deploy / undeploy / restart / update | Done |
| Sub-service lifecycle (postgres, dragonfly) | Done |
| Post-deploy hooks (kanidm, forgejo, stalwart, cryptpad, tuwunel, vikunja) | Done |
| Server bootstrap (`fsn server setup`) | Done |
| Constraint enforcement (per\_host) | Done |
| DNS management (Hetzner) | Done |
| DNS reconciliation (rename cleanup) | Done |
| Secrets vault (age encryption) | Done |
| Zentinel KDL config generation | In Progress |
| TUI dashboard (ratatui) | Planned |
| Multi-host deploy | Planned |
| Federation / ID Broker | Designed |
| Cloudflare DNS | Planned |
| SCIM provisioning | Planned |

---

## Requirements

- Linux (Fedora, Debian, Ubuntu, Arch, CoreOS — detected automatically)
- Podman ≥ 5.0
- Rust toolchain (installed automatically by `fsn-install.sh` if missing)
- A domain name
- A DNS provider with API access (Hetzner DNS today)

---

## Quick Start

```bash
# Download and run the bootstrap installer
curl -fsSL https://raw.githubusercontent.com/FreeSynergyNet/FreeSynergy.Node/main/fsn-install.sh | bash
```

Or manually:

```bash
git clone https://github.com/FreeSynergyNet/FreeSynergy.Node.git
cd FreeSynergy.Node
bash fsn-install.sh
```

The installer will:
1. Detect your OS and install Podman + Rust
2. Build the `fsn` binary
3. Run `fsn init` — interactive wizard for project setup and secret generation
4. Deploy your services

---

## FreeSynergy.Net

[FreeSynergy.Net](https://freesynergy.net) is the reference deployment of FreeSynergy.Node —
a federated network of autonomous nodes running the full service stack.

It is built with this exact codebase and serves as the live proof of concept.

---

## License

MIT — see [LICENSE](LICENSE).

Note: We are working on a custom license that better reflects the project's values
(freedom, decentralization, voluntary cooperation). See [contributors.md](contributors.md)
for the current contribution policy.

---

## Contributing

Contributions are not yet accepted while the license and CLA are being finalized.
See [contributors.md](contributors.md) for details on what you can do now.

Bug reports and ideas are welcome via [GitHub Issues](https://github.com/FreeSynergyNet/FreeSynergy.Node/issues).
