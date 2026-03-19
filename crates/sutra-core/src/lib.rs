//! sutra-core — Playbook parser, task graph, execution engine, and module trait.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

mod ssh;

// ── Executor ────────────────────────────────────────────────────────────────

/// Result of executing a shell command on a node.
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

/// Concrete executor — how sutra reaches a node.
/// Uses enum dispatch to avoid async-dyn issues.
pub struct Executor {
    kind: ExecutorKind,
}

enum ExecutorKind {
    Local,
    Ssh {
        session: Arc<Mutex<Option<russh::client::Handle<ssh::SshHandler>>>>,
        host: String,
        port: u16,
        user: String,
        key_path: Option<PathBuf>,
    },
}

impl Executor {
    /// Create a local executor (runs commands on this machine).
    pub fn local() -> Self {
        Self {
            kind: ExecutorKind::Local,
        }
    }

    /// Create an SSH executor.
    pub fn ssh(host: &str, port: u16, user: &str, key_path: Option<PathBuf>) -> Self {
        Self {
            kind: ExecutorKind::Ssh {
                session: Arc::new(Mutex::new(None)),
                host: host.to_string(),
                port,
                user: user.to_string(),
                key_path,
            },
        }
    }

    /// Create the appropriate executor for a node based on its transport field.
    pub fn for_node(node: &NodeInfo) -> Self {
        match node.transport.as_str() {
            "local" => Self::local(),
            "ssh" => Self::ssh(
                &node.host,
                node.ssh_port.unwrap_or(22),
                node.ssh_user.as_deref().unwrap_or("root"),
                node.ssh_key.as_ref().map(PathBuf::from),
            ),
            other => {
                tracing::warn!("unsupported transport '{}', falling back to local", other);
                Self::local()
            }
        }
    }

    /// Ensure SSH session is connected and return a locked guard.
    /// Caller must hold the guard for the duration of the operation.
    async fn ensure_ssh_connected(&self) -> anyhow::Result<()> {
        let ExecutorKind::Ssh {
            session,
            host,
            port,
            user,
            key_path,
        } = &self.kind
        else {
            anyhow::bail!("ensure_ssh_connected called on non-SSH executor")
        };

        let mut guard = session.lock().await;
        if guard.is_none() {
            let handle = ssh::connect(host, *port, user, key_path.as_deref()).await?;
            *guard = Some(handle);
        }
        Ok(())
    }

    /// Execute a command via SSH (internal helper).
    async fn ssh_exec(&self, command: &str) -> anyhow::Result<ExecResult> {
        let ExecutorKind::Ssh { session, .. } = &self.kind else {
            anyhow::bail!("ssh_exec called on non-SSH executor")
        };

        self.ensure_ssh_connected().await?;
        let guard = session.lock().await;
        let handle = guard.as_ref().unwrap();
        ssh::exec(handle, command).await
    }

    /// Execute a shell command.
    pub async fn exec(&self, command: &str) -> anyhow::Result<ExecResult> {
        match &self.kind {
            ExecutorKind::Local => {
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
            ExecutorKind::Ssh { .. } => self.ssh_exec(command).await,
        }
    }

    /// Check if a file exists.
    pub async fn file_exists(&self, path: &str) -> anyhow::Result<bool> {
        match &self.kind {
            ExecutorKind::Local => Ok(tokio::fs::try_exists(path).await.unwrap_or(false)),
            ExecutorKind::Ssh { .. } => {
                let result = self
                    .exec(&format!("test -e {} && echo yes || echo no", esc(path)))
                    .await?;
                Ok(result.stdout.trim() == "yes")
            }
        }
    }

    /// Read a file's contents.
    pub async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        match &self.kind {
            ExecutorKind::Local => Ok(tokio::fs::read(path).await?),
            ExecutorKind::Ssh { .. } => {
                let result = self.exec(&format!("base64 {}", esc(path))).await?;
                if !result.success() {
                    anyhow::bail!("failed to read {}: {}", path, result.stderr.trim());
                }
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(result.stdout.trim().replace('\n', ""))?;
                Ok(bytes)
            }
        }
    }

