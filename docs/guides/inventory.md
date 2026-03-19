# Inventory Guide

## Format

Inventories are TOML files listing target nodes:

```toml
[[node]]
id = "rpi-kitchen"
host = "192.168.1.50"
role = "edge"
arch = "aarch64"
tags = ["iot", "home"]
transport = "daimon"

[[node]]
id = "nuc-office"
host = "192.168.1.10"
role = "desktop"
arch = "x86_64"
tags = ["workstation"]
transport = "ssh"
ssh_user = "deploy"
ssh_port = 22
ssh_key = "/home/deploy/.ssh/id_ed25519"

[[node]]
id = "localhost"
host = "127.0.0.1"
transport = "local"
```

## Node Fields

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `id` | Yes | — | Unique node identifier |
| `host` | Yes | — | Hostname or IP address |
| `role` | No | `""` | Node role (for target filtering) |
| `arch` | No | `""` | Architecture: `x86_64`, `aarch64`, etc. |
| `tags` | No | `[]` | Tags for target filtering |
| `transport` | No | `"local"` | Transport: `local`, `ssh`, `daimon` |
| `ssh_user` | No | `"root"` | SSH username |
| `ssh_port` | No | `22` | SSH port |
| `ssh_key` | No | Auto-detect | Path to SSH private key |

## Transports

### Local

Executes commands on the current machine via `sh -c`. Default when no inventory is provided.

### SSH

Connects via russh with key-based authentication. Tries (in order):
1. Specified `ssh_key` path
2. `~/.ssh/id_ed25519`
3. `~/.ssh/id_rsa`

Connection is established lazily on first command execution.

### Daimon

Executes commands via the AGNOS daimon agent HTTP API (port 8090). Requires daimon remote exec endpoints (not yet available — tracked as T1/T2 in AGNOS roadmap).

## Target Filtering

Playbook `[[target]]` sections filter inventory nodes. A node matches if ALL specified fields match (AND within a target, OR across multiple targets):

```toml
# Matches: edge nodes on aarch64
[[target]]
role = "edge"
arch = "aarch64"

# Also matches: any node tagged "production"
[[target]]
tag = "production"
```

The `--limit` CLI flag further restricts to a single node by ID.

## No Inventory

When no `-i` flag is provided, sutra creates a single localhost node. This is the default for local-only operations.
