//! Integration tests — run real playbooks against temp directories.

use std::io::Write;

use sutra_core::{
    parse_playbook, target_matches, Executor, NodeInfo, RunRecord, SutraModule,
};
use sutra_modules::ModuleRegistry;
use tempfile::NamedTempFile;

fn local_node() -> NodeInfo {
    NodeInfo::localhost()
}

/// Helper: write a TOML playbook to a temp file and parse it.
fn playbook_from_str(toml: &str) -> (tempfile::NamedTempFile, sutra_core::Playbook) {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();
    let pb = parse_playbook(f.path()).unwrap();
    (f, pb)
}

/// Run a playbook through the full plan→check→apply→check cycle.
async fn run_playbook(
    pb: &sutra_core::Playbook,
    node: &NodeInfo,
) -> Vec<sutra_core::TaskResult> {
    let registry = ModuleRegistry::new();
    let exec = Executor::for_node(node);
    let mut results = Vec::new();

    for task in &pb.task {
        let module = registry.get(&task.module).expect("unknown module");

        let already = module.check(task, node, &exec).await.unwrap_or(false);
        if already {
            results.push(sutra_core::TaskResult {
                module: task.module.clone(),
                action: task.action.clone(),
                success: true,
                changed: false,
                message: "already met".to_string(),
            });
            continue;
        }

        let result = module.apply(task, node, &exec).await.unwrap();
        results.push(result);
    }

    results
}

// ── File module integration ─────────────────────────────────────────────────

#[tokio::test]
async fn test_file_copy_lifecycle() {
    let path = format!("/tmp/sutra-integ-copy-{}", std::process::id());
    let toml = format!(
        r#"
[playbook]
name = "file copy test"

[[task]]
module = "file"
action = "copy"
path = "{path}"
content = "integration test content"

[[task]]
module = "verify"
action = "file_exists"
path = "{path}"
"#
    );

    let (_f, pb) = playbook_from_str(&toml);
    let node = local_node();

    // First run — should create file
    let results = run_playbook(&pb, &node).await;
    assert!(results[0].success);
    assert!(results[0].changed);
    assert!(results[1].success); // verify

    // Second run — idempotent, file already has correct content
    let results = run_playbook(&pb, &node).await;
    assert!(!results[0].changed, "file copy should be idempotent");

    // Cleanup
    let _ = tokio::fs::remove_file(&path).await;
}

#[tokio::test]
async fn test_file_absent_lifecycle() {
    let path = format!("/tmp/sutra-integ-absent-{}", std::process::id());

    // Create file first
    tokio::fs::write(&path, "temp").await.unwrap();

    let toml = format!(
        r#"
[playbook]
name = "file absent test"

[[task]]
module = "file"
action = "absent"
path = "{path}"
"#
    );

    let (_f, pb) = playbook_from_str(&toml);
    let node = local_node();

    let results = run_playbook(&pb, &node).await;
    assert!(results[0].success);
    assert!(results[0].changed);

    // Second run — already absent
    let results = run_playbook(&pb, &node).await;
    assert!(!results[0].changed);
}

#[tokio::test]
async fn test_file_line_in_file_lifecycle() {
    let path = format!("/tmp/sutra-integ-lif-{}", std::process::id());
    tokio::fs::write(&path, "existing line\n").await.unwrap();

    let toml = format!(
        r#"
[playbook]
name = "line_in_file test"

[[task]]
module = "file"
action = "line_in_file"
path = "{path}"
line = "new config line"
"#
    );

    let (_f, pb) = playbook_from_str(&toml);
    let node = local_node();

    let results = run_playbook(&pb, &node).await;
    assert!(results[0].success);
    assert!(results[0].changed);

    // Verify content
    let content = tokio::fs::read_to_string(&path).await.unwrap();
    assert!(content.contains("existing line"));
    assert!(content.contains("new config line"));

    // Second run — idempotent
    let results = run_playbook(&pb, &node).await;
    assert!(!results[0].changed);

    let _ = tokio::fs::remove_file(&path).await;
}

// ── Shell module integration ────────────────────────────────────────────────

#[tokio::test]
async fn test_shell_run_with_creates() {
    let path = format!("/tmp/sutra-integ-shell-{}", std::process::id());
    let toml = format!(
        r#"
[playbook]
name = "shell creates test"

[[task]]
module = "shell"
action = "run"
cmd = "echo hello > {path}"
creates = "{path}"
"#
    );

    let (_f, pb) = playbook_from_str(&toml);
    let node = local_node();

    let results = run_playbook(&pb, &node).await;
    assert!(results[0].success);
    assert!(results[0].changed);

    // Second run — file exists, should skip
    let results = run_playbook(&pb, &node).await;
    assert!(!results[0].changed, "shell with creates should be idempotent");

    let _ = tokio::fs::remove_file(&path).await;
}

// ── User module integration ─────────────────────────────────────────────────

#[tokio::test]
async fn test_user_check_existing() {
    let toml = r#"
[playbook]
name = "user check test"

[[task]]
module = "user"
action = "present"
username = "root"
"#;

    let (_f, pb) = playbook_from_str(toml);
    let node = local_node();

    // root exists — should not change
    let results = run_playbook(&pb, &node).await;
    assert!(!results[0].changed);
}

