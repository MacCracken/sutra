//! Sutra — AI-native infrastructure orchestration for AGNOS.

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};
use sutra_core::{
    Executor, NodeInfo, OnError, OutputEvent, RunRecord, SutraModule, TaskResult, VarContext,
    gather_facts, order_tasks, parse_inventory, parse_playbook, resolve_on_error, target_matches,
    toml_to_yaml, yaml_to_toml,
};
use sutra_modules::ModuleRegistry;

#[derive(Parser)]
#[command(
    name = "sutra",
    about = "Sutra — AI-native infrastructure orchestration for AGNOS",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format
    #[arg(long, global = true, default_value = "human")]
    output: OutputFormat,
}

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Subcommand)]
enum Commands {
    /// Apply playbook (dry-run by default, --confirm to execute)
    Apply {
        /// Path to TOML playbook
        playbook: PathBuf,
        /// Execute changes (default: dry-run)
        #[arg(long)]
        confirm: bool,
        /// Limit to specific node
        #[arg(long)]
        limit: Option<String>,
        /// Inventory file
        #[arg(short, long)]
        inventory: Option<PathBuf>,
        /// Run on nodes in parallel (max concurrent nodes)
        #[arg(short = 'j', long)]
        parallel: Option<usize>,
        /// Continue on node failure
        #[arg(long)]
        continue_on_error: bool,
        /// Set variable (key=value), can be repeated
        #[arg(long = "var", value_parser = parse_var)]
        vars: Vec<(String, String)>,
        /// Gather node facts before execution
        #[arg(long)]
        facts: bool,
    },
    /// Verify current state matches desired
    Check {
        /// Path to TOML playbook
        playbook: PathBuf,
        /// Inventory file
        #[arg(short, long)]
        inventory: Option<PathBuf>,
        /// Set variable (key=value)
        #[arg(long = "var", value_parser = parse_var)]
        vars: Vec<(String, String)>,
    },
    /// Show detailed execution plan
    Plan {
        /// Path to TOML playbook
        playbook: PathBuf,
        /// Inventory file
        #[arg(short, long)]
        inventory: Option<PathBuf>,
        /// Set variable (key=value)
        #[arg(long = "var", value_parser = parse_var)]
        vars: Vec<(String, String)>,
        /// Gather node facts before planning
        #[arg(long)]
        facts: bool,
    },
    /// Translate Markdown to TOML playbook via hoosh
    Translate {
        /// Path to Markdown file
        input: PathBuf,
        /// Output path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Convert between YAML and TOML formats
    Convert {
        /// Input file path
        input: PathBuf,
        /// Target format
        #[arg(long, value_parser = ["yaml", "toml"])]
        to: String,
        /// Output path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// List all known nodes
    Inventory {
        /// Inventory file
        #[arg(short, long)]
        file: Option<PathBuf>,
        /// Include daimon fleet nodes
        #[arg(long)]
        from_daimon: bool,
    },
    /// List available modules and actions
    Modules,
    /// Validate playbook syntax and module references
    Validate {
        /// Path to TOML playbook
        playbook: PathBuf,
    },
    /// Natural language to TOML via hoosh
    Nl {
        /// Natural language description
        prompt: Vec<String>,
        /// Output path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn parse_var(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("expected KEY=VALUE, got '{}'", s))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

/// Output helper — emit human-readable or JSON output.
struct Output {
    format: OutputFormat,
}

impl Output {
    fn emit(&self, event: &OutputEvent) {
        match self.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(event).unwrap());
            }
            OutputFormat::Human => {
                // Human output is handled inline for richer formatting.
            }
        }
    }

    fn is_json(&self) -> bool {
        matches!(self.format, OutputFormat::Json)
    }
}

/// Resolve the list of target nodes for a playbook run.
fn resolve_nodes(
    pb: &sutra_core::Playbook,
    inventory: &Option<PathBuf>,
    limit: &Option<String>,
) -> anyhow::Result<Vec<NodeInfo>> {
    let mut nodes = if let Some(inv_path) = inventory {
        let inv = parse_inventory(inv_path)?;
        inv.node
    } else {
        vec![NodeInfo::localhost()]
    };

    nodes.retain(|n| target_matches(n, &pb.target));

    if let Some(limit_id) = limit {
        nodes.retain(|n| n.id == *limit_id);
    }

    if nodes.is_empty() {
        anyhow::bail!("no nodes match the playbook targets");
    }

    Ok(nodes)
}

fn audit_log_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SUTRA_AUDIT_DIR") {
        PathBuf::from(dir)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".local/share/sutra/audit")
    }
}

