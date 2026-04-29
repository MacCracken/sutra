//! nein module — Fleet firewall management via nein/nftables.
//!
//! Actions:
//! - `apply` — Apply a TOML firewall config via `nft -f -`
//! - `check` — Verify nftables rules are loaded (idempotency guard)
//! - `flush` — Flush all nftables rules
//!
//! # Example playbook
//!
//! ```toml
//! [[task]]
//! module = "nein"
//! action = "apply"
//!
//! [task.params]
//! config = """
//! [[tables]]
//! name = "filter"
//! family = "inet"
//!
//! [[tables.chains]]
//! name = "input"
//! chain_type = "filter"
//! hook = "input"
//! priority = 0
//! policy = "drop"
//!
//! [[tables.chains.rules]]
//! matches = [{ type = "ct_state", states = ["established", "related"] }]
//! verdict = "accept"
//!
//! [[tables.chains.rules]]
//! matches = [{ type = "protocol", value = "tcp" }, { type = "dport", port = 22 }]
//! verdict = "accept"
//! comment = "SSH"
//! """
//! ```

use sutra_core::{Executor, NodeInfo, SutraModule, Task, TaskPlan, TaskResult, param_str};

pub struct NeinModule;

impl SutraModule for NeinModule {
    fn name(&self) -> &str {
        "nein"
    }

    fn actions(&self) -> &[&str] {
        &["apply", "check", "flush"]
    }

    async fn plan(
        &self,
        task: &Task,
        _node: &NodeInfo,
        exec: &Executor,
    ) -> anyhow::Result<TaskPlan> {
        let description = match task.action.as_str() {
            "apply" => {
                let config_toml = param_str(task, "config", "");
                if config_toml.is_empty() {
                    anyhow::bail!("nein.apply requires 'config' parameter (TOML string)");
                }
                // Parse and validate the config to catch errors early
                let fw = nein::config::from_toml(config_toml)
                    .map_err(|e| anyhow::anyhow!("invalid nein config: {e}"))?;
                fw.validate()
                    .map_err(|e| anyhow::anyhow!("validation failed: {e}"))?;

                let table_count = fw.tables().len();
                let rule_count: usize = fw
                    .tables()
                    .iter()
                    .flat_map(|t| &t.chains)
                    .map(|c| c.rules.len())
                    .sum();
                format!("apply {table_count} tables, {rule_count} rules")
            }
            "check" => "check nftables ruleset is loaded".to_string(),
            "flush" => "flush all nftables rules".to_string(),
            other => anyhow::bail!("unknown nein action: {other}"),
        };

        // For apply: check if nft is available
        let changed = match task.action.as_str() {
            "apply" => {
                // Always apply — nftables is atomic, re-applying is safe
                true
            }
            "check" => false,
            "flush" => {
                // Check if there are any rules loaded
                let result = exec.exec("nft list ruleset 2>/dev/null | head -1").await?;
                !result.stdout.trim().is_empty()
            }
            _ => true,
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
        let (success, message) = match task.action.as_str() {
            "apply" => {
                let config_toml = param_str(task, "config", "");
                if config_toml.is_empty() {
                    anyhow::bail!("nein.apply requires 'config' parameter (TOML string)");
                }
                let fw = nein::config::from_toml(config_toml)
                    .map_err(|e| anyhow::anyhow!("invalid nein config: {e}"))?;
                fw.validate()
                    .map_err(|e| anyhow::anyhow!("validation failed: {e}"))?;
                let ruleset = fw.render();

                // Pipe ruleset to nft via stdin
                let escaped = shlex::try_quote(&ruleset)
                    .map_err(|e| anyhow::anyhow!("failed to escape ruleset: {e}"))?;
                let cmd = format!("echo {escaped} | nft -f -");
                let result = exec.exec(&cmd).await?;
                if result.success() {
                    (true, format!("applied {} bytes of nftables rules", ruleset.len()))
                } else {
                    (false, format!("nft failed: {}", result.stderr.trim()))
                }
            }
            "flush" => {
                let result = exec.exec("nft flush ruleset").await?;
                if result.success() {
                    (true, "flushed all nftables rules".to_string())
                } else {
                    (false, format!("flush failed: {}", result.stderr.trim()))
                }
            }
            "check" => {
                let result = exec.exec("nft list ruleset").await?;
                if result.success() {
                    let lines = result.stdout.lines().count();
                    (true, format!("{lines} lines in current ruleset"))
                } else {
                    (false, format!("nft check failed: {}", result.stderr.trim()))
                }
            }
            other => anyhow::bail!("unknown nein action: {other}"),
        };

        Ok(TaskResult {
            module: self.name().to_string(),
            action: task.action.clone(),
            success,
            changed: success,
            message,
        })
    }

    async fn check(&self, task: &Task, _node: &NodeInfo, exec: &Executor) -> anyhow::Result<bool> {
        match task.action.as_str() {
            "apply" => {
                // Check if nft has any rules loaded
                let result = exec.exec("nft list ruleset 2>/dev/null").await?;
                Ok(!result.stdout.trim().is_empty())
            }
            "flush" => {
                // Check if ruleset is already empty
                let result = exec.exec("nft list ruleset 2>/dev/null").await?;
                Ok(result.stdout.trim().is_empty())
            }
            "check" => Ok(true), // check is always "met"
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_metadata() {
        let m = NeinModule;
        assert_eq!(m.name(), "nein");
        assert!(m.actions().contains(&"apply"));
        assert!(m.actions().contains(&"check"));
        assert!(m.actions().contains(&"flush"));
    }
}
