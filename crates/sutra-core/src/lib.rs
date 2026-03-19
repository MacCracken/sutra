//! sutra-core — Playbook parser, task graph, execution engine, and module trait.

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A parsed playbook — the unit of orchestration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playbook {
    pub playbook: PlaybookMeta,
    #[serde(default)]
    pub target: Vec<Target>,
    pub task: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// Targeting criteria — which nodes to run against.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub all: Option<bool>,
}

/// A single task within a playbook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub module: String,
    pub action: String,
    #[serde(flatten)]
    pub params: HashMap<String, toml::Value>,
}

/// Information about a target node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: String,
    pub host: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub arch: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_transport")]
    pub transport: String,
    #[serde(default)]
    pub ssh_user: Option<String>,
}

fn default_transport() -> String {
    "local".to_string()
}

/// What a module plans to do (dry-run output).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    pub module: String,
    pub action: String,
    pub changed: bool,
    pub description: String,
    #[serde(default)]
    pub diff: Option<String>,
}

/// Result of executing a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub module: String,
    pub action: String,
    pub success: bool,
    pub changed: bool,
    pub message: String,
}

/// Record of a playbook run for audit/rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: String,
    pub playbook: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub results: Vec<TaskResult>,
}

impl RunRecord {
    pub fn new(playbook: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            playbook: playbook.to_string(),
            started_at: Utc::now(),
            finished_at: None,
            results: Vec::new(),
        }
    }
}

/// The module trait — every orchestration module implements this.
#[allow(async_fn_in_trait)]
pub trait SutraModule: Send + Sync {
    /// Module name (e.g. "ark", "argonaut", "file").
    fn name(&self) -> &str;

    /// Supported actions (e.g. ["install", "remove", "upgrade"]).
    fn actions(&self) -> &[&str];

    /// Plan: return the diff between current and desired state.
    async fn plan(&self, task: &Task, node: &NodeInfo) -> anyhow::Result<TaskPlan>;

    /// Apply: execute the change. Only called after user confirmation.
    async fn apply(&self, task: &Task, node: &NodeInfo) -> anyhow::Result<TaskResult>;

    /// Check: is desired state already met? (idempotency guard)
    async fn check(&self, task: &Task, node: &NodeInfo) -> anyhow::Result<bool>;
}

/// Inventory — collection of nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    #[serde(default)]
    pub node: Vec<NodeInfo>,
}

/// Parse a playbook from a TOML file.
pub fn parse_playbook(path: &Path) -> anyhow::Result<Playbook> {
    let content = std::fs::read_to_string(path)?;
    let playbook: Playbook = toml::from_str(&content)?;
    Ok(playbook)
}

/// Parse an inventory from a TOML file.
pub fn parse_inventory(path: &Path) -> anyhow::Result<Inventory> {
    let content = std::fs::read_to_string(path)?;
    let inventory: Inventory = toml::from_str(&content)?;
    Ok(inventory)
}

/// Convert a YAML playbook string to canonical TOML.
pub fn yaml_to_toml(yaml_str: &str) -> anyhow::Result<String> {
    let value: serde_yaml::Value = serde_yaml::from_str(yaml_str)?;
    let toml_value = yaml_value_to_toml_value(&value)?;
    let toml_str = toml::to_string_pretty(&toml_value)?;
    Ok(toml_str)
}

/// Convert a TOML playbook string to YAML.
pub fn toml_to_yaml(toml_str: &str) -> anyhow::Result<String> {
    let value: toml::Value = toml::from_str(toml_str)?;
    let yaml_value = toml_value_to_yaml_value(&value);
    let yaml_str = serde_yaml::to_string(&yaml_value)?;
    Ok(yaml_str)
}

fn yaml_value_to_toml_value(yaml: &serde_yaml::Value) -> anyhow::Result<toml::Value> {
    match yaml {
        serde_yaml::Value::Null => Ok(toml::Value::String(String::new())),
        serde_yaml::Value::Bool(b) => Ok(toml::Value::Boolean(*b)),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(toml::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(toml::Value::Float(f))
            } else {
                Ok(toml::Value::String(n.to_string()))
            }
        }
        serde_yaml::Value::String(s) => Ok(toml::Value::String(s.clone())),
        serde_yaml::Value::Sequence(seq) => {
            let arr: Result<Vec<_>, _> = seq.iter().map(yaml_value_to_toml_value).collect();
            Ok(toml::Value::Array(arr?))
        }
        serde_yaml::Value::Mapping(map) => {
            let mut table = toml::map::Map::new();
            for (k, v) in map {
                let key = match k {
                    serde_yaml::Value::String(s) => s.clone(),
                    other => serde_yaml::to_string(other)?.trim().to_string(),
                };
                table.insert(key, yaml_value_to_toml_value(v)?);
            }
            Ok(toml::Value::Table(table))
        }
        serde_yaml::Value::Tagged(tagged) => yaml_value_to_toml_value(&tagged.value),
    }
}

