# ADR 004 — Error Recovery & Task Dependencies

**Status**: Accepted
**Date**: 2026-03-18

## Context

Infrastructure playbooks may need to tolerate partial failures (e.g., optional cleanup tasks) or enforce strict ordering between tasks.

## Decision

### Error recovery

Three strategies, configurable per-task and per-playbook:

| Strategy | Behavior |
|----------|----------|
| `fail` (default) | Abort the entire run on this node |
| `continue` | Log the error, mark run as failed, continue with next task |
| `ignore` | Treat the error as a non-event, run continues as successful |

Per-task `on_error` overrides the playbook-level default set in `[playbook]`.

```toml
[playbook]
name = "Deploy with cleanup"
on_error = "fail"

[[task]]
module = "ark"
action = "install"
package = "tarang"

[[task]]
module = "shell"
action = "run"
cmd = "cleanup-old-logs.sh"
on_error = "continue"
```

### Task dependencies

Tasks can declare `name` and `depends_on` fields:

```toml
[[task]]
name = "install-pkg"
module = "ark"
action = "install"
package = "tarang"

[[task]]
name = "enable-svc"
module = "argonaut"
action = "enable"
service = "tarang"
depends_on = ["install-pkg"]
```

Execution order is resolved via topological sort (Kahn's algorithm). Circular dependencies are detected and rejected at planning time.

Tasks without dependencies maintain their original playbook order.

## Consequences

- Playbooks can handle optional/best-effort tasks without aborting
- Task ordering is explicit and verifiable
- Circular dependencies produce clear error messages
- No implicit dependencies (e.g., module type doesn't imply ordering)
