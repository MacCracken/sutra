# Sutra Roadmap

## Current State (2026-03-18)

~1,700 lines of Rust across 5 crates. Solid scaffold: types, parsers, YAML/TOML
conversion, local transport, CLI with 9 subcommands, 36 passing tests. All module
logic is stubbed — plan/apply/check return hardcoded values. No real execution yet.

---

## MVP — "It actually does things on a local box"

Goal: `sutra apply playbook.toml --confirm` executes real changes on localhost.

| # | Work Item | Crate |
|---|-----------|-------|
| 1 | Wire transport into modules — modules receive `&dyn Transport` so plan/apply/check can exec commands and read files | `sutra-core`, `sutra-modules` |
| 2 | Implement ark module — real `ark install/remove/upgrade/pin/list` via shell commands through transport | `sutra-modules` |
| 3 | Implement argonaut module — real `argonaut enable/disable/start/stop/restart/status` | `sutra-modules` |
| 4 | Implement file module — copy, absent, permissions, line_in_file (defer template to v1) | `sutra-modules` |
| 5 | Implement verify module — port_listening, file_exists, service_running, http_ok as real checks | `sutra-modules` |
| 6 | Add shell module — escape hatch for arbitrary commands | `sutra-modules` |
| 7 | Add user module — user/group create/delete/modify | `sutra-modules` |
| 8 | Task execution engine — iterate tasks, call plan then apply, respect `--confirm` vs dry-run, bail on first failure | `src/main.rs`, `sutra-core` |
| 9 | Target filtering — match playbook targets against inventory (role, arch, tag, node_id, all) | `sutra-core` |
| 10 | Idempotency in apply — call check() before apply(), skip if already in desired state | `sutra-core` |
| 11 | Audit trail persistence — write RunRecord to disk (JSON lines) after each run | `sutra-core` |
| 12 | Integration tests — real playbook runs against a temp directory | workspace |

MVP ships 6 core modules: ark, argonaut, file, verify, shell, user. All work locally.

---

## v1 — "Production fleet orchestration"

Goal: `sutra apply playbook.toml -i fleet.toml --confirm` orchestrates across remote nodes.

| # | Work Item | Crate |
|---|-----------|-------|
| 1 | SSH transport — implement Transport trait over russh, key auth, host verification | `sutra-transport` |
| 2 | Daimon HTTP transport — implement Transport trait against daimon agent API (port 8090) | `sutra-transport` |
| 3 | Transport dispatch — `transport_for(node)` returns correct transport based on node.transport field | `sutra-transport` |
| 4 | Parallel node execution — run tasks across multiple nodes concurrently (tokio tasks, configurable concurrency) | `sutra-core` |
| 5 | File templating — Tera for file.template action with variable interpolation | `sutra-modules` |
| 6 | Error recovery — configurable on_error (fail/continue/rollback), retry with backoff | `sutra-core` |
| 7 | Task dependencies / ordering — task graph with explicit `depends_on` or implicit ordering | `sutra-core` |
| 8 | MCP tool handlers — implement the 6 MCP tools so AI agents can drive sutra | `sutra-mcp` |
| 9 | Daimon fleet integration — `sutra inventory --from-daimon` populates inventory from fleet API | `sutra-ai` |
| 10 | Hoosh NL integration — `sutra nl` sends to hoosh, receives TOML, user reviews | `sutra-ai` |
| 11 | Structured output — `--output json` for all commands (machine-readable for MCP/scripting) | CLI |
| 12 | Variables & facts — playbook-level vars, node facts gathered at start of run | `sutra-core` |
| 13 | Validate command hardening — check param types, required fields, module-specific schema validation | `sutra-core` |
| 14 | Comprehensive test suite — SSH transport tests (mock server), multi-node integration tests | workspace |

---

## v2 — "Linux-agnostic orchestration"

Goal: sutra works on any Linux distribution, not just AGNOS. Package managers, init
systems, and platform details are abstracted behind provider interfaces.

| # | Work Item | Notes |
|---|-----------|-------|
| 1 | Package provider abstraction — trait behind ark module: `ArkProvider`, `AptProvider`, `DnfProvider`, `PacmanProvider`, `ApkProvider` | Auto-detect or explicit `provider` field in task |
| 2 | Service provider abstraction — trait behind argonaut module: `ArgonautProvider`, `SystemdProvider`, `OpenRCProvider`, `RunitProvider` | Auto-detect from init system |
| 3 | OS fact gathering — detect distro, package manager, init system, arch at run start | Populate `node.facts` map |
| 4 | Provider auto-selection — match detected OS facts to correct provider impl | Fallback to explicit config |
| 5 | Cross-distro file paths — normalize config paths, service unit locations per distro | Provider-level knowledge |
| 6 | Generic package/service names — optional mapping table (e.g., `httpd` vs `apache2`) | User-configurable |
| 7 | Non-AGNOS testing matrix — CI tests against Debian, Fedora, Alpine, Arch containers | GitHub Actions matrix |
| 8 | Documentation — cross-distro usage guide, provider reference | |

The AGNOS-native ark/argonaut remain the default providers when running on AGNOS.
On other distros, sutra detects the environment and selects the appropriate provider.

---

## Post-v2 — "Ecosystem & community"

| # | Work Item | Notes |
|---|-----------|-------|
| 1 | sutra-community repo — separate repo for domain-specific modules | nftables, sysctl, aegis, daimon, edge, Docker/OCI, cloud providers, database |
| 2 | Community module loader — dynamic or compile-time registration of external module crates | Plugin trait + registry |
| 3 | Handlers / hooks — pre_task, post_task, on_failure callbacks in playbooks | |
| 4 | Roles / includes — reusable task groups, playbook composition | |
| 5 | Secrets management — encrypted vars, vault integration | |
| 6 | Diff output — show file diffs, config diffs in plan output | |
| 7 | Web UI / dashboard — run history, fleet state, playbook browser | Could be separate AGNOS app |
| 8 | Marketplace integration — publish/consume playbooks via recipes/marketplace | |
| 9 | Conditional tasks — `when:` clauses based on facts, vars, previous results | |
| 10 | Notification / webhook — post run results to Slack, HTTP endpoint, etc. | |

---

## Core vs Community Module Split

**Core** (ships with sutra — works on any Linux box):
- `ark` / package providers — package management
- `argonaut` / service providers — service state
- `file` — file templating/copy/permissions
- `verify` — post-task assertions
- `shell` — escape hatch for arbitrary commands
- `user` — user/group management

**Community** (sutra-community repo — domain/platform-specific):
- AGNOS-specific: `aegis`, `daimon`, `edge`
- System-specific: `nftables`, `sysctl`
- Platform-specific: Docker/OCI, cloud providers, database modules
