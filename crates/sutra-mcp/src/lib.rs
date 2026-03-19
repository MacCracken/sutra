//! sutra-mcp — MCP server for Sutra orchestration tools.
//!
//! Provides tool definitions and handlers for the 6 sutra MCP tools.
//! These allow AI agents (via MCP protocol) to drive sutra orchestration.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sutra_core::{
    order_tasks, parse_playbook, resolve_on_error, yaml_to_toml, Executor, NodeInfo, OnError,
    SutraModule, VarContext,
};
use sutra_modules::ModuleRegistry;

/// MCP tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// MCP tool call result.
#[derive(Debug, Serialize)]
pub struct McpResult {
    pub success: bool,
    pub content: Value,
}

impl McpResult {
    fn ok(content: Value) -> Self {
        Self {
            success: true,
            content,
        }
    }
    fn err(msg: &str) -> Self {
        Self {
            success: false,
            content: serde_json::json!({ "error": msg }),
        }
    }
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

/// Handle an MCP tool call.
pub async fn handle_tool(name: &str, params: &Value) -> McpResult {
    match name {
        "sutra_apply" => handle_apply(params).await,
        "sutra_plan" => handle_plan(params).await,
        "sutra_check" => handle_check(params).await,
        "sutra_inventory" => handle_inventory(params).await,
        "sutra_translate" => handle_translate(params).await,
        "sutra_convert" => handle_convert(params),
        _ => McpResult::err(&format!("unknown tool: {}", name)),
    }
}

async fn handle_apply(params: &Value) -> McpResult {
    let Some(playbook_path) = params.get("playbook").and_then(|v| v.as_str()) else {
        return McpResult::err("missing required parameter: playbook");
    };
    let confirm = params
        .get("confirm")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let pb = match parse_playbook(Path::new(playbook_path)) {
        Ok(pb) => pb,
        Err(e) => return McpResult::err(&format!("parse error: {}", e)),
    };

    let registry = ModuleRegistry::new();
    let node = NodeInfo::localhost();
    let exec = Executor::for_node(&node);
    let var_ctx = VarContext::new(&pb.vars, &[]);
    let task_order = match order_tasks(&pb.task) {
        Ok(o) => o,
        Err(e) => return McpResult::err(&format!("task ordering error: {}", e)),
    };

    let mut results = Vec::new();
    for &idx in &task_order {
        let task = &pb.task[idx];
        let expanded = var_ctx.expand_task(task);
        let err_strategy = resolve_on_error(&expanded, &pb.playbook.on_error);

        let Some(module) = registry.get(&expanded.module) else {
            results.push(serde_json::json!({
                "module": expanded.module,
                "action": expanded.action,
                "error": "unknown module"
            }));
            continue;
        };

        let already = module.check(&expanded, &node, &exec).await.unwrap_or(false);
        if already {
            results.push(serde_json::json!({
                "module": expanded.module,
                "action": expanded.action,
                "changed": false,
                "message": "already in desired state"
            }));
            continue;
        }

        let plan = match module.plan(&expanded, &node, &exec).await {
            Ok(p) => p,
            Err(e) => {
                results.push(serde_json::json!({
                    "module": expanded.module,
                    "action": expanded.action,
                    "error": e.to_string()
                }));
                continue;
            }
        };

        if confirm && plan.changed {
            match module.apply(&expanded, &node, &exec).await {
                Ok(r) => {
                    let success = r.success;
                    results.push(serde_json::json!({
                        "module": r.module,
                        "action": r.action,
                        "success": r.success,
                        "changed": r.changed,
                        "message": r.message
                    }));
                    if !success && *err_strategy == OnError::Fail {
                        break;
                    }
                }
                Err(e) => {
                    results.push(serde_json::json!({
                        "module": expanded.module,
                        "action": expanded.action,
                        "error": e.to_string()
                    }));
                    if *err_strategy == OnError::Fail {
                        break;
                    }
                }
            }
        } else {
            results.push(serde_json::json!({
                "module": plan.module,
                "action": plan.action,
                "changed": plan.changed,
                "description": plan.description,
                "dry_run": true
            }));
        }
    }

    McpResult::ok(serde_json::json!({
        "playbook": pb.playbook.name,
        "confirm": confirm,
        "results": results
    }))
}

async fn handle_plan(params: &Value) -> McpResult {
    let Some(playbook_path) = params.get("playbook").and_then(|v| v.as_str()) else {
        return McpResult::err("missing required parameter: playbook");
    };

    let pb = match parse_playbook(Path::new(playbook_path)) {
        Ok(pb) => pb,
        Err(e) => return McpResult::err(&format!("parse error: {}", e)),
    };

    let registry = ModuleRegistry::new();
    let node = NodeInfo::localhost();
    let exec = Executor::for_node(&node);
    let var_ctx = VarContext::new(&pb.vars, &[]);
    let task_order = match order_tasks(&pb.task) {
        Ok(o) => o,
        Err(e) => return McpResult::err(&format!("task ordering error: {}", e)),
    };

    let mut plans = Vec::new();
    for &idx in &task_order {
        let task = &pb.task[idx];
        let expanded = var_ctx.expand_task(task);
        if let Some(module) = registry.get(&expanded.module) {
            match module.plan(&expanded, &node, &exec).await {
                Ok(plan) => plans.push(serde_json::to_value(&plan).unwrap()),
                Err(e) => plans.push(serde_json::json!({
                    "module": expanded.module,
                    "error": e.to_string()
                })),
            }
        }
    }

    McpResult::ok(serde_json::json!({
        "playbook": pb.playbook.name,
        "plans": plans
    }))
}

async fn handle_check(params: &Value) -> McpResult {
    let Some(playbook_path) = params.get("playbook").and_then(|v| v.as_str()) else {
        return McpResult::err("missing required parameter: playbook");
    };

    let pb = match parse_playbook(Path::new(playbook_path)) {
        Ok(pb) => pb,
        Err(e) => return McpResult::err(&format!("parse error: {}", e)),
    };

    let registry = ModuleRegistry::new();
    let node = NodeInfo::localhost();
    let exec = Executor::for_node(&node);
    let var_ctx = VarContext::new(&pb.vars, &[]);

    let mut checks = Vec::new();
    let mut all_ok = true;
    for task in &pb.task {
        let expanded = var_ctx.expand_task(task);
        if let Some(module) = registry.get(&expanded.module) {
            let met = module.check(&expanded, &node, &exec).await.unwrap_or(false);
            if !met {
                all_ok = false;
            }
            checks.push(serde_json::json!({
                "module": expanded.module,
                "action": expanded.action,
                "met": met
            }));
        }
    }

    McpResult::ok(serde_json::json!({
        "playbook": pb.playbook.name,
        "all_ok": all_ok,
        "checks": checks
    }))
}

async fn handle_inventory(params: &Value) -> McpResult {
    let from_daimon = params
        .get("from_daimon")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut nodes = vec![serde_json::json!({
        "id": "local",
        "host": "localhost",
        "transport": "local"
    })];

    if from_daimon {
        let client =
            sutra_ai::daimon::DaimonClient::new(sutra_ai::daimon::DaimonConfig::default());
        if let Ok(fleet) = client.fetch_fleet_nodes().await {
            for n in fleet {
                nodes.push(serde_json::json!({
                    "id": n.id,
                    "host": n.host,
                    "role": n.role,
                    "arch": n.arch
                }));
            }
        }
    }

    McpResult::ok(serde_json::json!({ "nodes": nodes }))
}

async fn handle_translate(params: &Value) -> McpResult {
    let Some(input) = params.get("input").and_then(|v| v.as_str()) else {
        return McpResult::err("missing required parameter: input");
    };
    let format = params
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("nl");

    match format {
        "markdown" => {
            let sections = sutra_ai::markdown::extract_sections(input);
            let prompt = sections.to_prompt();
            McpResult::ok(serde_json::json!({
                "name": sections.name,
                "prompt": prompt,
                "note": "Use sutra nl or hoosh to generate TOML from this prompt"
            }))
        }
        _ => {
            let client =
                sutra_ai::daimon::HooshClient::new(sutra_ai::daimon::HooshConfig::default());
            match client.nl_to_toml(input).await {
                Ok(toml) => McpResult::ok(serde_json::json!({ "toml": toml })),
                Err(e) => McpResult::err(&format!("hoosh unavailable: {}", e)),
            }
        }
    }
}

fn handle_convert(params: &Value) -> McpResult {
    let Some(input) = params.get("input").and_then(|v| v.as_str()) else {
        return McpResult::err("missing required parameter: input");
    };
    let Some(to) = params.get("to").and_then(|v| v.as_str()) else {
        return McpResult::err("missing required parameter: to");
    };

    let result = match to {
        "toml" => yaml_to_toml(input),
        "yaml" => sutra_core::toml_to_yaml(input),
        _ => return McpResult::err(&format!("unsupported format: {}", to)),
    };

    match result {
        Ok(converted) => McpResult::ok(serde_json::json!({ "output": converted })),
        Err(e) => McpResult::err(&format!("conversion error: {}", e)),
    }
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

    #[test]
    fn test_convert_yaml_to_toml() {
        let params = serde_json::json!({
            "input": "playbook:\n  name: Test\ntask:\n  - module: ark\n    action: install\n    package: test\n",
            "from": "yaml",
            "to": "toml"
        });
        let result = handle_convert(&params);
        assert!(result.success);
        let output = result.content["output"].as_str().unwrap();
        assert!(output.contains("Test"));
    }

    #[test]
    fn test_handle_unknown_tool() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(handle_tool("nonexistent", &serde_json::json!({})));
        assert!(!result.success);
    }
}
