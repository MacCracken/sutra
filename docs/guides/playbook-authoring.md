# Playbook Authoring Guide

## Playbook Structure

```toml
[playbook]
name = "Deploy tarang to edge fleet"
description = "Optional description"
on_error = "fail"  # fail | continue | ignore

[vars]
version = "2026.3.18"
config_dir = "/etc/tarang"

[[target]]
role = "edge"
arch = "aarch64"

[[task]]
name = "install-tarang"
module = "ark"
action = "install"
package = "tarang"
version = "{{ version }}"

[[task]]
module = "argonaut"
action = "enable"
service = "tarang"
depends_on = ["install-tarang"]
```

## Sections

### `[playbook]`

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Human-readable name |
| `description` | No | What this playbook does |
| `on_error` | No | Default error strategy: `fail`, `continue`, `ignore` |

### `[vars]`

Key-value pairs available as `{{ key }}` in task parameters and Tera templates. CLI `--var key=value` overrides playbook vars.

### `[[target]]`

Filters which inventory nodes this playbook runs against. Multiple targets are OR'd. Empty targets match all nodes.

| Field | Description |
|-------|-------------|
| `role` | Match nodes with this role |
| `arch` | Match nodes with this architecture |
| `node_id` | Match a specific node by ID |
| `tag` | Match nodes with this tag |
| `all` | `true` to match all nodes |

### `[[task]]`

| Field | Required | Description |
|-------|----------|-------------|
| `module` | Yes | Module name (ark, argonaut, file, shell, user, verify) |
| `action` | Yes | Module action |
| `name` | No | Label for `depends_on` references |
| `depends_on` | No | List of task names that must complete first |
| `on_error` | No | Override playbook-level error strategy |
| (other fields) | Varies | Module-specific parameters |

## Variables

Use `{{ var_name }}` syntax in any string parameter:

```toml
[vars]
version = "2026.3.18"

[[task]]
module = "ark"
action = "install"
package = "tarang"
version = "{{ version }}"
```

Node facts are available as `{{ fact.key }}` when `--facts` is passed:

```toml
[[task]]
module = "file"
action = "copy"
path = "/etc/hostname"
content = "{{ fact.hostname }}"
```

Available facts: `os`, `arch`, `hostname`, `distro`, `distro_version`, `distro_name`, `pkg_manager`, `init_system`.

## Task Dependencies

```toml
[[task]]
name = "create-dir"
module = "shell"
action = "run"
cmd = "mkdir -p /opt/app"
creates = "/opt/app"

[[task]]
module = "file"
action = "copy"
path = "/opt/app/config.toml"
content = "key = 'value'"
depends_on = ["create-dir"]
```

Tasks without `depends_on` run in playbook order. Circular dependencies are rejected.

## Error Handling

```toml
# This task can fail without aborting the run
[[task]]
module = "shell"
action = "run"
cmd = "cleanup-old-logs.sh"
on_error = "continue"
```

## Idempotency

Most modules check current state before making changes. The shell module uses `creates` and `removes` for idempotency guards:

```toml
[[task]]
module = "shell"
action = "run"
cmd = "wget -O /opt/app.tar.gz https://example.com/app.tar.gz"
creates = "/opt/app.tar.gz"
```

## Format Conversion

Write in YAML, convert to TOML:

```bash
sutra convert playbook.yaml --to toml -o playbook.toml
```

Write in Markdown, extract for NL translation:

```bash
sutra translate playbook.md
sutra nl "install tarang on edge nodes"
```