    /// Write content to a file with the given mode.
    pub async fn write_file(&self, path: &str, content: &[u8], mode: u32) -> anyhow::Result<()> {
        match &self.kind {
            ExecutorKind::Local => {
                tokio::fs::write(path, content).await?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = std::fs::Permissions::from_mode(mode);
                    tokio::fs::set_permissions(path, perms).await?;
                }
                Ok(())
            }
            ExecutorKind::Ssh { .. } => {
                use base64::Engine;
                let encoded = base64::engine::general_purpose::STANDARD.encode(content);
                let parent = Path::new(path)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let epath = esc(path);
                let cmd = if parent.is_empty() {
                    format!(
                        "printf '%s' '{}' | base64 -d > {} && chmod {:o} {}",
                        encoded, epath, mode, epath
                    )
                } else {
                    format!(
                        "mkdir -p {} && printf '%s' '{}' | base64 -d > {} && chmod {:o} {}",
                        esc(&parent),
                        encoded,
                        epath,
                        mode,
                        epath
                    )
                };
                let result = self.exec(&cmd).await?;
                if !result.success() {
                    anyhow::bail!("failed to write {}: {}", path, result.stderr.trim());
                }
                Ok(())
            }
        }
    }

    /// Read a file as a UTF-8 string.
    pub async fn read_file_string(&self, path: &str) -> anyhow::Result<String> {
        let bytes = self.read_file(path).await?;
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }
}

// ── Playbook types ──────────────────────────────────────────────────────────

/// A parsed playbook — the unit of orchestration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playbook {
    pub playbook: PlaybookMeta,
    #[serde(default)]
    pub target: Vec<Target>,
    #[serde(default)]
    pub vars: HashMap<String, toml::Value>,
    pub task: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Default error handling for all tasks (can be overridden per-task).
    #[serde(default)]
    pub on_error: OnError,
}

/// Targeting criteria — which nodes to run against.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub all: Option<bool>,
}

/// Error handling strategy for a task or playbook.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OnError {
    /// Abort the entire run (default).
    #[default]
    Fail,
    /// Log the error and continue with the next task.
    Continue,
    /// Ignore the error entirely (task appears successful).
    Ignore,
}

/// A single task within a playbook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub module: String,
    pub action: String,
    /// Error handling for this task (overrides playbook-level on_error).
    #[serde(default)]
    pub on_error: Option<OnError>,
    /// Task name/label for depends_on references.
    #[serde(default)]
    pub name: Option<String>,
    /// Run this task only after named tasks complete.
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(flatten)]
    pub params: HashMap<String, toml::Value>,
}

impl Task {
    /// Create a task with just module, action, and params (convenience for tests).
    pub fn new(module: &str, action: &str, params: HashMap<String, toml::Value>) -> Self {
        Self {
            module: module.to_string(),
            action: action.to_string(),
            on_error: None,
            name: None,
            depends_on: Vec::new(),
            params,
        }
    }
}

/// Information about a target node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: String,
    pub host: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub arch: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_transport")]
    pub transport: String,
    #[serde(default)]
    pub ssh_user: Option<String>,
    #[serde(default)]
    pub ssh_port: Option<u16>,
    #[serde(default)]
    pub ssh_key: Option<String>,
}

fn default_transport() -> String {
    "local".to_string()
}

impl NodeInfo {
    /// Create a NodeInfo for localhost.
    pub fn localhost() -> Self {
        Self {
            id: "local".to_string(),
            host: "localhost".to_string(),
            role: String::new(),
            arch: std::env::consts::ARCH.to_string(),
            tags: vec![],
            transport: "local".to_string(),
            ssh_user: None,
            ssh_port: None,
            ssh_key: None,
        }
    }
}

// ── Variables & facts ───────────────────────────────────────────────────────

/// Variable context for template expansion in task params.
#[derive(Debug, Clone, Default)]
pub struct VarContext {
    /// Playbook-level vars.
    pub vars: HashMap<String, String>,
    /// Runtime node facts.
    pub facts: HashMap<String, String>,
}

impl VarContext {
    /// Build a VarContext from playbook vars and CLI overrides.
    pub fn new(
        playbook_vars: &HashMap<String, toml::Value>,
        cli_vars: &[(String, String)],
    ) -> Self {
        let mut vars = HashMap::new();
        for (k, v) in playbook_vars {
            vars.insert(k.clone(), toml_value_to_string(v));
        }
        // CLI vars override playbook vars.
        for (k, v) in cli_vars {
            vars.insert(k.clone(), v.clone());
        }
        Self {
            vars,
            facts: HashMap::new(),
        }
    }

