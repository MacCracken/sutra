//! Local transport — execute commands on the current node.

use sutra_core::NodeInfo;

use crate::{ExecResult, Transport};

pub struct LocalTransport;

impl Transport for LocalTransport {
    async fn exec(&self, _node: &NodeInfo, command: &str) -> anyhow::Result<ExecResult> {
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .await?;

        Ok(ExecResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn copy_file(
        &self,
        _node: &NodeInfo,
        content: &[u8],
        remote_path: &str,
        mode: u32,
    ) -> anyhow::Result<()> {
        tokio::fs::write(remote_path, content).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(mode);
            tokio::fs::set_permissions(remote_path, perms).await?;
        }

        Ok(())
    }

    async fn read_file(&self, _node: &NodeInfo, remote_path: &str) -> anyhow::Result<Vec<u8>> {
        let content = tokio::fs::read(remote_path).await?;
        Ok(content)
    }

    async fn file_exists(&self, _node: &NodeInfo, remote_path: &str) -> anyhow::Result<bool> {
        Ok(tokio::fs::try_exists(remote_path).await.unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_node() -> NodeInfo {
        NodeInfo {
            id: "local".to_string(),
            host: "localhost".to_string(),
            role: "".to_string(),
            arch: "x86_64".to_string(),
            tags: vec![],
            transport: "local".to_string(),
            ssh_user: None,
            ssh_port: None,
            ssh_key: None,
        }
    }

    #[tokio::test]
    async fn test_local_exec() {
        let transport = LocalTransport;
        let result = transport.exec(&local_node(), "echo hello").await.unwrap();
        assert!(result.success());
        assert!(result.stdout.contains("hello"));
    }

    #[tokio::test]
    async fn test_local_file_exists() {
        let transport = LocalTransport;
        let exists = transport.file_exists(&local_node(), "/tmp").await.unwrap();
        assert!(exists);

        let missing = transport
            .file_exists(&local_node(), "/nonexistent-sutra-test-path")
            .await
            .unwrap();
        assert!(!missing);
    }

    #[tokio::test]
    async fn test_local_copy_and_read() {
        let transport = LocalTransport;
        let node = local_node();
        let path = format!("/tmp/sutra-test-{}", std::process::id());

        transport
            .copy_file(&node, b"sutra test content", &path, 0o644)
            .await
            .unwrap();

        let content = transport.read_file(&node, &path).await.unwrap();
        assert_eq!(content, b"sutra test content");

        // Cleanup
        let _ = tokio::fs::remove_file(&path).await;
    }
}
