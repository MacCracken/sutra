//! shell module — Escape hatch for arbitrary shell commands.

use sutra_core::{param_str, Executor, NodeInfo, SutraModule, Task, TaskPlan, TaskResult};

pub struct ShellModule;

impl SutraModule for ShellModule {
    fn name(&self) -> &str {
        "shell"
    }

    fn actions(&self) -> &[&str] {
        &["run", "script"]
    }

    async fn plan(
        &self,
        task: &Task,
        _node: &NodeInfo,
        _exec: &Executor,
    ) -> anyhow::Result<TaskPlan> {
        let description = match task.action.as_str() {
            "run" => {
                let cmd = param_str(task, "cmd", "");
                format!("run: {}", cmd)
            }
            "script" => {
                let src = param_str(task, "src", "");
                format!("run script: {}", src)
            }
            other => anyhow::bail!("unknown shell action: {}", other),
        };

        // Shell commands are always considered changed (not idempotent).
        // Use `creates` or `removes` param for idempotency hints.
        let creates = param_str(task, "creates", "");
        let removes = param_str(task, "removes", "");

        let changed = if !creates.is_empty() {
            !_exec.file_exists(creates).await?
        } else if !removes.is_empty() {
            _exec.file_exists(removes).await?
        } else {
            true
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
        let result = match task.action.as_str() {
            "run" => {
                let cmd = param_str(task, "cmd", "");
                if cmd.is_empty() {
                    anyhow::bail!("shell.run requires 'cmd' parameter");
                }
                exec.exec(cmd).await?
            }
            "script" => {
                let src = param_str(task, "src", "");
                if src.is_empty() {
                    anyhow::bail!("shell.script requires 'src' parameter");
                }
                // Read script content and execute it
                let content = exec.read_file_string(src).await?;
                exec.exec(&content).await?
            }
            other => anyhow::bail!("unknown shell action: {}", other),
        };

        Ok(TaskResult {
            module: self.name().to_string(),
            action: task.action.clone(),
            success: result.success(),
            changed: result.success(),
            message: if result.success() {
                let stdout = result.stdout.trim();
                if stdout.is_empty() {
                    "ok".to_string()
                } else {
                    stdout.to_string()
                }
            } else {
                format!("failed (exit {}): {}", result.exit_code, result.stderr.trim())
            },
        })
    }

    async fn check(
        &self,
        task: &Task,
        _node: &NodeInfo,
        exec: &Executor,
    ) -> anyhow::Result<bool> {
        // Shell commands use `creates` / `removes` for idempotency.
        let creates = param_str(task, "creates", "");
        let removes = param_str(task, "removes", "");

        if !creates.is_empty() {
            return exec.file_exists(creates).await;
        }
        if !removes.is_empty() {
            return exec.file_exists(removes).await.map(|v| !v);
        }

        // Without idempotency hints, always run.
        Ok(false)
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
    async fn test_shell_run() {
        let module = ShellModule;
        let exec = Executor::local();
        let mut params = HashMap::new();
        params.insert(
            "cmd".to_string(),
            toml::Value::String("echo hello".to_string()),
        );

        let task = Task {
            module: "shell".to_string(),
            action: "run".to_string(),
            params,
        };

        let result = module.apply(&task, &test_node(), &exec).await.unwrap();
        assert!(result.success);
        assert!(result.message.contains("hello"));
    }

    #[tokio::test]
    async fn test_shell_creates_idempotency() {
        let module = ShellModule;
        let exec = Executor::local();
        let path = format!("/tmp/sutra-shell-test-{}", std::process::id());

        let mut params = HashMap::new();
        params.insert(
            "cmd".to_string(),
            toml::Value::String(format!("touch {}", path)),
        );
        params.insert(
            "creates".to_string(),
            toml::Value::String(path.clone()),
        );

        let task = Task {
            module: "shell".to_string(),
            action: "run".to_string(),
            params,
        };

        // File doesn't exist — should plan to change
        let plan = module.plan(&task, &test_node(), &exec).await.unwrap();
        assert!(plan.changed);

        // Apply
        let result = module.apply(&task, &test_node(), &exec).await.unwrap();
        assert!(result.success);

        // Now check should pass — file exists
        assert!(module.check(&task, &test_node(), &exec).await.unwrap());

        // Plan again — should not change
        let plan = module.plan(&task, &test_node(), &exec).await.unwrap();
        assert!(!plan.changed);

        let _ = tokio::fs::remove_file(&path).await;
    }

    #[test]
    fn test_shell_actions() {
        let module = ShellModule;
        assert_eq!(module.name(), "shell");
        assert!(module.actions().contains(&"run"));
        assert!(module.actions().contains(&"script"));
    }
}
