//! argonaut module — Service state management via AGNOS argonaut init system.

use sutra_core::{NodeInfo, SutraModule, Task, TaskPlan, TaskResult};

pub struct ArgonautModule;

impl SutraModule for ArgonautModule {
    fn name(&self) -> &str {
        "argonaut"
    }

    fn actions(&self) -> &[&str] {
        &["enable", "disable", "start", "stop", "restart", "status"]
    }

    async fn plan(&self, task: &Task, _node: &NodeInfo) -> anyhow::Result<TaskPlan> {
        let service = task
            .params
            .get("service")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        Ok(TaskPlan {
            module: self.name().to_string(),
            action: task.action.clone(),
            changed: true,
            description: format!("argonaut {} {}", task.action, service),
            diff: None,
        })
    }

    async fn apply(&self, task: &Task, node: &NodeInfo) -> anyhow::Result<TaskResult> {
        let plan = self.plan(task, node).await?;

        // TODO: Execute argonaut command via transport
        Ok(TaskResult {
            module: self.name().to_string(),
            action: task.action.clone(),
            success: true,
            changed: plan.changed,
            message: plan.description,
        })
    }

    async fn check(&self, task: &Task, _node: &NodeInfo) -> anyhow::Result<bool> {
        let _service = task
            .params
            .get("service")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // TODO: Query argonaut for service state
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
            role: "edge".to_string(),
            arch: "x86_64".to_string(),
            tags: vec![],
            transport: "local".to_string(),
            ssh_user: None,
        }
    }

    #[tokio::test]
    async fn test_argonaut_plan_enable() {
        let module = ArgonautModule;
        let mut params = HashMap::new();
        params.insert("service".to_string(), toml::Value::String("tarang".to_string()));

        let task = Task {
            module: "argonaut".to_string(),
            action: "enable".to_string(),
            params,
        };

        let plan = module.plan(&task, &test_node()).await.unwrap();
        assert!(plan.description.contains("tarang"));
    }

    #[test]
    fn test_argonaut_actions() {
        let module = ArgonautModule;
        assert_eq!(module.name(), "argonaut");
        assert!(module.actions().contains(&"enable"));
        assert!(module.actions().contains(&"restart"));
    }
}
