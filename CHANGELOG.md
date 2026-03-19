# Changelog

## [2026.3.18] - 2026-03-18

### Added

#### Core
- **Executor** — concrete struct with enum dispatch (`ExecutorKind::Local`, `ExecutorKind::Ssh`), avoids async-dyn trait issues
- **SSH transport** — russh 0.48, lazy connect, ed25519/rsa key auth, `ssh_port`/`ssh_key` fields on NodeInfo
- **Variables & facts** — `[vars]` table in playbooks, `{{ var }}` and `{{ fact.key }}` expansion, `--var key=value` CLI override
- **Fact gathering** — `--facts` detects os, arch, hostname, distro, distro_version, pkg_manager, init_system
- **Error recovery** — `on_error: fail|continue|ignore` per-task and per-playbook
- **Task dependencies** — `name` + `depends_on` fields, topological sort via Kahn's algorithm, cycle detection
- **JSON output events** — `OutputEvent` enum: run_start, node_start, task_check, task_plan, task_result, node_end, run_end
- **Audit trail** — `RunRecord` persisted as JSON-lines to `~/.local/share/sutra/audit/`
- **Target filtering** — `target_matches()` filters nodes by role, arch, tag, node_id, all
- **Playbook parser** — TOML canonical format, YAML↔TOML conversion, Markdown section extraction
- **Param helpers** — `param_str()`, `param_int()`, `param_bool()`, `Task::new()`

#### Modules (6 core)
- **ark** — install, remove, upgrade, pin, list via `ark` CLI; queries installed state for idempotency
- **argonaut** — enable, disable, start, stop, restart, status via `argonaut` CLI; queries enabled/running state
- **file** — copy (content diffing), absent, permissions (chmod + chown), line_in_file (idempotent append), template (Tera engine with variable interpolation)
- **verify** — port_listening (ss/netstat), file_exists, service_running, http_ok (curl with timeout)
- **shell** — run, script; `creates`/`removes` params for idempotency guards
- **user** — present, absent, group_present, group_absent; useradd/userdel/groupadd/groupdel

#### CLI
- 9 subcommands: apply, check, plan, translate, convert, inventory, modules, validate, nl
- **`--output json`** — global flag, structured JSON-lines for all commands
- **`-j N` / `--parallel N`** — bounded concurrent node execution
- **`--continue-on-error`** — don't abort on node failure
- **`--var key=value`** — override playbook vars from CLI (repeatable)
- **`--facts`** — gather node facts before execution
- Dry-run by default, `--confirm` to execute

#### MCP (sutra-mcp)
- 6 tool handlers: `sutra_apply`, `sutra_plan`, `sutra_check`, `sutra_inventory`, `sutra_translate`, `sutra_convert`

#### AI Integration (sutra-ai)
- Markdown playbook parser (section extraction for hoosh translation)
- Daimon client (agent registration, fleet inventory)
- Hoosh client (NL to TOML translation)

#### Infrastructure
- Module registry with enum dispatch
- Local transport (shell exec, file copy/read)
- Transport trait for future SSH and daimon implementations
- Example playbooks (TOML, YAML, Markdown) and inventory
- CI/CD workflows (ci.yml, release.yml)
- 70 tests across 5 crates + integration tests