    /// Expand `{{ var }}` and `{{ fact.key }}` placeholders in a string.
    pub fn expand(&self, input: &str) -> String {
        let mut result = input.to_string();
        // Simple {{ key }} expansion — vars first, then facts.
        for (k, v) in &self.vars {
            result = result.replace(&format!("{{{{ {} }}}}", k), v);
        }
        for (k, v) in &self.facts {
            result = result.replace(&format!("{{{{ fact.{} }}}}", k), v);
        }
        result
    }

    /// Expand all string params in a task, returning a new task.
    pub fn expand_task(&self, task: &Task) -> Task {
        let mut params = HashMap::new();
        for (k, v) in &task.params {
            params.insert(k.clone(), self.expand_value(v));
        }
        Task {
            module: task.module.clone(),
            action: task.action.clone(),
            on_error: task.on_error.clone(),
            name: task.name.clone(),
            depends_on: task.depends_on.clone(),
            params,
        }
    }

    fn expand_value(&self, val: &toml::Value) -> toml::Value {
        match val {
            toml::Value::String(s) => toml::Value::String(self.expand(s)),
            toml::Value::Array(arr) => {
                toml::Value::Array(arr.iter().map(|v| self.expand_value(v)).collect())
            }
            toml::Value::Table(table) => {
                let mut new_table = toml::map::Map::new();
                for (k, v) in table {
                    new_table.insert(k.clone(), self.expand_value(v));
                }
                toml::Value::Table(new_table)
            }
            other => other.clone(),
        }
    }
}

fn toml_value_to_string(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// Gather facts about a node by running commands via the executor.
pub async fn gather_facts(exec: &Executor) -> HashMap<String, String> {
    let mut facts = HashMap::new();

    if let Ok(r) = exec.exec("uname -s").await {
        facts.insert("os".into(), r.stdout.trim().into());
    }
    if let Ok(r) = exec.exec("uname -m").await {
        facts.insert("arch".into(), r.stdout.trim().into());
    }
    if let Ok(r) = exec.exec("hostname").await {
        facts.insert("hostname".into(), r.stdout.trim().into());
    }

    // Distro detection from /etc/os-release.
    if let Ok(r) = exec.exec("cat /etc/os-release 2>/dev/null").await
        && r.success()
    {
        for line in r.stdout.lines() {
            if let Some(id) = line.strip_prefix("ID=") {
                facts.insert("distro".into(), id.trim_matches('"').into());
            }
            if let Some(ver) = line.strip_prefix("VERSION_ID=") {
                facts.insert("distro_version".into(), ver.trim_matches('"').into());
            }
            if let Some(name) = line.strip_prefix("PRETTY_NAME=") {
                facts.insert("distro_name".into(), name.trim_matches('"').into());
            }
        }
    }

    // Package manager detection (for v2 cross-distro support).
    for (cmd, name) in [
        ("ark --version", "ark"),
        ("apt --version", "apt"),
        ("dnf --version", "dnf"),
        ("pacman --version", "pacman"),
        ("apk --version", "apk"),
    ] {
        if let Ok(r) = exec.exec(&format!("{} 2>/dev/null", cmd)).await
            && r.success()
        {
            facts.insert("pkg_manager".into(), name.into());
            break;
        }
    }

    // Init system detection.
    for (cmd, name) in [
        ("argonaut --version", "argonaut"),
        ("systemctl --version", "systemd"),
        ("rc-status --version", "openrc"),
    ] {
        if let Ok(r) = exec.exec(&format!("{} 2>/dev/null", cmd)).await
            && r.success()
        {
            facts.insert("init_system".into(), name.into());
            break;
        }
    }

    facts
}

// ── Module trait ─────────────────────────────────────────────────────────────

/// What a module plans to do (dry-run output).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    pub module: String,
    pub action: String,
    pub changed: bool,
    pub description: String,
    #[serde(default)]
    pub diff: Option<String>,
}

