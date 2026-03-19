//! Sutra — AI-native infrastructure orchestration for AGNOS.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use sutra_core::{
    parse_inventory, parse_playbook, target_matches, toml_to_yaml, yaml_to_toml, Executor,
    NodeInfo, RunRecord, SutraModule,
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
    },
    /// Verify current state matches desired
    Check {
        /// Path to TOML playbook
        playbook: PathBuf,
        /// Inventory file
        #[arg(short, long)]
        inventory: Option<PathBuf>,
    },
    /// Show detailed execution plan
    Plan {
        /// Path to TOML playbook
        playbook: PathBuf,
        /// Inventory file
        #[arg(short, long)]
        inventory: Option<PathBuf>,
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
    /// Validate playbook syntax
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

    // Filter by playbook targets.
    nodes.retain(|n| target_matches(n, &pb.target));

    // Apply --limit filter.
    if let Some(limit_id) = limit {
        nodes.retain(|n| n.id == *limit_id);
    }

    if nodes.is_empty() {
        anyhow::bail!("no nodes match the playbook targets");
    }

    Ok(nodes)
}

/// Default audit log directory.
fn audit_log_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SUTRA_AUDIT_DIR") {
        PathBuf::from(dir)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".local/share/sutra/audit")
    }
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

    match cli.command {
        Commands::Apply {
            playbook,
            confirm,
            limit,
            inventory,
        } => {
            let pb = parse_playbook(&playbook)?;
            let registry = ModuleRegistry::new();
            let nodes = resolve_nodes(&pb, &inventory, &limit)?;

            println!("Playbook: {}", pb.playbook.name);
            if !pb.playbook.description.is_empty() {
                println!("  {}", pb.playbook.description);
            }
            println!("Nodes: {}", nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>().join(", "));
            println!();

            for node in &nodes {
                let exec = Executor::for_node(node);
                let mut record = RunRecord::new(
                    playbook.to_string_lossy().as_ref(),
                    &node.id,
                );

                if nodes.len() > 1 {
                    println!("--- {} ({}) ---", node.id, node.host);
                }

                for task in &pb.task {
                    let Some(module) = registry.get(&task.module) else {
                        eprintln!("  [ERROR]  Unknown module: {}", task.module);
                        continue;
                    };

                    // Idempotency check — skip if desired state already met.
                    let already_met = module.check(task, node, &exec).await.unwrap_or(false);
                    if already_met {
                        println!("  [OK]     {} {} — already in desired state", task.module, task.action);
                        record.results.push(sutra_core::TaskResult {
                            module: task.module.clone(),
                            action: task.action.clone(),
                            success: true,
                            changed: false,
                            message: "already in desired state".to_string(),
                        });
                        continue;
                    }

                    let plan = module.plan(task, node, &exec).await?;

                    if plan.changed {
                        println!("  [CHANGE] {}", plan.description);
                    } else {
                        println!("  [OK]     {}", plan.description);
                    }

                    if let Some(ref diff) = plan.diff {
                        for line in diff.lines() {
                            println!("           {}", line);
                        }
                    }

                    if confirm && plan.changed {
                        let result = module.apply(task, node, &exec).await?;
                        if result.success {
                            println!("  [DONE]   {}", result.message);
                        } else {
                            eprintln!("  [FAIL]   {}", result.message);
                            record.results.push(result);
                            record.finish();
                            record.write_to_log(&audit_log_dir()).ok();
                            anyhow::bail!(
                                "task {} {} failed on {}, aborting",
                                task.module,
                                task.action,
                                node.id
                            );
                        }
                        record.results.push(result);
                    }
                }

                record.finish();
                if confirm {
                    record.write_to_log(&audit_log_dir()).ok();
                }
            }

            if confirm {
                println!("\nDone. Audit log: {}", audit_log_dir().display());
            } else {
                println!("\nDry-run complete. Use --confirm to apply.");
            }
        }

        Commands::Check { playbook, inventory } => {
            let pb = parse_playbook(&playbook)?;
            let registry = ModuleRegistry::new();
            let nodes = resolve_nodes(&pb, &inventory, &None)?;

            println!("Checking state for: {}", pb.playbook.name);
            let mut all_ok = true;

            for node in &nodes {
                let exec = Executor::for_node(node);

                if nodes.len() > 1 {
                    println!("--- {} ---", node.id);
                }

                for task in &pb.task {
                    let Some(module) = registry.get(&task.module) else {
                        eprintln!("  [ERROR]  Unknown module: {}", task.module);
                        all_ok = false;
                        continue;
                    };

                    match module.check(task, node, &exec).await {
                        Ok(true) => {
                            println!("  [OK]     {} {}", task.module, task.action);
                        }
                        Ok(false) => {
                            println!("  [DRIFT]  {} {} — desired state not met", task.module, task.action);
                            all_ok = false;
                        }
                        Err(e) => {
                            eprintln!("  [ERROR]  {} {} — {}", task.module, task.action, e);
                            all_ok = false;
                        }
                    }
                }
            }

            if all_ok {
                println!("\nAll checks passed.");
            } else {
                println!("\nSome checks failed. Run `sutra apply` to remediate.");
                std::process::exit(1);
            }
        }

        Commands::Plan { playbook, inventory } => {
            let pb = parse_playbook(&playbook)?;
            let registry = ModuleRegistry::new();
            let nodes = resolve_nodes(&pb, &inventory, &None)?;

            println!("Execution plan for: {}", pb.playbook.name);
            println!("Target nodes: {}\n", nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>().join(", "));

            for node in &nodes {
                let exec = Executor::for_node(node);

                if nodes.len() > 1 {
                    println!("--- {} ---", node.id);
                }

                for (i, task) in pb.task.iter().enumerate() {
                    let Some(module) = registry.get(&task.module) else {
                        eprintln!("  {}. [ERROR] Unknown module: {}", i + 1, task.module);
                        continue;
                    };

                    let plan = module.plan(task, node, &exec).await?;
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
                            println!("  {} ({}) — {} {}", node.id, node.host, node.role, node.arch);
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

        Commands::Validate { playbook } => {
            match parse_playbook(&playbook) {
                Ok(pb) => {
                    let registry = ModuleRegistry::new();
                    let mut errors = 0;

                    for task in &pb.task {
                        if registry.get(&task.module).is_none() {
                            eprintln!("  ERROR: Unknown module '{}'", task.module);
                            errors += 1;
                        }

                        if let Some(module) = registry.get(&task.module) {
                            if !module.actions().contains(&task.action.as_str()) {
                                eprintln!(
                                    "  ERROR: Unknown action '{}' for module '{}'",
                                    task.action, task.module
                                );
                                errors += 1;
                            }
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
            }
        }

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
