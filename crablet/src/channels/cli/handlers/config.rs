use anyhow::Result;
use crate::config::Config;

pub fn handle_config(config: &Config) -> Result<()> {
    println!("Current Configuration:");
    println!("  Database: {}", config.database_url);
    println!("  Skills Dir: {:?}", config.skills_dir);
    println!("  Model: {}", config.model_name);
    println!("  Log Level: {}", config.log_level);
    Ok(())
}