/// Result of executing a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub module: String,
    pub action: String,
    pub success: bool,
    pub changed: bool,
    pub message: String,
}

/// The module trait — every orchestration module implements this.
#[allow(async_fn_in_trait)]
pub trait SutraModule: Send + Sync {
    /// Module name (e.g. "ark", "argonaut", "file").
    fn name(&self) -> &str;

    /// Supported actions (e.g. ["install", "remove", "upgrade"]).
    fn actions(&self) -> &[&str];

    /// Plan: return the diff between current and desired state.
    async fn plan(&self, task: &Task, node: &NodeInfo, exec: &Executor)
    -> anyhow::Result<TaskPlan>;

    /// Apply: execute the change. Only called after user confirmation.
    async fn apply(
        &self,
        task: &Task,
        node: &NodeInfo,
        exec: &Executor,
    ) -> anyhow::Result<TaskResult>;

    /// Check: is desired state already met? (idempotency guard)
    async fn check(&self, task: &Task, node: &NodeInfo, exec: &Executor) -> anyhow::Result<bool>;
}

// ── Inventory & targeting ───────────────────────────────────────────────────

/// Inventory — collection of nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    #[serde(default)]
    pub node: Vec<NodeInfo>,
}

/// Check if a node matches a set of playbook targets.
/// If targets is empty, matches all nodes.
pub fn target_matches(node: &NodeInfo, targets: &[Target]) -> bool {
    if targets.is_empty() {
        return true;
    }

    targets.iter().any(|t| {
        if t.all == Some(true) {
            return true;
        }
        let role_ok = t.role.as_ref().is_none_or(|r| r == &node.role);
        let arch_ok = t.arch.as_ref().is_none_or(|a| a == &node.arch);
        let id_ok = t.node_id.as_ref().is_none_or(|id| id == &node.id);
        let tag_ok = t.tag.as_ref().is_none_or(|tag| node.tags.contains(tag));
        role_ok && arch_ok && id_ok && tag_ok
    })
}

// ── Audit trail ─────────────────────────────────────────────────────────────

/// Record of a playbook run for audit/rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: String,
    pub playbook: String,
    pub node_id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub results: Vec<TaskResult>,
}

impl RunRecord {
    pub fn new(playbook: &str, node_id: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            playbook: playbook.to_string(),
            node_id: node_id.to_string(),
            started_at: Utc::now(),
            finished_at: None,
            results: Vec::new(),
        }
    }

    pub fn finish(&mut self) {
        self.finished_at = Some(Utc::now());
    }

    /// Append to a JSON-lines audit log.
    pub fn write_to_log(&self, log_dir: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(log_dir)?;
        let path = log_dir.join("sutra-audit.jsonl");
        let line = serde_json::to_string(self)?;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }
}

// ── Parsing ─────────────────────────────────────────────────────────────────

/// Parse a playbook from a TOML file.
pub fn parse_playbook(path: &Path) -> anyhow::Result<Playbook> {
    let content = std::fs::read_to_string(path)?;
    let playbook: Playbook = toml::from_str(&content)?;
    Ok(playbook)
}

/// Parse an inventory from a TOML file.
pub fn parse_inventory(path: &Path) -> anyhow::Result<Inventory> {
    let content = std::fs::read_to_string(path)?;
    let inventory: Inventory = toml::from_str(&content)?;
    Ok(inventory)
}

/// Convert a YAML playbook string to canonical TOML.
pub fn yaml_to_toml(yaml_str: &str) -> anyhow::Result<String> {
    let value: serde_yaml::Value = serde_yaml::from_str(yaml_str)?;
    let toml_value = yaml_value_to_toml_value(&value)?;
    let toml_str = toml::to_string_pretty(&toml_value)?;
    Ok(toml_str)
}

/// Convert a TOML playbook string to YAML.
pub fn toml_to_yaml(toml_str: &str) -> anyhow::Result<String> {
    let value: toml::Value = toml::from_str(toml_str)?;
    let yaml_value = toml_value_to_yaml_value(&value);
    let yaml_str = serde_yaml::to_string(&yaml_value)?;
    Ok(yaml_str)
}

