//! sutra-transport — Transport layer for executing tasks on local/remote nodes.
//!
//! For MVP, the Executor in sutra-core handles local execution directly.
//! This crate provides the Transport trait for v1 SSH/daimon transports.

use sutra_core::NodeInfo;

pub mod local;

/// Transport trait — how sutra reaches a node to execute tasks.
/// Used by SSH and daimon transports (v1). Local execution uses sutra_core::Executor.
#[allow(async_fn_in_trait)]
pub trait Transport: Send + Sync {
    /// Execute a shell command on the target node.
    async fn exec(&self, node: &NodeInfo, command: &str) -> anyhow::Result<ExecResult>;

    /// Copy a file to the target node.
    async fn copy_file(
        &self,
        node: &NodeInfo,
        content: &[u8],
        remote_path: &str,
        mode: u32,
    ) -> anyhow::Result<()>;

    /// Read a file from the target node.
    async fn read_file(&self, node: &NodeInfo, remote_path: &str) -> anyhow::Result<Vec<u8>>;

    /// Check if a file exists on the target node.
    async fn file_exists(&self, node: &NodeInfo, remote_path: &str) -> anyhow::Result<bool>;
}

#[derive(Debug, Clone)]
pub struct ExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl ExecResult {
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }
}

/// Select the appropriate transport for a node.
/// Returns LocalTransport for now — SSH and daimon transports will be added in v1.
pub fn transport_for(_node: &NodeInfo) -> local::LocalTransport {
    local::LocalTransport
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_result_success() {
        let result = ExecResult {
            exit_code: 0,
            stdout: "ok".to_string(),
            stderr: String::new(),
        };
        assert!(result.success());
    }

    #[test]
    fn test_exec_result_failure() {
        let result = ExecResult {
            exit_code: 1,
            stdout: String::new(),
            stderr: "error".to_string(),
        };
        assert!(!result.success());
    }

    #[test]
    fn test_transport_for_local() {
        let node = NodeInfo {
            id: "test".to_string(),
            host: "localhost".to_string(),
            role: "".to_string(),
            arch: "x86_64".to_string(),
            tags: vec![],
            transport: "local".to_string(),
            ssh_user: None,
            ssh_port: None,
            ssh_key: None,
        };
        let _transport = transport_for(&node);
    }
}
