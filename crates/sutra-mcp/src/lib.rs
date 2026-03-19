//! sutra-mcp — MCP server for Sutra orchestration tools.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// List all sutra MCP tools.
pub fn tool_list() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "sutra_apply".to_string(),
            description: "Apply a playbook (dry-run by default, pass confirm=true to execute)"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "playbook": { "type": "string", "description": "Path to TOML playbook" },
                    "confirm": { "type": "boolean", "description": "Execute changes (default: false, dry-run)" },
                    "limit": { "type": "string", "description": "Limit to specific node ID" }
                },
                "required": ["playbook"]
            }),
        },
        McpTool {
            name: "sutra_plan".to_string(),
            description: "Show detailed execution plan for a playbook".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "playbook": { "type": "string", "description": "Path to TOML playbook" }
                },
                "required": ["playbook"]
            }),
        },
        McpTool {
            name: "sutra_check".to_string(),
            description: "Verify current state matches desired playbook state".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "playbook": { "type": "string", "description": "Path to TOML playbook" }
                },
                "required": ["playbook"]
            }),
        },
        McpTool {
            name: "sutra_inventory".to_string(),
            description: "List all known nodes (static + daimon fleet)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "from_daimon": { "type": "boolean", "description": "Include daimon fleet nodes" }
                }
            }),
        },
        McpTool {
            name: "sutra_translate".to_string(),
            description: "Translate Markdown or natural language to a TOML playbook via hoosh"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Markdown content or natural language" },
                    "format": { "type": "string", "enum": ["markdown", "nl"], "description": "Input format" }
                },
                "required": ["input"]
            }),
        },
        McpTool {
            name: "sutra_convert".to_string(),
            description: "Convert between YAML and TOML playbook formats".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Playbook content to convert" },
                    "from": { "type": "string", "enum": ["yaml", "toml"], "description": "Source format" },
                    "to": { "type": "string", "enum": ["yaml", "toml"], "description": "Target format" }
                },
                "required": ["input", "from", "to"]
            }),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_list_count() {
        let tools = tool_list();
        assert_eq!(tools.len(), 6);
    }

    #[test]
    fn test_tool_names() {
        let tools = tool_list();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"sutra_apply"));
        assert!(names.contains(&"sutra_plan"));
        assert!(names.contains(&"sutra_check"));
        assert!(names.contains(&"sutra_inventory"));
        assert!(names.contains(&"sutra_translate"));
        assert!(names.contains(&"sutra_convert"));
    }

    #[test]
    fn test_tool_schemas_valid() {
        let tools = tool_list();
        for tool in &tools {
            assert!(tool.input_schema.is_object());
            assert!(tool.input_schema.get("type").is_some());
        }
    }

    #[test]
    fn test_apply_tool_schema() {
        let tools = tool_list();
        let apply = tools.iter().find(|t| t.name == "sutra_apply").unwrap();
        let required = apply.input_schema["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::json!("playbook")));
    }
}