fn yaml_value_to_toml_value(yaml: &serde_yaml::Value) -> anyhow::Result<toml::Value> {
    match yaml {
        serde_yaml::Value::Null => Ok(toml::Value::String(String::new())),
        serde_yaml::Value::Bool(b) => Ok(toml::Value::Boolean(*b)),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(toml::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(toml::Value::Float(f))
            } else {
                Ok(toml::Value::String(n.to_string()))
            }
        }
        serde_yaml::Value::String(s) => Ok(toml::Value::String(s.clone())),
        serde_yaml::Value::Sequence(seq) => {
            let arr: Result<Vec<_>, _> = seq.iter().map(yaml_value_to_toml_value).collect();
            Ok(toml::Value::Array(arr?))
        }
        serde_yaml::Value::Mapping(map) => {
            let mut table = toml::map::Map::new();
            for (k, v) in map {
                let key = match k {
                    serde_yaml::Value::String(s) => s.clone(),
                    other => serde_yaml::to_string(other)?.trim().to_string(),
                };
                table.insert(key, yaml_value_to_toml_value(v)?);
            }
            Ok(toml::Value::Table(table))
        }
        serde_yaml::Value::Tagged(tagged) => yaml_value_to_toml_value(&tagged.value),
    }
}

fn toml_value_to_yaml_value(toml_val: &toml::Value) -> serde_yaml::Value {
    match toml_val {
        toml::Value::String(s) => serde_yaml::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_yaml::Value::Number((*i).into()),
        toml::Value::Float(f) => serde_yaml::Value::Number(serde_yaml::Number::from(*f)),
        toml::Value::Boolean(b) => serde_yaml::Value::Bool(*b),
        toml::Value::Datetime(dt) => serde_yaml::Value::String(dt.to_string()),
        toml::Value::Array(arr) => {
            let seq: Vec<_> = arr.iter().map(toml_value_to_yaml_value).collect();
            serde_yaml::Value::Sequence(seq)
        }
        toml::Value::Table(table) => {
            let mut map = serde_yaml::Mapping::new();
            for (k, v) in table {
                map.insert(
                    serde_yaml::Value::String(k.clone()),
                    toml_value_to_yaml_value(v),
                );
            }
            serde_yaml::Value::Mapping(map)
        }
    }
}

// ── Shell safety ────────────────────────────────────────────────────────────

/// Shell-escape a string for safe interpolation into shell commands.
/// Returns a quoted string that is safe to pass to `sh -c`.
pub fn esc(s: &str) -> String {
    shlex::try_quote(s)
        .unwrap_or_else(|_| std::borrow::Cow::Owned(format!("'{}'", s.replace('\'', "'\\''"))))
        .into_owned()
}

// ── Task ordering ───────────────────────────────────────────────────────────

/// Resolve effective on_error for a task (task-level overrides playbook-level).
pub fn resolve_on_error<'a>(task: &'a Task, playbook_default: &'a OnError) -> &'a OnError {
    task.on_error.as_ref().unwrap_or(playbook_default)
}

/// Order tasks respecting `depends_on`. Returns indices in execution order.
/// Tasks without dependencies maintain their original order.
pub fn order_tasks(tasks: &[Task]) -> anyhow::Result<Vec<usize>> {
    let n = tasks.len();

    // Build name-to-index map.
    let mut name_map: HashMap<&str, usize> = HashMap::new();
    for (i, task) in tasks.iter().enumerate() {
        if let Some(ref name) = task.name {
            name_map.insert(name.as_str(), i);
        }
    }

    // Build adjacency: deps[i] = set of indices that i depends on.
    let mut deps: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, task) in tasks.iter().enumerate() {
        for dep_name in &task.depends_on {
            let Some(&dep_idx) = name_map.get(dep_name.as_str()) else {
                anyhow::bail!(
                    "task {} depends on '{}', which does not exist",
                    task.name.as_deref().unwrap_or(&format!("#{}", i)),
                    dep_name
                );
            };
            deps[i].push(dep_idx);
        }
    }

    // Kahn's algorithm for topological sort.
    let mut in_degree = vec![0usize; n];
    for d in &deps {
        for &dep in d {
            in_degree[dep] += 0; // dep doesn't increase its own in_degree
        }
    }
    // Actually: in_degree[i] = number of deps that i has
    // We want: for each i, in_degree[i] = how many tasks depend on it being done
    // Re-think: we need successors, not predecessors.
    let mut successors: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_deg = vec![0usize; n];
    for (i, d) in deps.iter().enumerate() {
        in_deg[i] = d.len();
        for &dep in d {
            successors[dep].push(i);
        }
    }

    let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    // Enqueue tasks with no dependencies, in original order.
    for (i, deg) in in_deg.iter().enumerate() {
        if *deg == 0 {
            queue.push_back(i);
        }
    }

    let mut order = Vec::with_capacity(n);
    while let Some(idx) = queue.pop_front() {
        order.push(idx);
        for &succ in &successors[idx] {
            in_deg[succ] -= 1;
            if in_deg[succ] == 0 {
                queue.push_back(succ);
            }
        }
    }

    if order.len() != n {
        anyhow::bail!("circular dependency detected in tasks");
    }

    Ok(order)
}

