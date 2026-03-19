//! user module — User and group management.

use sutra_core::{param_bool, param_str, Executor, NodeInfo, SutraModule, Task, TaskPlan, TaskResult};

pub struct UserModule;

impl UserModule {
    async fn user_exists(&self, exec: &Executor, username: &str) -> anyhow::Result<bool> {
        let result = exec
            .exec(&format!("id {} 2>/dev/null", username))
            .await?;
        Ok(result.success())
    }

    async fn group_exists(&self, exec: &Executor, group: &str) -> anyhow::Result<bool> {
        let result = exec
            .exec(&format!("getent group {} 2>/dev/null", group))
            .await?;
        Ok(result.success())
    }
}

impl SutraModule for UserModule {
    fn name(&self) -> &str {
        "user"
    }

    fn actions(&self) -> &[&str] {
        &[
            "present",
            "absent",
            "group_present",
            "group_absent",
        ]
    }

    async fn plan(
        &self,
        task: &Task,
        _node: &NodeInfo,
        exec: &Executor,
    ) -> anyhow::Result<TaskPlan> {
        let (changed, description) = match task.action.as_str() {
            "present" => {
                let username = param_str(task, "username", "");
                let exists = self.user_exists(exec, username).await?;
                if exists {
                    (false, format!("user {} already exists", username))
                } else {
                    (true, format!("create user {}", username))
                }
            }
            "absent" => {
                let username = param_str(task, "username", "");
                let exists = self.user_exists(exec, username).await?;
                if exists {
                    (true, format!("remove user {}", username))
                } else {
                    (false, format!("user {} already absent", username))
                }
            }
            "group_present" => {
                let group = param_str(task, "group", "");
                let exists = self.group_exists(exec, group).await?;
                if exists {
                    (false, format!("group {} already exists", group))
                } else {
                    (true, format!("create group {}", group))
                }
            }
            "group_absent" => {
                let group = param_str(task, "group", "");
                let exists = self.group_exists(exec, group).await?;
                if exists {
                    (true, format!("remove group {}", group))
                } else {
                    (false, format!("group {} already absent", group))
                }
            }
            other => anyhow::bail!("unknown user action: {}", other),
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
        match task.action.as_str() {
            "present" => {
                let username = param_str(task, "username", "");
                if username.is_empty() {
                    anyhow::bail!("user.present requires 'username' parameter");
                }

                if self.user_exists(exec, username).await? {
                    return Ok(TaskResult {
                        module: self.name().to_string(),
                        action: task.action.clone(),
                        success: true,
                        changed: false,
                        message: format!("user {} already exists", username),
                    });
                }

                let shell = param_str(task, "shell", "/bin/sh");
                let home = param_str(task, "home", "");
                let group = param_str(task, "group", "");
                let system = param_bool(task, "system", false);

                let mut cmd = String::from("useradd");
                if system {
                    cmd.push_str(" --system");
                }
                if !home.is_empty() {
                    cmd.push_str(&format!(" --home-dir {}", home));
                    cmd.push_str(" --create-home");
                }
                if !group.is_empty() {
                    cmd.push_str(&format!(" --gid {}", group));
                }
                cmd.push_str(&format!(" --shell {}", shell));
                cmd.push_str(&format!(" {}", username));

                let result = exec.exec(&cmd).await?;
                Ok(TaskResult {
                    module: self.name().to_string(),
                    action: task.action.clone(),
                    success: result.success(),
                    changed: result.success(),
                    message: if result.success() {
                        format!("created user {}", username)
                    } else {
                        format!("failed to create user {}: {}", username, result.stderr.trim())
                    },
                })
            }
            "absent" => {
                let username = param_str(task, "username", "");
                if username.is_empty() {
                    anyhow::bail!("user.absent requires 'username' parameter");
                }

                if !self.user_exists(exec, username).await? {
                    return Ok(TaskResult {
                        module: self.name().to_string(),
                        action: task.action.clone(),
                        success: true,
                        changed: false,
                        message: format!("user {} already absent", username),
                    });
                }

                let remove_home = param_bool(task, "remove_home", false);
                let cmd = if remove_home {
                    format!("userdel --remove {}", username)
                } else {
                    format!("userdel {}", username)
                };

                let result = exec.exec(&cmd).await?;
                Ok(TaskResult {
                    module: self.name().to_string(),
                    action: task.action.clone(),
                    success: result.success(),
                    changed: result.success(),
                    message: if result.success() {
                        format!("removed user {}", username)
                    } else {
                        format!("failed to remove user {}: {}", username, result.stderr.trim())
                    },
                })
            }
            "group_present" => {
                let group = param_str(task, "group", "");
                if group.is_empty() {
                    anyhow::bail!("user.group_present requires 'group' parameter");
                }

                if self.group_exists(exec, group).await? {
                    return Ok(TaskResult {
                        module: self.name().to_string(),
                        action: task.action.clone(),
                        success: true,
                        changed: false,
                        message: format!("group {} already exists", group),
                    });
                }

                let system = param_bool(task, "system", false);
                let cmd = if system {
                    format!("groupadd --system {}", group)
                } else {
                    format!("groupadd {}", group)
                };

                let result = exec.exec(&cmd).await?;
                Ok(TaskResult {
                    module: self.name().to_string(),
                    action: task.action.clone(),
                    success: result.success(),
                    changed: result.success(),
                    message: if result.success() {
                        format!("created group {}", group)
                    } else {
                        format!("failed to create group {}: {}", group, result.stderr.trim())
                    },
                })
            }
            "group_absent" => {
                let group = param_str(task, "group", "");
                if group.is_empty() {
                    anyhow::bail!("user.group_absent requires 'group' parameter");
                }

                if !self.group_exists(exec, group).await? {
                    return Ok(TaskResult {
                        module: self.name().to_string(),
                        action: task.action.clone(),
                        success: true,
                        changed: false,
                        message: format!("group {} already absent", group),
                    });
                }

                let result = exec.exec(&format!("groupdel {}", group)).await?;
                Ok(TaskResult {
                    module: self.name().to_string(),
                    action: task.action.clone(),
                    success: result.success(),
                    changed: result.success(),
                    message: if result.success() {
                        format!("removed group {}", group)
                    } else {
                        format!("failed to remove group {}: {}", group, result.stderr.trim())
                    },
                })
            }
            other => anyhow::bail!("unknown user action: {}", other),
        }
    }

    async fn check(
        &self,
        task: &Task,
        _node: &NodeInfo,
        exec: &Executor,
    ) -> anyhow::Result<bool> {
        match task.action.as_str() {
            "present" => {
                let username = param_str(task, "username", "");
                self.user_exists(exec, username).await
            }
            "absent" => {
                let username = param_str(task, "username", "");
                self.user_exists(exec, username).await.map(|v| !v)
            }
            "group_present" => {
                let group = param_str(task, "group", "");
                self.group_exists(exec, group).await
            }
            "group_absent" => {
                let group = param_str(task, "group", "");
                self.group_exists(exec, group).await.map(|v| !v)
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
    async fn test_user_check_root_exists() {
        let module = UserModule;
        let exec = Executor::local();
        let mut params = HashMap::new();
        params.insert(
            "username".to_string(),
            toml::Value::String("root".to_string()),
        );

        let task = Task {
            module: "user".to_string(),
            action: "present".to_string(),
            params,
        };

        // root should exist on any Linux system
        assert!(module.check(&task, &test_node(), &exec).await.unwrap());

        // Plan should say no change needed
        let plan = module.plan(&task, &test_node(), &exec).await.unwrap();
        assert!(!plan.changed);
    }

    #[tokio::test]
    async fn test_user_check_nonexistent() {
        let module = UserModule;
        let exec = Executor::local();
        let mut params = HashMap::new();
        params.insert(
            "username".to_string(),
            toml::Value::String("sutra_nonexistent_test_user_12345".to_string()),
        );

        let task = Task {
            module: "user".to_string(),
            action: "present".to_string(),
            params,
        };

        assert!(!module.check(&task, &test_node(), &exec).await.unwrap());
    }

    #[tokio::test]
    async fn test_group_check_root() {
        let module = UserModule;
        let exec = Executor::local();
        let mut params = HashMap::new();
        params.insert(
            "group".to_string(),
            toml::Value::String("root".to_string()),
        );

        let task = Task {
            module: "user".to_string(),
            action: "group_present".to_string(),
            params,
        };

        assert!(module.check(&task, &test_node(), &exec).await.unwrap());
    }

    #[test]
    fn test_user_actions() {
        let module = UserModule;
        assert_eq!(module.name(), "user");
        assert!(module.actions().contains(&"present"));
        assert!(module.actions().contains(&"absent"));
        assert!(module.actions().contains(&"group_present"));
    }
}
