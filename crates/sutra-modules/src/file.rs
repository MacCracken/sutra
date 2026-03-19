//! file module — File state management (copy, absent, permissions, line_in_file, template).

use sutra_core::{
    Executor, NodeInfo, SutraModule, Task, TaskPlan, TaskResult, esc, param_int, param_str,
};
use tera::Tera;

pub struct FileModule;

impl FileModule {
    fn path<'a>(&self, task: &'a Task) -> &'a str {
        param_str(task, "path", "unknown")
    }

    /// Extract template variables from task params (all params except module/action/path/src/mode).
    fn template_vars(task: &Task) -> tera::Context {
        let mut ctx = tera::Context::new();
        for (k, v) in &task.params {
            match k.as_str() {
                "path" | "src" | "mode" | "module" | "action" => continue,
                _ => {
                    let val = match v {
                        toml::Value::String(s) => tera::Value::String(s.clone()),
                        toml::Value::Integer(i) => tera::Value::Number((*i).into()),
                        toml::Value::Float(f) => {
                            tera::Value::Number(serde_json::Number::from_f64(*f).unwrap())
                        }
                        toml::Value::Boolean(b) => tera::Value::Bool(*b),
                        other => tera::Value::String(other.to_string()),
                    };
                    ctx.insert(k, &val);
                }
            }
        }
        ctx
    }

    /// Render a Tera template string with the given context.
    fn render_template(template: &str, ctx: &tera::Context) -> anyhow::Result<String> {
        let result = Tera::one_off(template, ctx, false)?;
        Ok(result)
    }
}

impl SutraModule for FileModule {
    fn name(&self) -> &str {
        "file"
    }

    fn actions(&self) -> &[&str] {
        &["template", "copy", "absent", "permissions", "line_in_file"]
    }

    async fn plan(
        &self,
        task: &Task,
        _node: &NodeInfo,
        exec: &Executor,
    ) -> anyhow::Result<TaskPlan> {
        let path = self.path(task);

        let (changed, description, diff) = match task.action.as_str() {
            "copy" => {
                let content = param_str(task, "content", "");
                let exists = exec.file_exists(path).await?;
                if exists {
                    let current = exec.read_file_string(path).await.unwrap_or_default();
                    if current == content {
                        (false, format!("{} already has desired content", path), None)
                    } else {
                        (
                            true,
                            format!("update {}", path),
                            Some("--- current\n+++ desired\n content differs".to_string()),
                        )
                    }
                } else {
                    (true, format!("create {}", path), None)
                }
            }
            "absent" => {
                let exists = exec.file_exists(path).await?;
                if exists {
                    (true, format!("remove {}", path), None)
                } else {
                    (false, format!("{} already absent", path), None)
                }
            }
            "permissions" => {
                let mode = param_int(task, "mode", 0o644);
                (
                    true,
                    format!("set permissions on {} to {:o}", path, mode),
                    None,
                )
            }
            "line_in_file" => {
                let line = param_str(task, "line", "");
                let exists = exec.file_exists(path).await?;
                if exists {
                    let current = exec.read_file_string(path).await.unwrap_or_default();
                    if current.lines().any(|l| l == line) {
                        (false, format!("line already present in {}", path), None)
                    } else {
                        (true, format!("add line to {}", path), None)
                    }
                } else {
                    (true, format!("create {} with line", path), None)
                }
            }
            "template" => {
                let src = param_str(task, "src", "");
                let exists = exec.file_exists(path).await?;
                if exists && exec.file_exists(src).await.unwrap_or(false) {
                    // Render template and compare with current file.
                    let tmpl = exec.read_file_string(src).await.unwrap_or_default();
                    let vars = Self::template_vars(task);
                    match Self::render_template(&tmpl, &vars) {
                        Ok(rendered) => {
                            let current = exec.read_file_string(path).await.unwrap_or_default();
                            if current == rendered {
                                (false, format!("{} already matches template", path), None)
                            } else {
                                (true, format!("render template {} -> {}", src, path), None)
                            }
                        }
                        Err(_) => (true, format!("render template {} -> {}", src, path), None),
                    }
                } else {
                    (true, format!("render template {} -> {}", src, path), None)
                }
            }
            other => anyhow::bail!("unknown file action: {}", other),
        };

        Ok(TaskPlan {
            module: self.name().to_string(),
            action: task.action.clone(),
            changed,
            description,
            diff,
        })
    }

