# ADR 003 — Core vs Community Module Split

**Status**: Accepted
**Date**: 2026-03-18

## Context

Sutra needs to support both generic Linux infrastructure operations and AGNOS-specific features. Bundling everything in one binary increases compile time and pulls in unnecessary dependencies for non-AGNOS users.

## Decision

### Core modules (ship with sutra)

Modules that work on any Linux box:

| Module | Purpose |
|--------|---------|
| `ark` | Package management (AGNOS ark, v2: apt/dnf/pacman/apk providers) |
| `argonaut` | Service management (AGNOS argonaut, v2: systemd/openrc providers) |
| `file` | File state (copy, template, absent, permissions, line_in_file) |
| `shell` | Arbitrary command execution |
| `user` | User and group management |
| `verify` | Post-task assertions |

### Community modules (sutra-community repo)

Modules that are domain-specific or AGNOS-specific:

| Module | Domain |
|--------|--------|
| `nftables` | Firewall rules |
| `sysctl` | Kernel parameters |
| `aegis` | AGNOS security policy |
| `daimon` | AGNOS agent lifecycle |
| `edge` | AGNOS edge fleet operations |

Future community modules: Docker/OCI, cloud providers, database management.

### Integration model

Community modules depend on `sutra-core` for the `SutraModule` trait and `Executor`. They are compiled separately and (in post-v2) loaded via a plugin registry.

## Consequences

- Sutra has standalone utility beyond AGNOS
- AGNOS integration is a first-party community pack, not baked into the binary
- Community modules can iterate independently from the core release cycle
- v2 provider abstraction will make core modules truly distro-agnostic