// ── Multi-task playbook ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_multi_task_playbook() {
    let dir = format!("/tmp/sutra-integ-multi-{}", std::process::id());
    let file1 = format!("{}/config.txt", dir);
    let file2 = format!("{}/data.txt", dir);

    let toml = format!(
        r#"
[playbook]
name = "multi-task test"

[[task]]
module = "shell"
action = "run"
cmd = "mkdir -p {dir}"
creates = "{dir}"

[[task]]
module = "file"
action = "copy"
path = "{file1}"
content = "config data"

[[task]]
module = "file"
action = "copy"
path = "{file2}"
content = "payload data"

[[task]]
module = "verify"
action = "file_exists"
path = "{file1}"

[[task]]
module = "verify"
action = "file_exists"
path = "{file2}"
"#
    );

    let (_f, pb) = playbook_from_str(&toml);
    let node = local_node();

    let results = run_playbook(&pb, &node).await;
    assert_eq!(results.len(), 5);
    assert!(results.iter().all(|r| r.success));

    // Verify content
    let c1 = tokio::fs::read_to_string(&file1).await.unwrap();
    assert_eq!(c1, "config data");
    let c2 = tokio::fs::read_to_string(&file2).await.unwrap();
    assert_eq!(c2, "payload data");

    let _ = tokio::fs::remove_dir_all(&dir).await;
}

// ── Target filtering ────────────────────────────────────────────────────────

#[test]
fn test_target_filtering_complex() {
    let nodes = vec![
        NodeInfo {
            id: "edge-1".into(),
            host: "10.0.0.1".into(),
            role: "edge".into(),
            arch: "aarch64".into(),
            tags: vec!["iot".into(), "kitchen".into()],
            transport: "local".into(),
            ssh_user: None,
            ssh_port: None,
            ssh_key: None,
        },
        NodeInfo {
            id: "edge-2".into(),
            host: "10.0.0.2".into(),
            role: "edge".into(),
            arch: "x86_64".into(),
            tags: vec!["iot".into()],
            transport: "local".into(),
            ssh_user: None,
            ssh_port: None,
            ssh_key: None,
        },
        NodeInfo {
            id: "server-1".into(),
            host: "10.0.0.10".into(),
            role: "server".into(),
            arch: "x86_64".into(),
            tags: vec!["production".into()],
            transport: "ssh".into(),
            ssh_user: Some("deploy".into()),
            ssh_port: None,
            ssh_key: None,
        },
    ];

    // Target: edge + aarch64
    let targets = vec![sutra_core::Target {
        role: Some("edge".into()),
        arch: Some("aarch64".into()),
        node_id: None,
        tag: None,
        all: None,
    }];
    let matched: Vec<_> = nodes.iter().filter(|n| target_matches(n, &targets)).collect();
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].id, "edge-1");

    // Target: all nodes with tag "iot"
    let targets = vec![sutra_core::Target {
        role: None,
        arch: None,
        node_id: None,
        tag: Some("iot".into()),
        all: None,
    }];
    let matched: Vec<_> = nodes.iter().filter(|n| target_matches(n, &targets)).collect();
    assert_eq!(matched.len(), 2);

    // Target: all
    let targets = vec![sutra_core::Target {
        role: None,
        arch: None,
        node_id: None,
        tag: None,
        all: Some(true),
    }];
    let matched: Vec<_> = nodes.iter().filter(|n| target_matches(n, &targets)).collect();
    assert_eq!(matched.len(), 3);

    // Target: specific node
    let targets = vec![sutra_core::Target {
        role: None,
        arch: None,
        node_id: Some("server-1".into()),
        tag: None,
        all: None,
    }];
    let matched: Vec<_> = nodes.iter().filter(|n| target_matches(n, &targets)).collect();
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].id, "server-1");
}

// ── Audit trail ─────────────────────────────────────────────────────────────

#[test]
fn test_audit_trail_write_and_read() {
    let dir = tempfile::tempdir().unwrap();
    let mut record = RunRecord::new("test-playbook.toml", "local");
    record.results.push(sutra_core::TaskResult {
        module: "file".into(),
        action: "copy".into(),
        success: true,
        changed: true,
        message: "wrote /tmp/test".into(),
    });
    record.finish();

    record.write_to_log(dir.path()).unwrap();

    let log_path = dir.path().join("sutra-audit.jsonl");
    assert!(log_path.exists());

    let content = std::fs::read_to_string(&log_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    assert_eq!(parsed["node_id"], "local");
    assert_eq!(parsed["results"][0]["module"], "file");
    assert!(parsed["finished_at"].is_string());
}

// ── Validate ────────────────────────────────────────────────────────────────

#[test]
fn test_validate_catches_unknown_module() {
    let toml = r#"
[playbook]
name = "bad playbook"

[[task]]
module = "nonexistent"
action = "do_thing"
"#;
    let (_f, _pb) = playbook_from_str(toml);
    let registry = ModuleRegistry::new();
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn test_validate_all_core_modules_registered() {
    let registry = ModuleRegistry::new();
    for name in &["ark", "argonaut", "file", "shell", "user", "verify"] {
        assert!(registry.get(name).is_some(), "{} not registered", name);
    }
}