    async fn apply(
        &self,
        task: &Task,
        _node: &NodeInfo,
        exec: &Executor,
    ) -> anyhow::Result<TaskResult> {
        let path = self.path(task);

        match task.action.as_str() {
            "copy" => {
                let content = param_str(task, "content", "");
                let mode = param_int(task, "mode", 0o644) as u32;
                exec.write_file(path, content.as_bytes(), mode).await?;
                Ok(TaskResult {
                    module: self.name().to_string(),
                    action: task.action.clone(),
                    success: true,
                    changed: true,
                    message: format!("wrote {}", path),
                })
            }
            "absent" => {
                let result = exec.exec(&format!("rm -f {}", esc(path))).await?;
                Ok(TaskResult {
                    module: self.name().to_string(),
                    action: task.action.clone(),
                    success: result.success(),
                    changed: true,
                    message: format!("removed {}", path),
                })
            }
            "permissions" => {
                let mode = param_int(task, "mode", 0o644);
                let owner = param_str(task, "owner", "");
                let group = param_str(task, "group", "");

                let result = exec
                    .exec(&format!("chmod {:o} {}", mode, esc(path)))
                    .await?;
                if !result.success() {
                    return Ok(TaskResult {
                        module: self.name().to_string(),
                        action: task.action.clone(),
                        success: false,
                        changed: false,
                        message: format!("chmod failed: {}", result.stderr.trim()),
                    });
                }

                if !owner.is_empty() || !group.is_empty() {
                    let chown_target = if !owner.is_empty() && !group.is_empty() {
                        format!("{}:{}", owner, group)
                    } else if !owner.is_empty() {
                        owner.to_string()
                    } else {
                        format!(":{}", group)
                    };
                    let result = exec
                        .exec(&format!("chown {} {}", esc(&chown_target), esc(path)))
                        .await?;
                    if !result.success() {
                        return Ok(TaskResult {
                            module: self.name().to_string(),
                            action: task.action.clone(),
                            success: false,
                            changed: false,
                            message: format!("chown failed: {}", result.stderr.trim()),
                        });
                    }
                }

                Ok(TaskResult {
                    module: self.name().to_string(),
                    action: task.action.clone(),
                    success: true,
                    changed: true,
                    message: format!("set permissions on {}", path),
                })
            }
            "line_in_file" => {
                let line = param_str(task, "line", "");
                let exists = exec.file_exists(path).await?;
                let current = if exists {
                    exec.read_file_string(path).await.unwrap_or_default()
                } else {
                    String::new()
                };

                if current.lines().any(|l| l == line) {
                    return Ok(TaskResult {
                        module: self.name().to_string(),
                        action: task.action.clone(),
                        success: true,
                        changed: false,
                        message: format!("line already present in {}", path),
                    });
                }

                let new_content = if current.is_empty() || current.ends_with('\n') {
                    format!("{}{}\n", current, line)
                } else {
                    format!("{}\n{}\n", current, line)
                };

                let mode = param_int(task, "mode", 0o644) as u32;
                exec.write_file(path, new_content.as_bytes(), mode).await?;

                Ok(TaskResult {
                    module: self.name().to_string(),
                    action: task.action.clone(),
                    success: true,
                    changed: true,
                    message: format!("added line to {}", path),
                })
            }
            "template" => {
                let src = param_str(task, "src", "");
                if src.is_empty() {
                    anyhow::bail!("template action requires 'src' parameter");
                }
                let tmpl_content = exec.read_file_string(src).await?;
                let vars = Self::template_vars(task);
                let rendered = Self::render_template(&tmpl_content, &vars)?;
                let mode = param_int(task, "mode", 0o644) as u32;
                exec.write_file(path, rendered.as_bytes(), mode).await?;
                Ok(TaskResult {
                    module: self.name().to_string(),
                    action: task.action.clone(),
                    success: true,
                    changed: true,
                    message: format!("rendered {} -> {}", src, path),
                })
            }
            other => anyhow::bail!("unknown file action: {}", other),
        }
    }

