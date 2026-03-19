# Sutra Roadmap

> **Status**: v1 complete | **Last Updated**: 2026-03-19

---

## Completed

### MVP — "It actually does things on a local box"

All items complete. `sutra apply playbook.toml --confirm` executes real changes on localhost with idempotency, audit trail, and 6 core modules.

- Executor with enum dispatch (avoids async-dyn issues)
- 6 core modules: ark, argonaut, file, verify, shell, user — all with real implementations
- Task execution engine with plan→check→apply loop
- Target filtering (role, arch, tag, node_id, all)
- Idempotency — check() before apply(), skip if already met
- Audit trail — JSON-lines to `~/.local/share/sutra/audit/`
- Integration tests — 10 tests covering file lifecycle, shell idempotency, targeting, audit

### v1 — "Production fleet orchestration"

All items complete except daimon HTTP transport (blocked on AGNOS API — tracked as T1/T2 in AGNOS roadmap).

- SSH transport — `ExecutorKind::Ssh` via russh 0.48, lazy connect, ed25519/rsa key auth
- Parallel node execution — `-j N` / `--parallel N` with bounded semaphore, `--continue-on-error`
- File templating — Tera engine for `file.template` action
- Error recovery — `on_error: fail/continue/ignore` per-task and per-playbook
- Task dependencies — `name` + `depends_on` fields, topological sort, cycle detection
- MCP tool handlers — all 6 tools implemented with real execution
- Daimon fleet integration — client wired into CLI and MCP
- Hoosh NL integration — client wired into CLI and MCP
- Structured JSON output — `--output json` emits JSON-lines events
- Variables & facts — `[vars]` in playbooks, `{{ var }}` expansion, `--var` CLI override, `--facts` gathers os/arch/hostname/distro/pkg_manager/init_system
- Validate hardening — checks unknown actions, not just unknown modules
- 70 tests across 5 crates + integration tests

### sutra-community — scaffolded

Separate repo (`MacCracken/sutra-community`) with 5 module crates:
- `sutra-nftables` — firewall rules via nftables (implemented)
- `sutra-sysctl` — kernel parameter tuning with persistence (implemented)
- `sutra-aegis` — AGNOS security policy enforcement (stub)
- `sutra-daimon` — AGNOS agent lifecycle and fleet reporting (stub)
- `sutra-edge` — edge node fleet operations (stub)

---

## Blocked — Waiting on AGNOS

| # | Item | Blocker | AGNOS Tracker |
|---|------|---------|---------------|
| 1 | Daimon HTTP transport (`ExecutorKind::Daimon`) | Needs `POST /v1/agents/{id}/exec` API in daimon | T1 |
| 2 | Daimon file transfer | Needs `PUT/GET /v1/agents/{id}/files/{path}` API | T2 |
| 3 | Fleet audit ingestion | Needs `POST /v1/audit/runs` in daimon | T3 |
| 4 | Hoosh playbook generation quality | Needs playbook-aware few-shot tuning in hoosh | T4 |
| 5 | sutra-community marketplace recipe | Needs `recipes/marketplace/sutra-community.toml` in AGNOS | T5 |

---

## v2 — "Linux-agnostic orchestration"

Goal: sutra works on any Linux distribution, not just AGNOS. Package managers, init
systems, and platform details are abstracted behind provider interfaces.

| # | Work Item | Notes |
|---|-----------|-------|
| 1 | Package provider abstraction — trait behind ark module: `ArkProvider`, `AptProvider`, `DnfProvider`, `PacmanProvider`, `ApkProvider` | Auto-detect or explicit `provider` field in task |
| 2 | Service provider abstraction — trait behind argonaut module: `ArgonautProvider`, `SystemdProvider`, `OpenRCProvider`, `RunitProvider` | Auto-detect from init system |
| 3 | OS fact gathering already detects distro, pkg_manager, init_system | Wire into provider auto-selection |
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
| 1 | Community module loader — dynamic or compile-time registration of external module crates | Plugin trait + registry |
| 2 | Handlers / hooks — pre_task, post_task, on_failure callbacks in playbooks | |
| 3 | Roles / includes — reusable task groups, playbook composition | |
| 4 | Secrets management — encrypted vars, vault integration | |
| 5 | Diff output — show file diffs, config diffs in plan output | |
| 6 | Web UI / dashboard — run history, fleet state, playbook browser | Could be separate AGNOS app |
| 7 | Marketplace integration — publish/consume playbooks via recipes/marketplace | |
| 8 | Conditional tasks — `when:` clauses based on facts, vars, previous results | |
| 9 | Notification / webhook — post run results to Slack, HTTP endpoint, etc. | |

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
