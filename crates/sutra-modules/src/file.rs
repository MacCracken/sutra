//! file module — File state management (template, copy, permissions, absent).

use sutra_core::{NodeInfo, SutraModule, Task, TaskPlan, TaskResult};

pub struct FileModule;

impl SutraModule for FileModule {
    fn name(&self) -> &str {
        "file"
    }

    fn actions(&self) -> &[&str] {
        &["template", "copy", "absent", "permissions", "line_in_file"]
    }

    async fn plan(&self, task: &Task, _node: &NodeInfo) -> anyhow::Result<TaskPlan> {
        let path = task
            .params
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        Ok(TaskPlan {
            module: self.name().to_string(),
            action: task.action.clone(),
            changed: true,
            description: format!("file {} {}", task.action, path),
            diff: None,
        })
    }

    async fn apply(&self, task: &Task, node: &NodeInfo) -> anyhow::Result<TaskResult> {
        let plan = self.plan(task, node).await?;

        // TODO: Execute file operation via transport
        Ok(TaskResult {
            module: self.name().to_string(),
            action: task.action.clone(),
            success: true,
            changed: plan.changed,
            message: plan.description,
        })
    }

    async fn check(&self, task: &Task, _node: &NodeInfo) -> anyhow::Result<bool> {
        let _path = task
            .params
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // TODO: Check file state
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
    async fn test_file_plan_template() {
        let module = FileModule;
        let mut params = HashMap::new();
        params.insert(
            "path".to_string(),
            toml::Value::String("/etc/agnos/config.toml".to_string()),
        );

        let task = Task {
            module: "file".to_string(),
            action: "template".to_string(),
            params,
        };

        let plan = module.plan(&task, &test_node()).await.unwrap();
        assert!(plan.description.contains("/etc/agnos/config.toml"));
    }

    #[test]
    fn test_file_actions() {
        let module = FileModule;
        assert_eq!(module.name(), "file");
        assert!(module.actions().contains(&"template"));
        assert!(module.actions().contains(&"absent"));
    }
}