// ── Helper for extracting task params ───────────────────────────────────────

/// Get a string param from a task, returning a default if not found.
pub fn param_str<'a>(task: &'a Task, key: &str, default: &'a str) -> &'a str {
    task.params
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or(default)
}

/// Get an integer param from a task.
pub fn param_int(task: &Task, key: &str, default: i64) -> i64 {
    task.params
        .get(key)
        .and_then(|v| v.as_integer())
        .unwrap_or(default)
}

/// Get a boolean param from a task.
pub fn param_bool(task: &Task, key: &str, default: bool) -> bool {
    task.params
        .get(key)
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

// ── JSON output events ──────────────────────────────────────────────────────

/// Structured output events for `--output json`.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum OutputEvent {
    #[serde(rename = "run_start")]
    RunStart {
        playbook: String,
        nodes: Vec<String>,
    },
    #[serde(rename = "node_start")]
    NodeStart {
        node_id: String,
        #[serde(skip_serializing_if = "HashMap::is_empty")]
        facts: HashMap<String, String>,
    },
    #[serde(rename = "task_check")]
    TaskCheck {
        node_id: String,
        module: String,
        action: String,
        met: bool,
    },
    #[serde(rename = "task_plan")]
    TaskPlanEvent { node_id: String, plan: TaskPlan },
    #[serde(rename = "task_result")]
    TaskResultEvent { node_id: String, result: TaskResult },
    #[serde(rename = "node_end")]
    NodeEnd { node_id: String, success: bool },
    #[serde(rename = "run_end")]
    RunEnd {
        success: bool,
        changed: u32,
        ok: u32,
        failed: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_playbook() {
        let toml_content = r#"
[playbook]
name = "Test playbook"
description = "A test"

[[target]]
role = "edge"
arch = "aarch64"

[[task]]
module = "ark"
action = "install"
package = "tarang"
version = "2026.3.18"

[[task]]
module = "verify"
action = "port_listening"
port = 8070
"#;
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(toml_content.as_bytes()).unwrap();
        let pb = parse_playbook(f.path()).unwrap();
        assert_eq!(pb.playbook.name, "Test playbook");
        assert_eq!(pb.target.len(), 1);
        assert_eq!(pb.target[0].role, Some("edge".to_string()));
        assert_eq!(pb.task.len(), 2);
        assert_eq!(pb.task[0].module, "ark");
        assert_eq!(pb.task[1].module, "verify");
    }

    #[test]
    fn test_parse_playbook_with_vars() {
        let toml_content = r#"
[playbook]
name = "Test with vars"

[vars]
version = "2026.3.18"
config_dir = "/etc/tarang"

[[task]]
module = "ark"
action = "install"
package = "tarang"
version = "{{ version }}"
"#;
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(toml_content.as_bytes()).unwrap();
        let pb = parse_playbook(f.path()).unwrap();
        assert_eq!(pb.vars.len(), 2);
        assert_eq!(
            pb.vars.get("version").unwrap().as_str().unwrap(),
            "2026.3.18"
        );
    }

    #[test]
    fn test_var_context_expansion() {
        let mut playbook_vars = HashMap::new();
        playbook_vars.insert(
            "version".to_string(),
            toml::Value::String("2026.3.18".to_string()),
        );
        playbook_vars.insert("port".to_string(), toml::Value::Integer(8080));

        let mut ctx = VarContext::new(&playbook_vars, &[]);
        ctx.facts.insert("arch".into(), "x86_64".into());

        assert_eq!(ctx.expand("version {{ version }}"), "version 2026.3.18");
        assert_eq!(ctx.expand("port {{ port }}"), "port 8080");
        assert_eq!(ctx.expand("arch {{ fact.arch }}"), "arch x86_64");
        assert_eq!(ctx.expand("no vars here"), "no vars here");
    }

    #[test]
    fn test_var_context_expand_task() {
        let mut playbook_vars = HashMap::new();
        playbook_vars.insert(
            "version".to_string(),
            toml::Value::String("2026.3.18".to_string()),
        );
        let ctx = VarContext::new(&playbook_vars, &[]);

        let mut params = HashMap::new();
        params.insert(
            "package".to_string(),
            toml::Value::String("tarang".to_string()),
        );
        params.insert(
            "version".to_string(),
            toml::Value::String("{{ version }}".to_string()),
        );
        let task = Task::new("ark", "install", params);

        let expanded = ctx.expand_task(&task);
        assert_eq!(
            expanded.params.get("version").unwrap().as_str().unwrap(),
            "2026.3.18"
        );
        assert_eq!(
            expanded.params.get("package").unwrap().as_str().unwrap(),
            "tarang"
        );
    }

    #[test]
    fn test_cli_vars_override_playbook() {
        let mut playbook_vars = HashMap::new();
        playbook_vars.insert(
            "version".to_string(),
            toml::Value::String("old".to_string()),
        );

        let cli_vars = vec![("version".to_string(), "new".to_string())];
        let ctx = VarContext::new(&playbook_vars, &cli_vars);

        assert_eq!(ctx.expand("{{ version }}"), "new");
    }

    #[test]
    fn test_parse_inventory() {
        let toml_content = r#"
[[node]]
id = "rpi-kitchen"
host = "192.168.1.50"
role = "edge"
arch = "aarch64"
tags = ["iot", "home"]
transport = "daimon"

[[node]]
id = "nuc-office"
host = "192.168.1.10"
role = "desktop"
arch = "x86_64"
tags = ["workstation"]
transport = "ssh"
ssh_user = "user"
ssh_port = 2222
ssh_key = "/home/user/.ssh/id_ed25519"
"#;
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(toml_content.as_bytes()).unwrap();
        let inv = parse_inventory(f.path()).unwrap();
        assert_eq!(inv.node.len(), 2);
        assert_eq!(inv.node[0].id, "rpi-kitchen");
        assert_eq!(inv.node[0].transport, "daimon");
        assert_eq!(inv.node[1].ssh_user, Some("user".to_string()));
        assert_eq!(inv.node[1].ssh_port, Some(2222));
        assert_eq!(
            inv.node[1].ssh_key,
            Some("/home/user/.ssh/id_ed25519".to_string())
        );
    }

    #[test]
    fn test_yaml_to_toml_roundtrip() {
        let yaml = r#"
playbook:
  name: Deploy tarang
  description: Install on edge fleet
task:
  - module: ark
    action: install
    package: tarang
"#;
        let toml_str = yaml_to_toml(yaml).unwrap();
        assert!(toml_str.contains("Deploy tarang"));
        assert!(toml_str.contains("ark"));

        let yaml_again = toml_to_yaml(&toml_str).unwrap();
        assert!(yaml_again.contains("Deploy tarang"));
    }

    #[test]
    fn test_toml_to_yaml() {
        let toml_str = r#"
[playbook]
name = "Test"
description = "A test playbook"

[[task]]
module = "file"
action = "template"
path = "/etc/agnos/config.toml"
"#;
        let yaml = toml_to_yaml(toml_str).unwrap();
        assert!(yaml.contains("Test"));
        assert!(yaml.contains("file"));
        assert!(yaml.contains("template"));
    }

    #[test]
    fn test_run_record() {
        let record = RunRecord::new("deploy-tarang.toml", "local");
        assert!(!record.id.is_empty());
        assert_eq!(record.playbook, "deploy-tarang.toml");
        assert_eq!(record.node_id, "local");
        assert!(record.finished_at.is_none());
        assert!(record.results.is_empty());
    }

    #[test]
    fn test_target_all() {
        let target = Target {
            role: None,
            arch: None,
            node_id: None,
            tag: None,
            all: Some(true),
        };
        assert_eq!(target.all, Some(true));
    }

    #[test]
    fn test_task_plan_no_change() {
        let plan = TaskPlan {
            module: "ark".to_string(),
            action: "install".to_string(),
            changed: false,
            description: "tarang 2026.3.18 already installed".to_string(),
            diff: None,
        };
        assert!(!plan.changed);
    }

    #[test]
    fn test_task_result_success() {
        let result = TaskResult {
            module: "argonaut".to_string(),
            action: "enable".to_string(),
            success: true,
            changed: true,
            message: "tarang.service enabled".to_string(),
        };
        assert!(result.success);
        assert!(result.changed);
    }

    #[test]
    fn test_target_matches_empty_targets() {
        let node = NodeInfo::localhost();
        assert!(target_matches(&node, &[]));
    }

    #[test]
    fn test_target_matches_role() {
        let node = NodeInfo {
            id: "n1".to_string(),
            host: "h1".to_string(),
            role: "edge".to_string(),
            arch: "aarch64".to_string(),
            tags: vec![],
            transport: "local".to_string(),
            ssh_user: None,
            ssh_port: None,
            ssh_key: None,
        };
        let targets = vec![Target {
            role: Some("edge".to_string()),
            arch: None,
            node_id: None,
            tag: None,
            all: None,
        }];
        assert!(target_matches(&node, &targets));

        let targets_miss = vec![Target {
            role: Some("desktop".to_string()),
            arch: None,
            node_id: None,
            tag: None,
            all: None,
        }];
        assert!(!target_matches(&node, &targets_miss));
    }

    #[test]
    fn test_target_matches_tag() {
        let node = NodeInfo {
            id: "n1".to_string(),
            host: "h1".to_string(),
            role: "edge".to_string(),
            arch: "aarch64".to_string(),
            tags: vec!["iot".to_string(), "home".to_string()],
            transport: "local".to_string(),
            ssh_user: None,
            ssh_port: None,
            ssh_key: None,
        };
        let targets = vec![Target {
            role: None,
            arch: None,
            node_id: None,
            tag: Some("iot".to_string()),
            all: None,
        }];
        assert!(target_matches(&node, &targets));
    }

    #[tokio::test]
    async fn test_executor_local_exec() {
        let exec = Executor::local();
        let result = exec.exec("echo hello").await.unwrap();
        assert!(result.success());
        assert!(result.stdout.contains("hello"));
    }

    #[tokio::test]
    async fn test_executor_local_file_ops() {
        let exec = Executor::local();
        let path = format!("/tmp/sutra-core-test-{}", std::process::id());

        exec.write_file(&path, b"test content", 0o644)
            .await
            .unwrap();
        assert!(exec.file_exists(&path).await.unwrap());

        let content = exec.read_file_string(&path).await.unwrap();
        assert_eq!(content, "test content");

        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn test_gather_facts() {
        let exec = Executor::local();
        let facts = gather_facts(&exec).await;
        assert!(facts.contains_key("os"));
        assert!(facts.contains_key("arch"));
        assert!(facts.contains_key("hostname"));
    }

    #[test]
    fn test_param_helpers() {
        let mut params = HashMap::new();
        params.insert(
            "package".to_string(),
            toml::Value::String("tarang".to_string()),
        );
        params.insert("port".to_string(), toml::Value::Integer(8080));
        params.insert("enabled".to_string(), toml::Value::Boolean(true));

        let task = Task::new("test", "test", params);

        assert_eq!(param_str(&task, "package", "none"), "tarang");
        assert_eq!(param_str(&task, "missing", "default"), "default");
        assert_eq!(param_int(&task, "port", 0), 8080);
        assert_eq!(param_int(&task, "missing", 99), 99);
        assert!(param_bool(&task, "enabled", false));
        assert!(!param_bool(&task, "missing", false));
    }

    #[test]
    fn test_output_event_serialization() {
        let event = OutputEvent::RunStart {
            playbook: "test.toml".into(),
            nodes: vec!["n1".into(), "n2".into()],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("run_start"));
        assert!(json.contains("test.toml"));
    }
}
