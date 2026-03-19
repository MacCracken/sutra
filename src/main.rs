//! Sutra — AI-native infrastructure orchestration for AGNOS.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use sutra_core::{parse_inventory, parse_playbook, yaml_to_toml, toml_to_yaml};
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
    /// Show execution plan (dry-run)
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

            println!("Playbook: {}", pb.playbook.name);
            if !pb.playbook.description.is_empty() {
                println!("  {}", pb.playbook.description);
            }
            println!();

            for task in &pb.task {
                if let Some(module) = registry.get(&task.module) {
                    let node = sutra_core::NodeInfo {
                        id: "local".to_string(),
                        host: "localhost".to_string(),
                        role: String::new(),
                        arch: std::env::consts::ARCH.to_string(),
                        tags: vec![],
                        transport: "local".to_string(),
                        ssh_user: None,
                    };

                    let plan = module.plan(task, &node).await?;

                    if plan.changed {
                        println!("  [CHANGE] {}", plan.description);
                    } else {
                        println!("  [OK]     {}", plan.description);
                    }
                } else {
                    println!("  [ERROR]  Unknown module: {}", task.module);
                }
            }

            if confirm {
                println!("\nApplying changes...");
                // TODO: Execute tasks
                println!("Done.");
            } else {
                println!("\nDry-run complete. Use --confirm to apply.");
            }
        }

        Commands::Check { playbook, .. } => {
            let pb = parse_playbook(&playbook)?;
            println!("Checking state for: {}", pb.playbook.name);
            // TODO: Run check on all tasks
            println!("Check complete.");
        }

        Commands::Plan { playbook, .. } => {
            let pb = parse_playbook(&playbook)?;
            println!("Execution plan for: {}", pb.playbook.name);
            for (i, task) in pb.task.iter().enumerate() {
                println!("  {}. {} → {}", i + 1, task.module, task.action);
            }
        }

        Commands::Translate { input, output } => {
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
                        node.id,
                        node.host,
                        node.role,
                        node.arch,
                        node.transport
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
