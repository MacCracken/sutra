//! ark module — Package state management via AGNOS ark package manager.

use sutra_core::{NodeInfo, SutraModule, Task, TaskPlan, TaskResult};

pub struct ArkModule;

impl SutraModule for ArkModule {
    fn name(&self) -> &str {
        "ark"
    }

    fn actions(&self) -> &[&str] {
        &["install", "remove", "upgrade", "pin", "list"]
    }

    async fn plan(&self, task: &Task, _node: &NodeInfo) -> anyhow::Result<TaskPlan> {
        let package = task
            .params
            .get("package")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let version = task
            .params
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("latest");

        Ok(TaskPlan {
            module: self.name().to_string(),
            action: task.action.clone(),
            changed: true,
            description: format!("ark {} {} {}", task.action, package, version),
            diff: None,
        })
    }

    async fn apply(&self, task: &Task, node: &NodeInfo) -> anyhow::Result<TaskResult> {
        let plan = self.plan(task, node).await?;

        // TODO: Execute ark command via transport
        Ok(TaskResult {
            module: self.name().to_string(),
            action: task.action.clone(),
            success: true,
            changed: plan.changed,
            message: plan.description,
        })
    }

    async fn check(&self, task: &Task, _node: &NodeInfo) -> anyhow::Result<bool> {
        let _package = task
            .params
            .get("package")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // TODO: Query ark for installed package state
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_node() -> NodeInfo {
        NodeInfo {
            id: "test-node".to_string(),
            host: "localhost".to_string(),
            role: "edge".to_string(),
            arch: "x86_64".to_string(),
            tags: vec![],
            transport: "local".to_string(),
            ssh_user: None,
        }
    }

    #[tokio::test]
    async fn test_ark_plan_install() {
        let module = ArkModule;
        let mut params = HashMap::new();
        params.insert("package".to_string(), toml::Value::String("tarang".to_string()));
        params.insert("version".to_string(), toml::Value::String("2026.3.18".to_string()));

        let task = Task {
            module: "ark".to_string(),
            action: "install".to_string(),
            params,
        };

        let plan = module.plan(&task, &test_node()).await.unwrap();
        assert!(plan.changed);
        assert!(plan.description.contains("tarang"));
        assert!(plan.description.contains("2026.3.18"));
    }

    #[tokio::test]
    async fn test_ark_apply() {
        let module = ArkModule;
        let mut params = HashMap::new();
        params.insert("package".to_string(), toml::Value::String("tarang".to_string()));

        let task = Task {
            module: "ark".to_string(),
            action: "install".to_string(),
            params,
        };

        let result = module.apply(&task, &test_node()).await.unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_ark_actions() {
        let module = ArkModule;
        assert_eq!(module.name(), "ark");
        let actions = module.actions();
        assert!(actions.contains(&"install"));
        assert!(actions.contains(&"remove"));
        assert!(actions.contains(&"upgrade"));
    }
}