fn toml_value_to_yaml_value(toml_val: &toml::Value) -> serde_yaml::Value {
    match toml_val {
        toml::Value::String(s) => serde_yaml::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_yaml::Value::Number((*i).into()),
        toml::Value::Float(f) => {
            serde_yaml::Value::Number(serde_yaml::Number::from(*f))
        }
        toml::Value::Boolean(b) => serde_yaml::Value::Bool(*b),
        toml::Value::Datetime(dt) => serde_yaml::Value::String(dt.to_string()),
        toml::Value::Array(arr) => {
            let seq: Vec<_> = arr.iter().map(toml_value_to_yaml_value).collect();
            serde_yaml::Value::Sequence(seq)
        }
        toml::Value::Table(table) => {
            let mut map = serde_yaml::Mapping::new();
            for (k, v) in table {
                map.insert(
                    serde_yaml::Value::String(k.clone()),
                    toml_value_to_yaml_value(v),
                );
            }
            serde_yaml::Value::Mapping(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_playbook() {
        let toml_content = r#"
[playbook]
name = "Test playbook"
description = "A test"

[[target]]
role = "edge"
arch = "aarch64"

[[task]]
module = "ark"
action = "install"
package = "tarang"
version = "2026.3.18"

[[task]]
module = "verify"
action = "port_listening"
port = 8070
"#;
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(toml_content.as_bytes()).unwrap();
        let pb = parse_playbook(f.path()).unwrap();
        assert_eq!(pb.playbook.name, "Test playbook");
        assert_eq!(pb.target.len(), 1);
        assert_eq!(pb.target[0].role, Some("edge".to_string()));
        assert_eq!(pb.task.len(), 2);
        assert_eq!(pb.task[0].module, "ark");
        assert_eq!(pb.task[1].module, "verify");
    }

    #[test]
    fn test_parse_inventory() {
        let toml_content = r#"
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
ssh_user = "user"
"#;
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(toml_content.as_bytes()).unwrap();
        let inv = parse_inventory(f.path()).unwrap();
        assert_eq!(inv.node.len(), 2);
        assert_eq!(inv.node[0].id, "rpi-kitchen");
        assert_eq!(inv.node[0].transport, "daimon");
        assert_eq!(inv.node[1].ssh_user, Some("user".to_string()));
    }

    #[test]
    fn test_yaml_to_toml_roundtrip() {
        let yaml = r#"
playbook:
  name: Deploy tarang
  description: Install on edge fleet
task:
  - module: ark
    action: install
    package: tarang
"#;
        let toml_str = yaml_to_toml(yaml).unwrap();
        assert!(toml_str.contains("Deploy tarang"));
        assert!(toml_str.contains("ark"));

        // Round-trip back
        let yaml_again = toml_to_yaml(&toml_str).unwrap();
        assert!(yaml_again.contains("Deploy tarang"));
    }

    #[test]
    fn test_toml_to_yaml() {
        let toml_str = r#"
[playbook]
name = "Test"
description = "A test playbook"

[[task]]
module = "file"
action = "template"
path = "/etc/agnos/config.toml"
"#;
        let yaml = toml_to_yaml(toml_str).unwrap();
        assert!(yaml.contains("Test"));
        assert!(yaml.contains("file"));
        assert!(yaml.contains("template"));
    }

    #[test]
    fn test_run_record() {
        let record = RunRecord::new("deploy-tarang.toml");
        assert!(!record.id.is_empty());
        assert_eq!(record.playbook, "deploy-tarang.toml");
        assert!(record.finished_at.is_none());
        assert!(record.results.is_empty());
    }

    #[test]
    fn test_target_all() {
        let target = Target {
            role: None,
            arch: None,
            node_id: None,
            tag: None,
            all: Some(true),
        };
        assert_eq!(target.all, Some(true));
    }

    #[test]
    fn test_task_plan_no_change() {
        let plan = TaskPlan {
            module: "ark".to_string(),
            action: "install".to_string(),
            changed: false,
            description: "tarang 2026.3.18 already installed".to_string(),
            diff: None,
        };
        assert!(!plan.changed);
    }

    #[test]
    fn test_task_result_success() {
        let result = TaskResult {
            module: "argonaut".to_string(),
            action: "enable".to_string(),
            success: true,
            changed: true,
            message: "tarang.service enabled".to_string(),
        };
        assert!(result.success);
        assert!(result.changed);
    }
}
