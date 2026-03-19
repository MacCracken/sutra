//! verify module — Post-task assertions (port listening, file exists, service running, HTTP OK).

use sutra_core::{param_int, param_str, Executor, NodeInfo, SutraModule, Task, TaskPlan, TaskResult};

pub struct VerifyModule;

impl SutraModule for VerifyModule {
    fn name(&self) -> &str {
        "verify"
    }

    fn actions(&self) -> &[&str] {
        &["port_listening", "file_exists", "service_running", "http_ok"]
    }

    async fn plan(
        &self,
        task: &Task,
        _node: &NodeInfo,
        _exec: &Executor,
    ) -> anyhow::Result<TaskPlan> {
        let desc = match task.action.as_str() {
            "port_listening" => {
                let port = param_int(task, "port", 0);
                format!("verify port {} is listening", port)
            }
            "file_exists" => {
                let path = param_str(task, "path", "unknown");
                format!("verify file {} exists", path)
            }
            "service_running" => {
                let service = param_str(task, "service", "unknown");
                format!("verify service {} is running", service)
            }
            "http_ok" => {
                let url = param_str(task, "url", "unknown");
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

    async fn apply(
        &self,
        task: &Task,
        _node: &NodeInfo,
        exec: &Executor,
    ) -> anyhow::Result<TaskResult> {
        let (success, message) = match task.action.as_str() {
            "port_listening" => {
                let port = param_int(task, "port", 0);
                let result = exec
                    .exec(&format!(
                        "ss -tlnp 2>/dev/null | grep -q ':{} ' || netstat -tlnp 2>/dev/null | grep -q ':{} '",
                        port, port
                    ))
                    .await?;
                if result.success() {
                    (true, format!("port {} is listening", port))
                } else {
                    (false, format!("port {} is NOT listening", port))
                }
            }
            "file_exists" => {
                let path = param_str(task, "path", "");
                let exists = exec.file_exists(path).await?;
                if exists {
                    (true, format!("{} exists", path))
                } else {
                    (false, format!("{} does NOT exist", path))
                }
            }
            "service_running" => {
                let service = param_str(task, "service", "");
                let result = exec
                    .exec(&format!("argonaut status {} 2>/dev/null", service))
                    .await?;
                let running = result.success() && result.stdout.contains("running");
                if running {
                    (true, format!("{} is running", service))
                } else {
                    (false, format!("{} is NOT running", service))
                }
            }
            "http_ok" => {
                let url = param_str(task, "url", "");
                let timeout = param_int(task, "timeout", 5);
                let result = exec
                    .exec(&format!(
                        "curl -sf -o /dev/null -w '%{{http_code}}' --max-time {} {}",
                        timeout, url
                    ))
                    .await?;
                let code = result.stdout.trim();
                if result.success() && code == "200" {
                    (true, format!("HTTP 200 from {}", url))
                } else {
                    (
                        false,
                        format!("HTTP check failed for {} (got {})", url, code),
                    )
                }
            }
            other => (false, format!("unknown verify action: {}", other)),
        };

        Ok(TaskResult {
            module: self.name().to_string(),
            action: task.action.clone(),
            success,
            changed: false,
            message,
        })
    }

    async fn check(
        &self,
        task: &Task,
        node: &NodeInfo,
        exec: &Executor,
    ) -> anyhow::Result<bool> {
        // For verify, check == apply (they're both assertions).
        let result = self.apply(task, node, exec).await?;
        Ok(result.success)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_node() -> NodeInfo {
        NodeInfo::localhost()
    }

    #[tokio::test]
    async fn test_verify_file_exists() {
        let module = VerifyModule;
        let exec = Executor::local();
        let mut params = HashMap::new();
        params.insert(
            "path".to_string(),
            toml::Value::String("/tmp".to_string()),
        );

        let task = Task::new("verify", "file_exists", params);

        let result = module.apply(&task, &test_node(), &exec).await.unwrap();
        assert!(result.success);
        assert!(result.message.contains("exists"));
    }

    #[tokio::test]
    async fn test_verify_file_not_exists() {
        let module = VerifyModule;
        let exec = Executor::local();
        let mut params = HashMap::new();
        params.insert(
            "path".to_string(),
            toml::Value::String("/nonexistent-sutra-test-path".to_string()),
        );

        let task = Task::new("verify", "file_exists", params);

        let result = module.apply(&task, &test_node(), &exec).await.unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_verify_port() {
        let module = VerifyModule;
        let exec = Executor::local();
        let mut params = HashMap::new();
        params.insert("port".to_string(), toml::Value::Integer(8070));

        let task = Task::new("verify", "port_listening", params);

        let plan = module.plan(&task, &test_node(), &exec).await.unwrap();
        assert!(!plan.changed);
        assert!(plan.description.contains("8070"));
    }

    #[test]
    fn test_verify_actions() {
        let module = VerifyModule;
        assert_eq!(module.name(), "verify");
        assert!(module.actions().contains(&"port_listening"));
        assert!(module.actions().contains(&"http_ok"));
    }
}
