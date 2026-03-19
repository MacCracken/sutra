# ADR 002 — Security Model

**Status**: Accepted
**Date**: 2026-03-18

## Context

Sutra executes shell commands with system-level privileges on local and remote nodes. Playbook parameters are user-supplied and flow into shell commands.

## Decision

### Shell escaping

All user-supplied parameters are escaped via `sutra_core::esc()` (wraps `shlex::try_quote()`) before interpolation into shell commands. This prevents command injection via malicious parameter values.

The `shell` module is intentionally unescaped — its purpose is to run arbitrary commands. Users accept this when using `module = "shell"`.

### Dry-run by default

`sutra apply` shows a plan without executing. `--confirm` is required to apply changes. This prevents accidental execution.

### Idempotency

Modules implement `check()` to determine if desired state is already met. The execution engine skips tasks where `check()` returns `true`. This prevents redundant changes and enables safe re-runs.

### Audit trail

Every confirmed run produces a `RunRecord` appended to `~/.local/share/sutra/audit/sutra-audit.jsonl`. Records include playbook path, node ID, timestamps, and per-task results.

Audit log integrity (HMAC signing) is deferred to post-v2.

### SSH host key validation

v1 accepts all SSH server keys (equivalent to `StrictHostKeyChecking=no`). This is acceptable for infrastructure tooling where nodes are in a managed inventory. Known_hosts validation is planned for v2.

### Playbook trust model

Playbooks are treated like code — users should review before executing. There is no sandboxing of playbook content. The `shell` module can run arbitrary commands by design.

## Consequences

- Command injection is mitigated for all modules except `shell` (which is intentionally an escape hatch)
- Users must trust playbooks before running with `--confirm`
- SSH connections on untrusted networks are vulnerable to MITM until known_hosts is implemented
- Audit logs can be tampered with on disk until signing is implemented
