# Module Reference

## ark

Package management via AGNOS ark (v2: cross-distro providers).

| Action | Params | Idempotent | Description |
|--------|--------|------------|-------------|
| `install` | `package`, `version` (default: latest) | Yes | Install package if not present or wrong version |
| `remove` | `package` | Yes | Remove package if installed |
| `upgrade` | `package`, `version` | No | Upgrade to specified version |
| `pin` | `package`, `version` | No | Pin package at version |
| `list` | — | Read-only | List installed packages |

## argonaut

Service management via AGNOS argonaut (v2: systemd/openrc providers).

| Action | Params | Idempotent | Description |
|--------|--------|------------|-------------|
| `enable` | `service` | Yes | Enable service for boot start |
| `disable` | `service` | Yes | Disable service |
| `start` | `service` | Yes | Start service if not running |
| `stop` | `service` | Yes | Stop service if running |
| `restart` | `service` | No | Restart service |
| `status` | `service` | Read-only | Query service status |

## file

File state management with Tera templating.

| Action | Params | Idempotent | Description |
|--------|--------|------------|-------------|
| `copy` | `path`, `content`, `mode` (0o644) | Yes | Write content to file |
| `template` | `path`, `src`, `mode`, + template vars | Yes | Render Tera template to file |
| `absent` | `path` | Yes | Remove file if exists |
| `permissions` | `path`, `mode`, `owner`, `group` | No | Set file permissions/ownership |
| `line_in_file` | `path`, `line`, `mode` | Yes | Ensure line is present in file |

### Template Variables

For `template` action, all task params except `path`, `src`, `mode`, `module`, `action` are passed as Tera template variables:

```toml
[[task]]
module = "file"
action = "template"
src = "templates/config.toml.tera"
path = "/etc/app/config.toml"
port = 8080
hostname = "{{ fact.hostname }}"
```

Template file (`config.toml.tera`):
```
listen_port = {{ port }}
hostname = "{{ hostname }}"
```

## shell

Arbitrary command execution. Not idempotent unless `creates` or `removes` is specified.

| Action | Params | Idempotent | Description |
|--------|--------|------------|-------------|
| `run` | `cmd`, `creates`, `removes` | With guards | Execute a shell command |
| `script` | `src`, `creates`, `removes` | With guards | Read and execute a script file |

### Idempotency Guards

| Param | Behavior |
|-------|----------|
| `creates` | Skip if this file path exists |
| `removes` | Skip if this file path does NOT exist |

```toml
[[task]]
module = "shell"
action = "run"
cmd = "wget -O /opt/app.tar.gz https://example.com/app.tar.gz"
creates = "/opt/app.tar.gz"
```

## user

User and group management.

| Action | Params | Idempotent | Description |
|--------|--------|------------|-------------|
| `present` | `username`, `shell`, `home`, `group`, `system` | Yes | Ensure user exists |
| `absent` | `username`, `remove_home` | Yes | Ensure user does not exist |
| `group_present` | `group`, `system` | Yes | Ensure group exists |
| `group_absent` | `group` | Yes | Ensure group does not exist |

## verify

Post-task assertions. These check state without modifying it. A failed verify causes the task to report failure.

| Action | Params | Description |
|--------|--------|-------------|
| `port_listening` | `port` | Check if a TCP port is listening |
| `file_exists` | `path` | Check if a file exists |
| `service_running` | `service` | Check if a service is running |
| `http_ok` | `url`, `timeout` (5s) | Check if URL returns HTTP 200 |