    async fn check(&self, task: &Task, _node: &NodeInfo, exec: &Executor) -> anyhow::Result<bool> {
        let path = self.path(task);

        match task.action.as_str() {
            "copy" => {
                let content = param_str(task, "content", "");
                let exists = exec.file_exists(path).await?;
                if !exists {
                    return Ok(false);
                }
                let current = exec.read_file_string(path).await?;
                Ok(current == content)
            }
            "absent" => {
                let exists = exec.file_exists(path).await?;
                Ok(!exists)
            }
            "line_in_file" => {
                let line = param_str(task, "line", "");
                let exists = exec.file_exists(path).await?;
                if !exists {
                    return Ok(false);
                }
                let current = exec.read_file_string(path).await?;
                Ok(current.lines().any(|l| l == line))
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
    async fn test_file_copy_and_check() {
        let module = FileModule;
        let exec = Executor::local();
        let path = format!("/tmp/sutra-file-test-{}", std::process::id());
        let mut params = HashMap::new();
        params.insert("path".to_string(), toml::Value::String(path.clone()));
        params.insert(
            "content".to_string(),
            toml::Value::String("hello world".to_string()),
        );

        let task = Task::new("file", "copy", params);

        // File doesn't exist yet — plan should say create
        let plan = module.plan(&task, &test_node(), &exec).await.unwrap();
        assert!(plan.changed);

        // Apply
        let result = module.apply(&task, &test_node(), &exec).await.unwrap();
        assert!(result.success);

        // Check should now pass
        assert!(module.check(&task, &test_node(), &exec).await.unwrap());

        // Plan again — should be no change
        let plan = module.plan(&task, &test_node(), &exec).await.unwrap();
        assert!(!plan.changed);

        // Cleanup
        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn test_file_absent() {
        let module = FileModule;
        let exec = Executor::local();
        let path = format!("/tmp/sutra-absent-test-{}", std::process::id());

        // Create a file first
        exec.write_file(&path, b"temp", 0o644).await.unwrap();

        let mut params = HashMap::new();
        params.insert("path".to_string(), toml::Value::String(path.clone()));

        let task = Task::new("file", "absent", params);

        let plan = module.plan(&task, &test_node(), &exec).await.unwrap();
        assert!(plan.changed);

        let result = module.apply(&task, &test_node(), &exec).await.unwrap();
        assert!(result.success);

        assert!(module.check(&task, &test_node(), &exec).await.unwrap());
    }

    #[tokio::test]
    async fn test_file_line_in_file() {
        let module = FileModule;
        let exec = Executor::local();
        let path = format!("/tmp/sutra-lif-test-{}", std::process::id());

        // Create initial file
        exec.write_file(&path, b"line1\nline2\n", 0o644)
            .await
            .unwrap();

        let mut params = HashMap::new();
        params.insert("path".to_string(), toml::Value::String(path.clone()));
        params.insert("line".to_string(), toml::Value::String("line3".to_string()));

        let task = Task::new("file", "line_in_file", params);

        // line3 not present — should change
        let plan = module.plan(&task, &test_node(), &exec).await.unwrap();
        assert!(plan.changed);

        let result = module.apply(&task, &test_node(), &exec).await.unwrap();
        assert!(result.success);
        assert!(result.changed);

        // Now it should be present
        assert!(module.check(&task, &test_node(), &exec).await.unwrap());

        // Cleanup
        let _ = tokio::fs::remove_file(&path).await;
    }

    #[test]
    fn test_file_actions() {
        let module = FileModule;
        assert_eq!(module.name(), "file");
        assert!(module.actions().contains(&"template"));
        assert!(module.actions().contains(&"absent"));
        assert!(module.actions().contains(&"line_in_file"));
    }
}
