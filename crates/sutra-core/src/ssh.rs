//! SSH transport — connect and execute commands on remote nodes via russh.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use russh::ChannelMsg;

use crate::ExecResult;

/// SSH client handler — minimal implementation for infrastructure tooling.
pub struct SshHandler;

#[async_trait]
impl russh::client::Handler for SshHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh_keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        // TODO: Check against known_hosts file.
        // For v1, accept all keys (infrastructure nodes are in our inventory).
        Ok(true)
    }
}

/// Connect to an SSH server and authenticate.
pub async fn connect(
    host: &str,
    port: u16,
    user: &str,
    key_path: Option<&Path>,
) -> anyhow::Result<russh::client::Handle<SshHandler>> {
    let config = Arc::new(russh::client::Config::default());
    let handler = SshHandler;

    let mut handle = russh::client::connect(config, (host, port), handler).await?;

    // Try key file authentication.
    let key_paths: Vec<std::path::PathBuf> = if let Some(p) = key_path {
        vec![p.to_path_buf()]
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        vec![
            format!("{}/.ssh/id_ed25519", home).into(),
            format!("{}/.ssh/id_rsa", home).into(),
        ]
    };

    let mut authenticated = false;
    for path in &key_paths {
        if !path.exists() {
            continue;
        }
        match russh_keys::load_secret_key(path, None) {
            Ok(key) => match handle.authenticate_publickey(user, Arc::new(key)).await {
                Ok(true) => {
                    authenticated = true;
                    break;
                }
                Ok(false) => continue,
                Err(e) => {
                    tracing::debug!("key auth failed with {}: {}", path.display(), e);
                    continue;
                }
            },
            Err(e) => {
                tracing::debug!("failed to load key {}: {}", path.display(), e);
                continue;
            }
        }
    }

    if !authenticated {
        anyhow::bail!(
            "SSH authentication failed for {}@{}:{} — no valid key found",
            user,
            host,
            port
        );
    }

    Ok(handle)
}

/// Execute a command on an SSH session and collect output.
pub async fn exec(
    handle: &russh::client::Handle<SshHandler>,
    command: &str,
) -> anyhow::Result<ExecResult> {
    let mut channel = handle.channel_open_session().await?;
    channel.exec(true, command).await?;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut exit_code: i32 = -1;

    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { ref data } => {
                stdout.extend_from_slice(data);
            }
            ChannelMsg::ExtendedData { ref data, ext: 1 } => {
                stderr.extend_from_slice(data);
            }
            ChannelMsg::ExitStatus { exit_status } => {
                exit_code = exit_status as i32;
            }
            _ => {}
        }
    }

    Ok(ExecResult {
        exit_code,
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
    })
}
