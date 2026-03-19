//! ark module — Package state management via AGNOS ark package manager.

use sutra_core::{esc, param_str, Executor, NodeInfo, SutraModule, Task, TaskPlan, TaskResult};

pub struct ArkModule;

impl ArkModule {
    fn package<'a>(&self, task: &'a Task) -> &'a str {
        param_str(task, "package", "unknown")
    }

    fn version<'a>(&self, task: &'a Task) -> &'a str {
        param_str(task, "version", "latest")
    }

    /// Query whether a package is installed, and optionally its version.
    async fn query_installed(
        &self,
        exec: &Executor,
        package: &str,
    ) -> anyhow::Result<Option<String>> {
        // `ark list --installed` and grep for the package.
        // If ark is not available, fall back to checking if the command exists.
        let result = exec
            .exec(&format!("ark query {} 2>/dev/null", esc(package)))
            .await?;
        if result.success() && !result.stdout.trim().is_empty() {
            // Output format: "package version"
            let version = result
                .stdout
                .split_whitespace()
                .nth(1)
                .unwrap_or("unknown")
                .to_string();
            Ok(Some(version))
        } else {
            Ok(None)
        }
    }
}

impl SutraModule for ArkModule {
    fn name(&self) -> &str {
        "ark"
    }

    fn actions(&self) -> &[&str] {
        &["install", "remove", "upgrade", "pin", "list"]
    }

    async fn plan(
        &self,
        task: &Task,
        _node: &NodeInfo,
        exec: &Executor,
    ) -> anyhow::Result<TaskPlan> {
        let package = self.package(task);
        let version = self.version(task);

        let (changed, description) = match task.action.as_str() {
            "install" => {
                let installed = self.query_installed(exec, package).await?;
                match installed {
                    Some(v) if version == "latest" || v == version => (
                        false,
                        format!("{} {} already installed", package, v),
                    ),
                    Some(v) => (
                        true,
                        format!("upgrade {} from {} to {}", package, v, version),
                    ),
                    None => (true, format!("install {} {}", package, version)),
                }
            }
            "remove" => {
                let installed = self.query_installed(exec, package).await?;
                match installed {
                    Some(_) => (true, format!("remove {}", package)),
                    None => (false, format!("{} not installed", package)),
                }
            }
            "upgrade" => (true, format!("upgrade {} to {}", package, version)),
            "pin" => (true, format!("pin {} at {}", package, version)),
            "list" => (false, "list installed packages".to_string()),
            other => anyhow::bail!("unknown ark action: {}", other),
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
        let package = self.package(task);
        let version = self.version(task);

        let pkg = esc(package);
        let ver = esc(version);
        let cmd = match task.action.as_str() {
            "install" if version != "latest" => format!("ark install {}={}", pkg, ver),
            "install" => format!("ark install {}", pkg),
            "remove" => format!("ark remove -y {}", pkg),
            "upgrade" if version != "latest" => format!("ark upgrade {}={}", pkg, ver),
            "upgrade" => format!("ark upgrade {}", pkg),
            "pin" => format!("ark pin {}={}", pkg, ver),
            "list" => "ark list --installed".to_string(),
            other => anyhow::bail!("unknown ark action: {}", other),
        };

        let result = exec.exec(&cmd).await?;

        if task.action == "list" {
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
                format!("ark {} {} — ok", task.action, package)
            } else {
                format!(
                    "ark {} {} — failed: {}",
                    task.action,
                    package,
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
        let package = self.package(task);
        let version = self.version(task);

        match task.action.as_str() {
            "install" => {
                let installed = self.query_installed(exec, package).await?;
                match installed {
                    Some(v) if version == "latest" => Ok(true),
                    Some(v) if v == version => Ok(true),
                    _ => Ok(false),
                }
            }
            "remove" => {
                let installed = self.query_installed(exec, package).await?;
                Ok(installed.is_none())
            }
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
    async fn test_ark_plan_install() {
        let module = ArkModule;
        let exec = Executor::local();
        let mut params = HashMap::new();
        params.insert(
            "package".to_string(),
            toml::Value::String("tarang".to_string()),
        );
        params.insert(
            "version".to_string(),
            toml::Value::String("2026.3.18".to_string()),
        );

        let task = Task::new("ark", "install", params);

        let plan = module.plan(&task, &test_node(), &exec).await.unwrap();
        // On a machine without ark, package won't be found, so changed=true
        assert!(plan.description.contains("tarang"));
    }

    #[tokio::test]
    async fn test_ark_plan_list() {
        let module = ArkModule;
        let exec = Executor::local();

        let task = Task::new("ark", "list", HashMap::new());

        let plan = module.plan(&task, &test_node(), &exec).await.unwrap();
        assert!(!plan.changed);
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
