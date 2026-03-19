# JSON Output Events

When `--output json` is passed, sutra emits one JSON object per line (JSON-lines format). Events are tagged by `type`.

## Event Types

### run_start

Emitted once at the beginning of a run.

```json
{ "type": "run_start", "playbook": "Deploy tarang", "nodes": ["edge-1", "edge-2"] }
```

### node_start

Emitted when execution begins on a node.

```json
{ "type": "node_start", "node_id": "edge-1", "facts": { "os": "Linux", "arch": "aarch64" } }
```

`facts` is present when `--facts` is used, empty object otherwise.

### task_check

Emitted after idempotency check for each task.

```json
{ "type": "task_check", "node_id": "edge-1", "module": "ark", "action": "install", "met": false }
```

### task_plan

Emitted after planning each task.

```json
{
  "type": "task_plan",
  "node_id": "edge-1",
  "plan": { "module": "ark", "action": "install", "changed": true, "description": "install tarang 2026.3.18" }
}
```

### task_result

Emitted after applying each task (only with `--confirm`).

```json
{
  "type": "task_result",
  "node_id": "edge-1",
  "result": { "module": "ark", "action": "install", "success": true, "changed": true, "message": "ark install tarang — ok" }
}
```

### node_end

Emitted when execution completes on a node.

```json
{ "type": "node_end", "node_id": "edge-1", "success": true }
```

### run_end

Emitted once at the end of a run.

```json
{ "type": "run_end", "success": true, "changed": 3, "ok": 2, "failed": 0 }
```

## Usage

```bash
# Pipe to jq
sutra apply deploy.toml --output json | jq 'select(.type == "task_result")'

# Save structured log
sutra apply deploy.toml --confirm --output json > run-log.jsonl

# Check if any tasks failed
sutra apply deploy.toml --output json | jq -e 'select(.type == "run_end") | .success'
```
