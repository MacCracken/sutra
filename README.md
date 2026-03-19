# Sutra

> **Sutra** (Sanskrit: सूत्र — thread, rule, formula) — AI-native infrastructure orchestration for AGNOS

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)

Declarative infrastructure-as-code for the AGNOS ecosystem. Define desired state in TOML playbooks, let Sutra enforce it across local nodes and fleets.

## Features

- **TOML playbooks** — canonical, versionable, diffable IaC format
- **YAML ↔ TOML conversion** — write in YAML, convert to TOML and back (`sutra convert --to toml`)
- **Markdown input** — write intent in Markdown, translate to TOML via hoosh
- **Natural language** — describe what you want, get a playbook (`sutra nl "install tarang on edge nodes"`)
- **Dry-run by default** — `sutra apply` shows a plan. `--confirm` executes.
- **Idempotent modules** — ark, argonaut, file, shell, user, verify
- **Tera templates** — `file.template` renders config files with playbook variables
- **Variables & facts** — `[vars]` in playbooks, `{{ var }}` expansion, `--facts` for OS/distro detection
- **Fleet orchestration** — target nodes by role, arch, tag, or ID
- **Parallel execution** — `-j N` for concurrent multi-node runs
- **Multiple transports** — local, SSH (russh), daimon HTTP (AGNOS fleet)
- **Task dependencies** — `depends_on` with topological ordering
- **Error recovery** — `on_error: fail | continue | ignore` per-task or per-playbook
- **JSON output** — `--output json` for scripting, CI/CD, and MCP integration
- **MCP tools** — 6 tools for AI-agent-driven orchestration
- **Audit trail** — every execution logged as JSON-lines for compliance

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
| `file` | template, copy, absent, permissions, line_in_file | File state (Tera templates) |
| `shell` | run, script | Arbitrary commands (`creates`/`removes` for idempotency) |
| `user` | present, absent, group_present, group_absent | User/group management |
| `verify` | port_listening, file_exists, service_running, http_ok | Post-task assertions |

Community modules (sutra-community repo): nftables, sysctl, aegis, daimon, edge.

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
- **MCP Tools** (6): `sutra_apply`, `sutra_plan`, `sutra_check`, `sutra_inventory`, `sutra_translate`, `sutra_convert`
- **Marketplace**: `recipes/marketplace/sutra.toml`

## Building

```bash
cargo build --release --workspace
cargo test --workspace
```

## Security

- **Treat playbooks like code** — review before executing with `--confirm`
- **Shell escaping** — all user-supplied parameters are escaped via `shlex` before shell interpolation
- **Dry-run by default** — `sutra apply` only shows a plan; `--confirm` required to execute
- **SSH host keys** — v1 accepts all server keys (MITM risk on untrusted networks); known_hosts validation planned for v2
- **Audit trail** — every confirmed run is logged to `~/.local/share/sutra/audit/`

## License

GPL-3.0 — see [LICENSE](LICENSE).
