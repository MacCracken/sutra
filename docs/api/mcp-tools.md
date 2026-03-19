# MCP Tools

Sutra exposes 6 MCP tools for AI agent integration. Tools are defined in `sutra-mcp` with full request/response handlers.

## sutra_apply

Apply a playbook (dry-run by default).

**Input**:
```json
{
  "playbook": "/path/to/playbook.toml",
  "confirm": false,
  "limit": "node-id"
}
```

**Output**:
```json
{
  "playbook": "Deploy tarang",
  "confirm": false,
  "results": [
    {
      "module": "ark",
      "action": "install",
      "changed": true,
      "description": "install tarang 2026.3.18",
      "dry_run": true
    }
  ]
}
```

## sutra_plan

Show detailed execution plan.

**Input**: `{ "playbook": "/path/to/playbook.toml" }`

**Output**:
```json
{
  "playbook": "Deploy tarang",
  "plans": [
    { "module": "ark", "action": "install", "changed": true, "description": "..." }
  ]
}
```

## sutra_check

Verify current state matches desired.

**Input**: `{ "playbook": "/path/to/playbook.toml" }`

**Output**:
```json
{
  "playbook": "Deploy tarang",
  "all_ok": false,
  "checks": [
    { "module": "ark", "action": "install", "met": false },
    { "module": "argonaut", "action": "enable", "met": true }
  ]
}
```

## sutra_inventory

List known nodes.

**Input**: `{ "from_daimon": true }`

**Output**:
```json
{
  "nodes": [
    { "id": "local", "host": "localhost", "transport": "local" },
    { "id": "rpi-01", "host": "192.168.1.50", "role": "edge", "arch": "aarch64" }
  ]
}
```

## sutra_translate

Translate Markdown or natural language to TOML via hoosh.

**Input**:
```json
{
  "input": "Install tarang on all edge nodes",
  "format": "nl"
}
```

**Output** (NL): `{ "toml": "[playbook]\nname = ..." }`

**Output** (Markdown): `{ "name": "...", "prompt": "...", "note": "..." }`

## sutra_convert

Convert between YAML and TOML formats.

**Input**:
```json
{
  "input": "playbook:\n  name: Test\ntask:\n  - module: ark\n    action: install\n",
  "from": "yaml",
  "to": "toml"
}
```

**Output**: `{ "output": "[playbook]\nname = \"Test\"\n..." }`
