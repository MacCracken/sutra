# CLI Reference

## Global Options

| Flag | Description |
|------|-------------|
| `--output json` | Emit structured JSON-lines instead of human-readable output |
| `--version` | Show version |
| `--help` | Show help |

## Commands

### `sutra apply <playbook>`

Apply a playbook. Dry-run by default.

| Flag | Description |
|------|-------------|
| `--confirm` | Execute changes (without this, only shows plan) |
| `--limit <node_id>` | Restrict to a specific node |
| `-i, --inventory <file>` | Inventory file (default: localhost) |
| `-j, --parallel <N>` | Run on N nodes concurrently |
| `--continue-on-error` | Don't abort when a node fails |
| `--var key=value` | Set/override a playbook variable (repeatable) |
| `--facts` | Gather node facts before execution |

```bash
# Dry-run
sutra apply deploy.toml

# Execute
sutra apply deploy.toml --confirm

# Execute on fleet, 4 nodes at a time
sutra apply deploy.toml -i fleet.toml --confirm -j 4

# Override a variable
sutra apply deploy.toml --confirm --var version=2026.3.19
```

### `sutra check <playbook>`

Verify current state matches desired state. Exits 1 if drift detected.

| Flag | Description |
|------|-------------|
| `-i, --inventory <file>` | Inventory file |
| `--var key=value` | Set/override a variable |

```bash
sutra check deploy.toml
sutra check deploy.toml -i fleet.toml
```

### `sutra plan <playbook>`

Show detailed execution plan with per-task change status.

| Flag | Description |
|------|-------------|
| `-i, --inventory <file>` | Inventory file |
| `--var key=value` | Set/override a variable |
| `--facts` | Gather node facts before planning |

### `sutra validate <playbook>`

Validate playbook syntax, module names, and action names. Exits 1 on errors.

```bash
sutra validate deploy.toml
```

### `sutra modules`

List all available modules and their actions.

### `sutra inventory`

List nodes from an inventory file or daimon fleet.

| Flag | Description |
|------|-------------|
| `-f, --file <path>` | Inventory file |
| `--from-daimon` | Fetch fleet nodes from daimon (port 8090) |

### `sutra convert <input> --to <format>`

Convert between YAML and TOML playbook formats.

| Flag | Description |
|------|-------------|
| `--to yaml\|toml` | Target format |
| `-o, --output <path>` | Output file (default: stdout) |

### `sutra translate <markdown>`

Extract sections from a Markdown playbook for hoosh translation.

### `sutra nl <prompt...>`

Translate natural language to a TOML playbook via hoosh (port 8088).

| Flag | Description |
|------|-------------|
| `-o, --output <path>` | Output file (default: stdout) |

```bash
sutra nl "install tarang on all edge nodes"
```

## JSON Output

All commands support `--output json`. Events are emitted as JSON-lines:

```bash
sutra apply deploy.toml --output json | jq '.type'
```

Event types: `run_start`, `node_start`, `task_check`, `task_plan`, `task_result`, `node_end`, `run_end`.

## Environment Variables

| Variable | Description |
|----------|-------------|
| `SUTRA_AUDIT_DIR` | Override audit log directory (default: `~/.local/share/sutra/audit`) |
| `RUST_LOG` | Set log level (e.g., `debug`, `info`, `warn`) |
