//! verify module — Post-task assertions (port listening, file exists, service running, HTTP OK).

use sutra_core::{NodeInfo, SutraModule, Task, TaskPlan, TaskResult};

pub struct VerifyModule;

impl SutraModule for VerifyModule {
    fn name(&self) -> &str {
        "verify"
    }

    fn actions(&self) -> &[&str] {
        &["port_listening", "file_exists", "service_running", "http_ok"]
    }

    async fn plan(&self, task: &Task, _node: &NodeInfo) -> anyhow::Result<TaskPlan> {
        let desc = match task.action.as_str() {
            "port_listening" => {
                let port = task
                    .params
                    .get("port")
                    .and_then(|v| v.as_integer())
                    .unwrap_or(0);
                format!("verify port {} is listening", port)
            }
            "file_exists" => {
                let path = task
                    .params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                format!("verify file {} exists", path)
            }
            "service_running" => {
                let service = task
                    .params
                    .get("service")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                format!("verify service {} is running", service)
            }
            "http_ok" => {
                let url = task
                    .params
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                format!("verify HTTP 200 from {}", url)
            }
            other => format!("verify {}", other),
        };

        Ok(TaskPlan {
            module: self.name().to_string(),
            action: task.action.clone(),
            changed: false,
            description: desc,
            diff: None,
        })
    }

    async fn apply(&self, task: &Task, node: &NodeInfo) -> anyhow::Result<TaskResult> {
        let plan = self.plan(task, node).await?;

        // TODO: Execute verification checks
        Ok(TaskResult {
            module: self.name().to_string(),
            action: task.action.clone(),
            success: true,
            changed: false,
            message: plan.description,
        })
    }

    async fn check(&self, _task: &Task, _node: &NodeInfo) -> anyhow::Result<bool> {
        // Verify modules are always "check" operations
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_node() -> NodeInfo {
        NodeInfo {
            id: "test".to_string(),
            host: "localhost".to_string(),
            role: "".to_string(),
            arch: "x86_64".to_string(),
            tags: vec![],
            transport: "local".to_string(),
            ssh_user: None,
        }
    }

    #[tokio::test]
    async fn test_verify_port() {
        let module = VerifyModule;
        let mut params = HashMap::new();
        params.insert("port".to_string(), toml::Value::Integer(8070));

        let task = Task {
            module: "verify".to_string(),
            action: "port_listening".to_string(),
            params,
        };

        let plan = module.plan(&task, &test_node()).await.unwrap();
        assert!(!plan.changed);
        assert!(plan.description.contains("8070"));
    }

    #[tokio::test]
    async fn test_verify_file() {
        let module = VerifyModule;
        let mut params = HashMap::new();
        params.insert(
            "path".to_string(),
            toml::Value::String("/etc/agnos/version".to_string()),
        );

        let task = Task {
            module: "verify".to_string(),
            action: "file_exists".to_string(),
            params,
        };

        let plan = module.plan(&task, &test_node()).await.unwrap();
        assert!(plan.description.contains("/etc/agnos/version"));
    }

    #[test]
    fn test_verify_actions() {
        let module = VerifyModule;
        assert_eq!(module.name(), "verify");
        assert!(module.actions().contains(&"port_listening"));
        assert!(module.actions().contains(&"http_ok"));
    }
}
