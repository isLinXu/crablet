use crate::config::Config;
use anyhow::Result;
use colored::Colorize;

pub fn handle_config(config: &Config) -> Result<()> {
    println!("{}", "⚙️  Crablet Configuration".bold().underline());
    println!();

    println!("{:<25} {}", "Property".dimmed(), "Value".dimmed());
    println!("{}", "─".repeat(60).dimmed());

    println!("{:<25} {}", "Database URL", config.database_url.cyan());
    println!("{:<25} {:?}", "Skills Directory", config.skills_dir);
    println!(
        "{:<25} {}",
        "LLM Vendor",
        config.llm_vendor.as_deref().unwrap_or("openai").green()
    );
    println!("{:<25} {}", "Model Name", config.model_name.yellow());
    println!("{:<25} {}", "Ollama Model", config.ollama_model.blue());
    println!("{:<25} {}", "Log Level", config.log_level);
    println!(
        "{:<25} {}",
        "Adaptive Routing",
        if config.enable_adaptive_routing {
            "Enabled".green()
        } else {
            "Disabled".dimmed()
        }
    );
    println!(
        "{:<25} {}",
        "Hierarchical Reasoning",
        if config.enable_hierarchical_reasoning {
            "Enabled".green()
        } else {
            "Disabled".dimmed()
        }
    );
    println!(
        "{:<25} {}",
        "System 2 Threshold",
        config.system2_threshold.to_string().yellow()
    );
    println!(
        "{:<25} {}",
        "System 3 Threshold",
        config.system3_threshold.to_string().yellow()
    );
    println!(
        "{:<25} {}",
        "Distributed Harness",
        if config.distributed_harness.is_enabled() {
            let backend = config
                .distributed_harness
                .backend
                .as_deref()
                .unwrap_or("redis");
            format!("Enabled ({})", backend).green().to_string()
        } else {
            "Disabled".dimmed().to_string()
        }
    );
    if config.distributed_harness.is_enabled() {
        println!(
            "{:<25} {}",
            "Distributed Node ID",
            config
                .distributed_harness
                .node_id
                .as_deref()
                .unwrap_or("<missing>")
        );
    }

    println!();
    println!(
        "{}: Configuration is loaded from config files, .env, and environment variables.",
        "Note".italic().dimmed()
    );

    Ok(())
}
