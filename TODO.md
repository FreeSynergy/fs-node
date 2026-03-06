# FreeSynergy.Node – Open Topics & TODO

This file tracks everything that is not yet decided or implemented.
It is a working document, not a specification.

---

## i18n – Installer Output Translation

The installer (`fsn-install.sh`) outputs all user-facing messages in English only.

**TODO:**
- Translate all `log()`, `warn()`, `err()`, `info()`, `ask()`, `step()` messages
- Target: all European languages + the 20 most spoken languages worldwide
- Detect system locale (`$LANG`, `$LC_ALL`) and select language automatically
- Fallback to English if locale is not supported
- Store translations as a separate `fsn-install-i18n.sh` (sourced by installer)
  or inline as a `declare -A` lookup table

**Languages (minimum):**
- European: English, German, French, Spanish, Portuguese, Italian, Dutch, Polish,
  Romanian, Hungarian, Czech, Slovak, Bulgarian, Croatian, Serbian, Greek,
  Swedish, Norwegian, Danish, Finnish, Albanian, Slovenian, Estonian, Latvian, Lithuanian
- Global top 20 by speakers: Mandarin, Hindi, Spanish, Arabic, Bengali, French,
  Russian, Portuguese, Urdu, Indonesian, German, Japanese, Swahili, Marathi,
  Telugu, Tamil, Turkish, Korean, Vietnamese, Italian

---

## Code Quality – Comments and File Headers

All files currently have inconsistent or missing headers and comments.

**TODO:**
- Add a standardized English header to every file:
  playbooks, tasks, module YAML files, shell scripts, templates, config files
- Header format for YAML/Ansible files:
  ```yaml
  ---
  # ==============================================================================
  # FreeSynergy.Node – <Short description>
  # File:    <relative path from repo root>
  # Purpose: <one sentence>
  # Called by: <who calls this>
  # Variables required: <key vars>
  # ==============================================================================
  ```
- Header format for shell scripts:
  ```bash
  # ==============================================================================
  # FreeSynergy.Node – <Short description>
  # File:    <relative path>
  # Purpose: <one sentence>
  # ==============================================================================
  ```
- All inline comments must be in English
- Remove redundant comments (comments that just repeat what the code says)
- Add comments where logic is non-obvious (Ansible set_fact scope, loop vars, etc.)

**Scope:** All files under `playbooks/`, `modules/`, `fsn-install.sh`, `playbooks/templates/`

---

## Code Cleanup – File and Structure Review

**TODO:**
- Audit all module YAML files for consistency:
  - Every module must have a `healthcheck` block
  - Every module must have `health_path`, `health_port`, `health_scheme`
  - Every module must have a `constraints` block
- Audit all task files for:
  - `loop_control` with `label` on every loop
  - No raw `loop:` without `loop_control`
  - `include_tasks` uses task-level `vars:` not `set_fact` for scope-sensitive data
- Verify `.gitignore` covers all generated files (vault.yml, host files, data dirs)
- Review module selection in installer: filter out infrastructure sub-modules
  (dragonfly, postgres) from the interactive list — they are loaded automatically

---

## Multi-Host Deploy

**Decision made:** Each server runs its own deployer. Central machine initiates
setup via SSH; after that, servers are autonomous.

**TODO:**
- Implement `{name}.{hostname}.yml` file handling in deploy-stack.yml
- Copy project files to remote host and execute playbooks there
- SSH connection verification step in the installer
- `sync-stack.yml` must report status across all hosts in a project

---

## Cloudflare DNS

**TODO:**
- Implement Cloudflare API calls in `dns-create-record.yml` (currently a fail-stub)
- Implement Cloudflare record deletion in `dns-remove-record.yml`
- Test with an actual Cloudflare API token

---

## Cache Slot Management

**TODO:**
- Scan all modules on a host for `cache_slot_*` environment variables
- Assign slots sequentially (0–15) per dragonfly instance
- When instance is full (16 slots used), create next instance (dragonfly-2)
- Resolve `cache_host`, `cache_port`, `cache_slot_N` variables during Quadlet generation

**Open decision:**
- Should dragonfly be shared at project level (like postgres), or per-module?
- Current model: each module that needs cache loads its own dragonfly sub-instance

---

## Federation

**Decisions made:**
- Mutual OIDC trust between autonomous Kanidm nodes
- Ed25519-signed provider list distributed across nodes
- Priority-based failover; invite tokens for partner onboarding
- Trust levels 0–4 via Kanidm groups
- `{name}.federation.yml` separate from project.yml

**TODO:**
- Implement Ed25519 signing/verification for provider list
- Implement auto-update mechanism (fetch + verify + replace)
- Implement `fsn federation invite` / `join` / `revoke` commands
- Implement Kanidm group auto-creation from federation config
- Playbooks: `federation-deploy.yml`, `federation-invite.yml`, `federation-revoke.yml`

---

## Nice to Have (Later)

- `fsn status` — show all running services + health
- Auto-update via systemd timer calling `update-stack.yml`
- Backup integration: Rustic/Restic hook after deploy
- `fsn logs {instance}` shortcut
- Web UI for non-technical users (much later)
- Forgejo Actions integration for CI/CD on the platform itself

---

## Decisions Already Made (for reference)

| Topic | Decision |
|-------|----------|
| Installer language | Bash (bootstrap) + Ansible (everything else) |
| Port access | `net.ipv4.ip_unprivileged_port_start=80` – no sudo needed |
| Network isolation | Only Zentinel has external access |
| SMTP/IMAP | Zentinel Layer-4 TCP forward to Stalwart |
| Stalwart published_ports | Removed – Zentinel handles all external ports |
| SSH key storage | Path only, never the key content |
| Proxy location | Host file, not project file |
| File naming | `{name}.project.yml` = local, `{name}.{host}.yml` = remote |
| Module constraints | Declared in module class YAML, enforced by deployer |
| Federation | Designed: signed provider list, OIDC trust, priority failover |
| Cache slots | Auto-assigned, 16 per instance, overflow to new dragonfly instance |
| load.services | Config access only, no container, module-level only |
| Proxy KDL management | Marker-based; deployer only touches its own markers |
| Host files in repo | Git-ignored; only `example.host.yml` is tracked |
| Host file required | Always required, even for localhost; created by installer |
| Service name uniqueness | Must be unique per project; duplicate = abort |
| Sync vs Deploy | `sync-stack.yml` (read-only report) + `deploy-stack.yml` (sync + apply) |
| vault.yml | Auto-generated on first install; never overwritten on re-run |
| DNS token | Collected by installer wizard; baked directly into vault.yml |
