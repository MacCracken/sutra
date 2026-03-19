//! argonaut module — Service state management via AGNOS argonaut init system.

use sutra_core::{param_str, Executor, NodeInfo, SutraModule, Task, TaskPlan, TaskResult};

pub struct ArgonautModule;

impl ArgonautModule {
    fn service<'a>(&self, task: &'a Task) -> &'a str {
        param_str(task, "service", "unknown")
    }

    /// Query whether a service is enabled (starts on boot).
    async fn is_enabled(&self, exec: &Executor, service: &str) -> anyhow::Result<bool> {
        let result = exec
            .exec(&format!("argonaut status {} 2>/dev/null", service))
            .await?;
        Ok(result.success() && result.stdout.contains("enabled"))
    }

    /// Query whether a service is currently running.
    async fn is_running(&self, exec: &Executor, service: &str) -> anyhow::Result<bool> {
        let result = exec
            .exec(&format!("argonaut status {} 2>/dev/null", service))
            .await?;
        Ok(result.success() && result.stdout.contains("running"))
    }
}

impl SutraModule for ArgonautModule {
    fn name(&self) -> &str {
        "argonaut"
    }

    fn actions(&self) -> &[&str] {
        &["enable", "disable", "start", "stop", "restart", "status"]
    }

    async fn plan(
        &self,
        task: &Task,
        _node: &NodeInfo,
        exec: &Executor,
    ) -> anyhow::Result<TaskPlan> {
        let service = self.service(task);

        let (changed, description) = match task.action.as_str() {
            "enable" => {
                let enabled = self.is_enabled(exec, service).await?;
                if enabled {
                    (false, format!("{} already enabled", service))
                } else {
                    (true, format!("enable {}", service))
                }
            }
            "disable" => {
                let enabled = self.is_enabled(exec, service).await?;
                if enabled {
                    (true, format!("disable {}", service))
                } else {
                    (false, format!("{} already disabled", service))
                }
            }
            "start" => {
                let running = self.is_running(exec, service).await?;
                if running {
                    (false, format!("{} already running", service))
                } else {
                    (true, format!("start {}", service))
                }
            }
            "stop" => {
                let running = self.is_running(exec, service).await?;
                if running {
                    (true, format!("stop {}", service))
                } else {
                    (false, format!("{} already stopped", service))
                }
            }
            "restart" => (true, format!("restart {}", service)),
            "status" => (false, format!("query status of {}", service)),
            other => anyhow::bail!("unknown argonaut action: {}", other),
        };

        Ok(TaskPlan {
            module: self.name().to_string(),
            action: task.action.clone(),
            changed,
            description,
            diff: None,
        })
    }

    async fn apply(
        &self,
        task: &Task,
        _node: &NodeInfo,
        exec: &Executor,
    ) -> anyhow::Result<TaskResult> {
        let service = self.service(task);

        let cmd = match task.action.as_str() {
            "enable" => format!("argonaut enable {}", service),
            "disable" => format!("argonaut disable {}", service),
            "start" => format!("argonaut start {}", service),
            "stop" => format!("argonaut stop {}", service),
            "restart" => format!("argonaut restart {}", service),
            "status" => format!("argonaut status {}", service),
            other => anyhow::bail!("unknown argonaut action: {}", other),
        };

        let result = exec.exec(&cmd).await?;

        if task.action == "status" {
            return Ok(TaskResult {
                module: self.name().to_string(),
                action: task.action.clone(),
                success: result.success(),
                changed: false,
                message: result.stdout,
            });
        }

        Ok(TaskResult {
            module: self.name().to_string(),
            action: task.action.clone(),
            success: result.success(),
            changed: result.success(),
            message: if result.success() {
                format!("argonaut {} {} — ok", task.action, service)
            } else {
                format!(
                    "argonaut {} {} — failed: {}",
                    task.action,
                    service,
                    result.stderr.trim()
                )
            },
        })
    }

    async fn check(
        &self,
        task: &Task,
        _node: &NodeInfo,
        exec: &Executor,
    ) -> anyhow::Result<bool> {
        let service = self.service(task);

        match task.action.as_str() {
            "enable" => self.is_enabled(exec, service).await,
            "disable" => self.is_enabled(exec, service).await.map(|v| !v),
            "start" => self.is_running(exec, service).await,
            "stop" => self.is_running(exec, service).await.map(|v| !v),
            _ => Ok(false),
        }
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
    async fn test_argonaut_plan_enable() {
        let module = ArgonautModule;
        let exec = Executor::local();
        let mut params = HashMap::new();
        params.insert(
            "service".to_string(),
            toml::Value::String("tarang".to_string()),
        );

        let task = Task::new("argonaut", "enable", params);

        let plan = module.plan(&task, &test_node(), &exec).await.unwrap();
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
