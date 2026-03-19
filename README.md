# Sutra

> **Sutra** (Sanskrit: सूत्र — thread, rule, formula) — AI-native infrastructure orchestration for AGNOS

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)

Declarative infrastructure-as-code for the AGNOS ecosystem. Define desired state in TOML playbooks, let Sutra enforce it across local nodes and fleets.

## Features

- **TOML playbooks** — canonical, versionable, diffable IaC format
- **YAML support** — write in YAML, convert to TOML (`sutra convert --to toml`)
- **Markdown input** — write intent in Markdown, translate to TOML via hoosh
- **Natural language** — describe what you want, get a playbook (`sutra nl "install tarang on edge nodes"`)
- **Dry-run by default** — `sutra apply` shows a plan. `--confirm` executes.
- **Idempotent modules** — ark, argonaut, file, verify, and more
- **Fleet orchestration** — target nodes by role, arch, tag, or ID
- **Multiple transports** — local, daimon HTTP (AGNOS fleet), SSH (anything else)
- **Audit trail** — every execution logged for compliance

## Quick Start

```bash
# Validate a playbook
sutra validate playbooks/deploy-tarang.toml

# Dry-run (show plan)
sutra apply playbooks/deploy-tarang.toml

# Execute
sutra apply playbooks/deploy-tarang.toml --confirm

# Convert YAML to TOML
sutra convert playbooks/deploy-tarang.yaml --to toml

# List available modules
sutra modules

# Natural language (requires hoosh on localhost:8088)
sutra nl "ensure all edge nodes are running tarang 2026.3.18"
```

## Playbook Formats

### TOML (canonical)

```toml
[playbook]
name = "Deploy tarang to edge fleet"

[[target]]
role = "edge"
arch = "aarch64"

[[task]]
module = "ark"
action = "install"
package = "tarang"
version = "2026.3.18"

[[task]]
module = "argonaut"
action = "enable"
service = "tarang"
```

### YAML (convert to TOML)

```yaml
playbook:
  name: Deploy tarang to edge fleet
target:
  - role: edge
    arch: aarch64
task:
  - module: ark
    action: install
    package: tarang
    version: "2026.3.18"
```

### Markdown (translate via hoosh)

```markdown
# Deploy tarang to edge fleet

## Target
- role: edge
- arch: aarch64

## Tasks
- Install `tarang` version `2026.3.18` via ark
- Enable `tarang.service` via argonaut
```

**Flow**: Markdown/YAML/NL are convenience inputs. The user always reviews the generated TOML before execution. AI assists, user approves.

## Modules

| Module | Actions | Description |
|--------|---------|-------------|
| `ark` | install, remove, upgrade, pin, list | Package state via AGNOS ark |
| `argonaut` | enable, disable, start, stop, restart, status | Service state via AGNOS argonaut |
| `file` | template, copy, absent, permissions, line_in_file | File state |
| `verify` | port_listening, file_exists, service_running, http_ok | Post-task assertions |

More modules planned: aegis, daimon, edge, shell, user, nftables, sysctl.

## Architecture

```
sutra-core       — Playbook parser, task graph, module trait
sutra-modules    — Built-in module implementations
sutra-transport  — Local, SSH, daimon HTTP transport
sutra-ai         — Markdown/NL translation, daimon/hoosh clients
sutra-mcp        — MCP server (6 tools)
```

## AGNOS Integration

- **Daimon** (port 8090): Agent registration, fleet inventory, audit
- **Hoosh** (port 8088): NL/Markdown to TOML translation
- **MCP Tools**: `sutra_apply`, `sutra_plan`, `sutra_check`, `sutra_inventory`, `sutra_translate`, `sutra_convert`
- **Marketplace**: `recipes/marketplace/sutra.toml`

## Building

```bash
cargo build --release --workspace
cargo test --workspace
```

## License

GPL-3.0 — see [LICENSE](LICENSE).