/// Execute tasks on a single node. Returns (node_id, record, output_lines, success).
struct NodeRunParams {
    node: NodeInfo,
    tasks: Vec<sutra_core::Task>,
    task_order: Vec<usize>,
    registry: Arc<ModuleRegistry>,
    confirm: bool,
    var_ctx: VarContext,
    gather_node_facts: bool,
    playbook_on_error: OnError,
    playbook_path: String,
    out: Arc<Output>,
}

async fn execute_on_node(
    p: NodeRunParams,
) -> anyhow::Result<(String, RunRecord, Vec<String>, bool)> {
    let NodeRunParams {
        node,
        tasks,
        task_order,
        registry,
        confirm,
        var_ctx,
        gather_node_facts,
        playbook_on_error,
        playbook_path,
        out,
    } = p;
    let exec = Executor::for_node(&node);
    let mut record = RunRecord::new(&playbook_path, &node.id);
    let mut lines = Vec::new();
    let mut success = true;

    // Gather facts if requested.
    let mut ctx = var_ctx;
    if gather_node_facts {
        ctx.facts = gather_facts(&exec).await;
        if out.is_json() {
            out.emit(&OutputEvent::NodeStart {
                node_id: node.id.clone(),
                facts: ctx.facts.clone(),
            });
        } else {
            lines.push(format!(
                "  Facts: os={}, arch={}, hostname={}",
                ctx.facts.get("os").unwrap_or(&"?".into()),
                ctx.facts.get("arch").unwrap_or(&"?".into()),
                ctx.facts.get("hostname").unwrap_or(&"?".into()),
            ));
        }
    } else if out.is_json() {
        out.emit(&OutputEvent::NodeStart {
            node_id: node.id.clone(),
            facts: std::collections::HashMap::new(),
        });
    }

    for &idx in &task_order {
        let task = &tasks[idx];
        let expanded = ctx.expand_task(task);
        let err_strategy = resolve_on_error(&expanded, &playbook_on_error);

        let Some(module) = registry.get(&expanded.module) else {
            let msg = format!("  [ERROR]  Unknown module: {}", expanded.module);
            lines.push(msg);
            continue;
        };

        // Idempotency check.
        let already_met = module.check(&expanded, &node, &exec).await.unwrap_or(false);

        if out.is_json() {
            out.emit(&OutputEvent::TaskCheck {
                node_id: node.id.clone(),
                module: expanded.module.clone(),
                action: expanded.action.clone(),
                met: already_met,
            });
        }

        if already_met {
            let msg = format!(
                "  [OK]     {} {} — already in desired state",
                expanded.module, expanded.action
            );
            lines.push(msg);
            record.results.push(TaskResult {
                module: expanded.module.clone(),
                action: expanded.action.clone(),
                success: true,
                changed: false,
                message: "already in desired state".to_string(),
            });
            continue;
        }

        let plan = module.plan(&expanded, &node, &exec).await?;

        if out.is_json() {
            out.emit(&OutputEvent::TaskPlanEvent {
                node_id: node.id.clone(),
                plan: plan.clone(),
            });
        }

        if plan.changed {
            lines.push(format!("  [CHANGE] {}", plan.description));
        } else {
            lines.push(format!("  [OK]     {}", plan.description));
        }

        if let Some(ref diff) = plan.diff {
            for line in diff.lines() {
                lines.push(format!("           {}", line));
            }
        }

        if confirm && plan.changed {
            let result = module.apply(&expanded, &node, &exec).await?;

            if out.is_json() {
                out.emit(&OutputEvent::TaskResultEvent {
                    node_id: node.id.clone(),
                    result: result.clone(),
                });
            }

            if result.success {
                lines.push(format!("  [DONE]   {}", result.message));
            } else {
                match err_strategy {
                    OnError::Fail => {
                        lines.push(format!("  [FAIL]   {}", result.message));
                        record.results.push(result);
                        record.finish();
                        record.write_to_log(&audit_log_dir()).ok();
                        success = false;
                        break;
                    }
                    OnError::Continue => {
                        lines.push(format!("  [FAIL]   {} (continuing)", result.message));
                        success = false;
                    }
                    OnError::Ignore => {
                        lines.push(format!("  [SKIP]   {} (ignored)", result.message));
                    }
                }
            }
            record.results.push(result);
        }
    }

    record.finish();
    if confirm {
        record.write_to_log(&audit_log_dir()).ok();
    }

    if out.is_json() {
        out.emit(&OutputEvent::NodeEnd {
            node_id: node.id.clone(),
            success,
        });
    }

    Ok((node.id.clone(), record, lines, success))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let out = Arc::new(Output { format: cli.output });

    match cli.command {
        Commands::Apply {
            playbook,
            confirm,
            limit,
            inventory,
            parallel,
            continue_on_error,
            vars,
            facts,
        } => {
            let pb = parse_playbook(&playbook)?;
            let registry = Arc::new(ModuleRegistry::new());
            let nodes = resolve_nodes(&pb, &inventory, &limit)?;
            let var_ctx = VarContext::new(&pb.vars, &vars);
            let node_ids: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();

            if out.is_json() {
                out.emit(&OutputEvent::RunStart {
                    playbook: pb.playbook.name.clone(),
                    nodes: node_ids.clone(),
                });
            } else {
                println!("Playbook: {}", pb.playbook.name);
                if !pb.playbook.description.is_empty() {
                    println!("  {}", pb.playbook.description);
                }
                println!("Nodes: {}", node_ids.join(", "));
                println!();
            }

            let playbook_path = playbook.to_string_lossy().to_string();
            let task_order = order_tasks(&pb.task)?;
            let pb_on_error = pb.playbook.on_error.clone();
            let concurrency = parallel.unwrap_or(1);
            let multi = nodes.len() > 1;

            let mut changed_total = 0u32;
            let mut ok_total = 0u32;
            let mut failed_total = 0u32;
            let mut all_success = true;

            if concurrency > 1 && nodes.len() > 1 {
                // Parallel execution with bounded concurrency.
                let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
                let mut handles = Vec::new();

                for node in nodes {
                    let tasks = pb.task.clone();
                    let t_order = task_order.clone();
                    let reg = Arc::clone(&registry);
                    let ctx = var_ctx.clone();
                    let pb_err = pb_on_error.clone();
                    let pb_path = playbook_path.clone();
                    let sem = Arc::clone(&semaphore);
                    let o = Arc::clone(&out);

                    handles.push(tokio::spawn(async move {
                        let _permit = sem.acquire().await.unwrap();
                        execute_on_node(NodeRunParams {
                            node,
                            tasks,
                            task_order: t_order,
                            registry: reg,
                            confirm,
                            var_ctx: ctx,
                            gather_node_facts: facts,
                            playbook_on_error: pb_err,
                            playbook_path: pb_path,
                            out: o,
                        })
                        .await
                    }));
                }

                for handle in handles {
                    let result = handle.await??;
                    let (node_id, record, lines, success) = result;

                    if !out.is_json() {
                        if multi {
                            println!("--- {} ---", node_id);
                        }
                        for line in &lines {
                            println!("{}", line);
                        }
                    }

                    for r in &record.results {
                        if r.changed {
                            changed_total += 1;
                        } else {
                            ok_total += 1;
                        }
                        if !r.success {
                            failed_total += 1;
                        }
                    }

                    if !success {
                        all_success = false;
                        if !continue_on_error {
                            break;
                        }
                    }
                }
            } else {
                // Sequential execution.
                for node in nodes {
                    let tasks = pb.task.clone();
                    let (node_id, record, lines, success) = execute_on_node(NodeRunParams {
                        node,
                        tasks,
                        task_order: task_order.clone(),
                        registry: Arc::clone(&registry),
                        confirm,
                        var_ctx: var_ctx.clone(),
                        gather_node_facts: facts,
                        playbook_on_error: pb_on_error.clone(),
                        playbook_path: playbook_path.clone(),
                        out: Arc::clone(&out),
                    })
                    .await?;

                    if !out.is_json() {
                        if multi {
                            println!("--- {} ---", node_id);
                        }
                        for line in &lines {
                            println!("{}", line);
                        }
                    }

                    for r in &record.results {
                        if r.changed {
                            changed_total += 1;
                        } else {
                            ok_total += 1;
                        }
                        if !r.success {
                            failed_total += 1;
                        }
                    }

                    if !success {
                        all_success = false;
                        if !continue_on_error {
                            break;
                        }
                    }
                }
            }

            if out.is_json() {
                out.emit(&OutputEvent::RunEnd {
                    success: all_success,
                    changed: changed_total,
                    ok: ok_total,
                    failed: failed_total,
                });
            } else if confirm {
                println!(
                    "\nDone. changed={} ok={} failed={} | Audit: {}",
                    changed_total,
                    ok_total,
                    failed_total,
                    audit_log_dir().display()
                );
            } else {
                println!("\nDry-run complete. Use --confirm to apply.");
            }

            if !all_success {
                std::process::exit(1);
            }
        }

        Commands::Check {
            playbook,
            inventory,
            vars,
        } => {
            let pb = parse_playbook(&playbook)?;
            let registry = ModuleRegistry::new();
            let nodes = resolve_nodes(&pb, &inventory, &None)?;
            let var_ctx = VarContext::new(&pb.vars, &vars);

            if !out.is_json() {
                println!("Checking state for: {}", pb.playbook.name);
            }

            let mut all_ok = true;

            for node in &nodes {
                let exec = Executor::for_node(node);

                if !out.is_json() && nodes.len() > 1 {
                    println!("--- {} ---", node.id);
                }

                for task in &pb.task {
                    let expanded = var_ctx.expand_task(task);
                    let Some(module) = registry.get(&expanded.module) else {
                        eprintln!("  [ERROR]  Unknown module: {}", expanded.module);
                        all_ok = false;
                        continue;
                    };

                    match module.check(&expanded, node, &exec).await {
                        Ok(true) => {
                            if out.is_json() {
                                out.emit(&OutputEvent::TaskCheck {
                                    node_id: node.id.clone(),
                                    module: expanded.module.clone(),
                                    action: expanded.action.clone(),
                                    met: true,
                                });
                            } else {
                                println!("  [OK]     {} {}", expanded.module, expanded.action);
                            }
                        }
                        Ok(false) => {
                            if out.is_json() {
                                out.emit(&OutputEvent::TaskCheck {
                                    node_id: node.id.clone(),
                                    module: expanded.module.clone(),
                                    action: expanded.action.clone(),
                                    met: false,
                                });
                            } else {
                                println!(
                                    "  [DRIFT]  {} {} — desired state not met",
                                    expanded.module, expanded.action
                                );
                            }
                            all_ok = false;
                        }
                        Err(e) => {
                            eprintln!("  [ERROR]  {} {} — {}", expanded.module, expanded.action, e);
                            all_ok = false;
                        }
                    }
                }
            }

            if !out.is_json() {
                if all_ok {
                    println!("\nAll checks passed.");
                } else {
                    println!("\nSome checks failed. Run `sutra apply` to remediate.");
                }
            }

            if !all_ok {
                std::process::exit(1);
            }
        }

        Commands::Plan {
            playbook,
            inventory,
            vars,
            facts,
        } => {
            let pb = parse_playbook(&playbook)?;
            let registry = ModuleRegistry::new();
            let nodes = resolve_nodes(&pb, &inventory, &None)?;
            let mut var_ctx = VarContext::new(&pb.vars, &vars);

            if !out.is_json() {
                println!("Execution plan for: {}", pb.playbook.name);
                println!(
                    "Target nodes: {}\n",
                    nodes
                        .iter()
                        .map(|n| n.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }

            for node in &nodes {
                let exec = Executor::for_node(node);

                if facts {
                    var_ctx.facts = gather_facts(&exec).await;
                }

                if !out.is_json() && nodes.len() > 1 {
                    println!("--- {} ---", node.id);
                }

                for (i, task) in pb.task.iter().enumerate() {
                    let expanded = var_ctx.expand_task(task);
                    let Some(module) = registry.get(&expanded.module) else {
                        eprintln!("  {}. [ERROR] Unknown module: {}", i + 1, expanded.module);
                        continue;
                    };

                    let plan = module.plan(&expanded, node, &exec).await?;

                    if out.is_json() {
                        out.emit(&OutputEvent::TaskPlanEvent {
                            node_id: node.id.clone(),
                            plan,
                        });
                    } else {
                        let tag = if plan.changed { "CHANGE" } else { "OK" };
                        println!("  {}. [{}] {}", i + 1, tag, plan.description);
                        if let Some(ref diff) = plan.diff {
                            for line in diff.lines() {
                                println!("           {}", line);
                            }
                        }
                    }
                }
            }
        }

        Commands::Translate { input, output: _ } => {
            let content = std::fs::read_to_string(&input)?;
            let sections = sutra_ai::markdown::extract_sections(&content);
            let prompt = sections.to_prompt();

            println!("Extracted from Markdown:");
            println!("  Name: {}", sections.name);
            for section in &sections.sections {
                println!("  {}: {} items", section.heading, section.items.len());
            }
            println!();
            println!("Prompt for hoosh:");
            println!("{}", prompt);
            println!();
            println!("To generate TOML, run with hoosh available:");
            println!("  sutra nl {}", prompt.lines().next().unwrap_or(""));
        }

        Commands::Convert { input, to, output } => {
            let content = std::fs::read_to_string(&input)?;
            let result = match to.as_str() {
                "toml" => yaml_to_toml(&content)?,
                "yaml" => toml_to_yaml(&content)?,
                _ => anyhow::bail!("Unsupported target format: {}", to),
            };

            if let Some(out_path) = output {
                std::fs::write(&out_path, &result)?;
                println!("Written to {}", out_path.display());
            } else {
                print!("{}", result);
            }
        }

        Commands::Inventory { file, from_daimon } => {
            if let Some(path) = file {
                let inv = parse_inventory(&path)?;
                println!("Inventory: {} nodes", inv.node.len());
                for node in &inv.node {
                    println!(
                        "  {} ({}) — {} {} [{}]",
                        node.id, node.host, node.role, node.arch, node.transport
                    );
                }
            }

            if from_daimon {
                println!("\nFetching fleet from daimon...");
                let client =
                    sutra_ai::daimon::DaimonClient::new(sutra_ai::daimon::DaimonConfig::default());
                match client.fetch_fleet_nodes().await {
                    Ok(nodes) => {
                        println!("Fleet: {} nodes", nodes.len());
                        for node in &nodes {
                            println!(
                                "  {} ({}) — {} {}",
                                node.id, node.host, node.role, node.arch
                            );
                        }
                    }
                    Err(e) => println!("  Could not reach daimon: {}", e),
                }
            }
        }

        Commands::Modules => {
            let registry = ModuleRegistry::new();
            println!("Available modules:\n");
            for (name, actions) in registry.list() {
                println!("  {} — {}", name, actions.join(", "));
            }
        }

        Commands::Validate { playbook } => match parse_playbook(&playbook) {
            Ok(pb) => {
                let registry = ModuleRegistry::new();
                let mut errors = 0;

                for task in &pb.task {
                    if registry.get(&task.module).is_none() {
                        eprintln!("  ERROR: Unknown module '{}'", task.module);
                        errors += 1;
                    }

                    if let Some(module) = registry.get(&task.module)
                        && !module.actions().contains(&task.action.as_str())
                    {
                        eprintln!(
                            "  ERROR: Unknown action '{}' for module '{}'",
                            task.action, task.module
                        );
                        errors += 1;
                    }
                }

                if errors == 0 {
                    println!("Valid: {} ({} tasks)", pb.playbook.name, pb.task.len());
                } else {
                    eprintln!("{} validation errors", errors);
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("Parse error: {}", e);
                std::process::exit(1);
            }
        },

        Commands::Nl { prompt, output } => {
            let prompt_str = prompt.join(" ");
            println!("Translating: \"{}\"", prompt_str);

            let client =
                sutra_ai::daimon::HooshClient::new(sutra_ai::daimon::HooshConfig::default());
            match client.nl_to_toml(&prompt_str).await {
                Ok(toml) => {
                    if let Some(out_path) = output {
                        std::fs::write(&out_path, &toml)?;
                        println!("Written to {}", out_path.display());
                    } else {
                        println!("\n{}", toml);
                    }
                }
                Err(e) => {
                    eprintln!("Could not reach hoosh: {}", e);
                    eprintln!("Ensure hoosh is running on localhost:8088");
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
