use anyhow::Result;
use directories::ProjectDirs;
use std::fs;

pub async fn init_environment() -> Result<()> {
    println!("Initializing Crablet environment...");

    if let Some(proj_dirs) = ProjectDirs::from("com", "crablet", "crablet") {
        // Fallback to ~/.config/crablet if system path fails (macOS sandbox issue)
        let config_dir = if fs::create_dir_all(proj_dirs.config_dir()).is_err() {
            let home = directories::UserDirs::new().ok_or_else(|| anyhow::anyhow!("Home dir not found"))?;
            home.home_dir().join(".config").join("crablet")
        } else {
            proj_dirs.config_dir().to_path_buf()
        };
        
        let data_dir = if fs::create_dir_all(proj_dirs.data_dir()).is_err() {
             let home = directories::UserDirs::new().ok_or_else(|| anyhow::anyhow!("Home dir not found"))?;
             home.home_dir().join(".local").join("share").join("crablet")
        } else {
             proj_dirs.data_dir().to_path_buf()
        };

        // 1. Create Config Directory
        if !config_dir.exists() {
            println!("Creating config directory: {:?}", config_dir);
            fs::create_dir_all(&config_dir)?;
        }

        // 2. Create Default Config File
        let config_path = config_dir.join("config.toml");
        if !config_path.exists() {
            println!("Creating default config: {:?}", config_path);
            let default_config = r#"
database_url = "sqlite:crablet.db?mode=rwc"
# skills_dir = "skills" # Defaults to ./skills or XDG data dir
model_name = "gpt-4o-mini"
log_level = "info"
"#;
            fs::write(config_path, default_config)?;
        } else {
            println!("Config file already exists: {:?}", config_path);
        }

        // 3. Create Data Directory (for Skills)
        if !data_dir.exists() {
            println!("Creating data directory: {:?}", data_dir);
            fs::create_dir_all(&data_dir)?;
        }

        let skills_dir = data_dir.join("skills");
        if !skills_dir.exists() {
            println!("Creating skills directory: {:?}", skills_dir);
            fs::create_dir_all(&skills_dir)?;
            
            // Create a sample skill?
            let hello_skill = skills_dir.join("hello");
            fs::create_dir_all(&hello_skill)?;
            fs::write(hello_skill.join("skill.yaml"), r#"
name: hello
description: A built-in hello world skill
version: 1.0.0
entrypoint: echo "Hello from global skill!"
parameters: {}
"#)?;
        }
        
        // 4. Add to PATH in .zshrc
        if let Some(user_dirs) = directories::UserDirs::new() {
            let home_dir = user_dirs.home_dir();
            let zshrc_path = home_dir.join(".zshrc");
            let cargo_bin_path = "$HOME/.cargo/bin";
            let export_line = format!(r#"export PATH="{}:$PATH""#, cargo_bin_path);
            
            // Check if file exists
            let mut content = if zshrc_path.exists() {
                std::fs::read_to_string(&zshrc_path)?
            } else {
                String::new()
            };

            if !content.contains(cargo_bin_path) {
                println!("Adding cargo bin to PATH in {:?}", zshrc_path);
                use std::fmt::Write;
                if !content.ends_with('\n') && !content.is_empty() {
                    writeln!(content)?;
                }
                writeln!(content, "\n# Added by Crablet init")?;
                writeln!(content, "{}", export_line)?;
                std::fs::write(zshrc_path, content)?;
                println!("Please restart your terminal or run 'source ~/.zshrc' for changes to take effect.");
            } else {
                println!("PATH already configured in {:?}", zshrc_path);
            }
        }

        println!("Initialization complete! You can now run 'crablet chat'.");
    } else {
        println!("Error: Could not determine home directory.");
    }

    Ok(())
}
