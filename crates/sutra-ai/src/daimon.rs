//! Daimon + hoosh integration for sutra.

use serde::{Deserialize, Serialize};

pub struct DaimonConfig {
    pub endpoint: String,
    pub api_key: Option<String>,
}

impl Default for DaimonConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:8090".to_string(),
            api_key: None,
        }
    }
}

pub struct HooshConfig {
    pub endpoint: String,
    pub api_key: Option<String>,
}

impl Default for HooshConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:8088".to_string(),
            api_key: None,
        }
    }
}

pub struct DaimonClient {
    config: DaimonConfig,
    client: reqwest::Client,
}

impl DaimonClient {
    pub fn new(config: DaimonConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");
        Self { config, client }
    }

    /// Register sutra as an agent with daimon.
    pub async fn register_agent(&self) -> anyhow::Result<()> {
        let body = serde_json::json!({
            "name": "sutra",
            "agent_type": "orchestrator",
            "capabilities": ["infrastructure", "fleet", "deployment"],
        });

        let mut req = self
            .client
            .post(format!("{}/v1/agents/register", self.config.endpoint))
            .json(&body);

        if let Some(key) = &self.config.api_key {
            req = req.bearer_auth(key);
        }

        req.send().await?;
        Ok(())
    }

    /// Fetch fleet inventory from daimon edge API.
    pub async fn fetch_fleet_nodes(&self) -> anyhow::Result<Vec<FleetNode>> {
        let mut req = self
            .client
            .get(format!("{}/v1/edge/nodes", self.config.endpoint));

        if let Some(key) = &self.config.api_key {
            req = req.bearer_auth(key);
        }

        let resp = req.send().await?;
        let nodes: Vec<FleetNode> = resp.json().await?;
        Ok(nodes)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetNode {
    pub id: String,
    pub host: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub arch: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

pub struct HooshClient {
    config: HooshConfig,
    client: reqwest::Client,
}

impl HooshClient {
    pub fn new(config: HooshConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("failed to build HTTP client");
        Self { config, client }
    }

    /// Translate natural language to a TOML playbook via hoosh LLM.
    pub async fn nl_to_toml(&self, prompt: &str) -> anyhow::Result<String> {
        let body = serde_json::json!({
            "model": "default",
            "messages": [
                {
                    "role": "system",
                    "content": "You are a TOML playbook generator for Sutra, the AGNOS infrastructure orchestrator. Generate valid TOML playbooks with [playbook], [[target]], and [[task]] sections. Available modules: ark (install/remove/upgrade), argonaut (enable/disable/start/stop/restart), file (template/copy/absent/permissions), verify (port_listening/file_exists/service_running/http_ok), daimon (register/report), edge (target/heartbeat/update), shell (run), user (present/absent), nftables (rule/policy), sysctl (set). Output ONLY the TOML, no explanation."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
        });

        let mut req = self
            .client
            .post(format!("{}/v1/chat/completions", self.config.endpoint))
            .json(&body);

        if let Some(key) = &self.config.api_key {
            req = req.bearer_auth(key);
        }

        let resp = req.send().await?;
        let json: serde_json::Value = resp.json().await?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daimon_config_default() {
        let config = DaimonConfig::default();
        assert_eq!(config.endpoint, "http://localhost:8090");
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_hoosh_config_default() {
        let config = HooshConfig::default();
        assert_eq!(config.endpoint, "http://localhost:8088");
    }

    #[test]
    fn test_fleet_node_deserialize() {
        let json = r#"{"id": "rpi-1", "host": "192.168.1.50", "role": "edge", "arch": "aarch64", "tags": ["iot"]}"#;
        let node: FleetNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.id, "rpi-1");
        assert_eq!(node.arch, "aarch64");
    }
}
