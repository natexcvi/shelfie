mod ai_structs;
mod config;
mod file_analyzer;
mod organizer;
mod providers;
mod utils;

use anyhow::Result;
use clap::{Arg, Command};
use colored::*;
use std::path::PathBuf;

use crate::{
    config::Config,
    organizer::FileOrganizer,
    providers::LLMProvider,
    utils::{walk_directory, print_tree},
};

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("fs-organiser")
        .version("0.1.0")
        .author("AI File Organizer")
        .about("Organize your files using AI - analyzes content and creates logical directory structures")
        .subcommand_required(false)
        .subcommand(
            Command::new("organize")
                .about("Organize files in a directory")
                .arg(
                    Arg::new("directory")
                        .help("Target directory to organize")
                        .required(true)
                        .index(1)
                )
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .help("Show what would be done without making changes")
                        .action(clap::ArgAction::SetTrue)
                )
                .arg(
                    Arg::new("show-tree")
                        .long("show-tree")
                        .help("Show current directory tree")
                        .action(clap::ArgAction::SetTrue)
                )
        )
        .subcommand(
            Command::new("config")
                .about("Configuration management")
                .subcommand(
                    Command::new("edit")
                        .about("Edit the configuration interactively")
                )
                .subcommand(
                    Command::new("show")
                        .about("Show current configuration")
                )
                .subcommand(
                    Command::new("reset")
                        .about("Reset configuration (will prompt for new settings)")
                )
        )
        .arg(
            Arg::new("directory")
                .help("Target directory to organize (default mode)")
                .index(1)
        )
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .help("Show what would be done without making changes")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("show-tree")
                .long("show-tree")
                .help("Show current directory tree")
                .action(clap::ArgAction::SetTrue)
        )
        .get_matches();

    match matches.subcommand() {
        Some(("organize", sub_matches)) => {
            let target_dir = PathBuf::from(sub_matches.get_one::<String>("directory").unwrap());
            run_organize_command(target_dir, sub_matches).await?;
        }
        Some(("config", sub_matches)) => {
            run_config_command(sub_matches).await?;
        }
        None => {
            // Default mode - organize if directory is provided
            if let Some(directory) = matches.get_one::<String>("directory") {
                let target_dir = PathBuf::from(directory);
                run_organize_command(target_dir, &matches).await?;
            } else {
                println!("{}", "🤖 AI File Organizer".cyan().bold());
                println!("Use 'fs-organiser --help' for usage information");
                println!("Quick start: fs-organiser <directory>");
            }
        }
        _ => unreachable!(),
    }
    
    Ok(())
}

async fn run_organize_command(target_dir: PathBuf, matches: &clap::ArgMatches) -> Result<()> {
    if !target_dir.exists() {
        eprintln!("{}: Directory does not exist: {}", 
            "Error".red().bold(), 
            target_dir.display()
        );
        std::process::exit(1);
    }
    
    if !target_dir.is_dir() {
        eprintln!("{}: Path is not a directory: {}", 
            "Error".red().bold(), 
            target_dir.display()
        );
        std::process::exit(1);
    }

    println!("{}", "🤖 AI File Organizer".cyan().bold());
    println!("Target directory: {}\n", target_dir.display().to_string().yellow());

    if matches.get_flag("show-tree") {
        println!("{}", "Current Directory Structure:".green().bold());
        print_tree(&target_dir, "", true);
        println!();
    }

    match run_organizer(target_dir).await {
        Ok(_) => {
            println!("\n{}", "🎉 File organization completed successfully!".green().bold());
        }
        Err(e) => {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
            
            if e.to_string().contains("API_KEY") {
                eprintln!("\n{}", "💡 Tip: Make sure to set your API keys:".yellow());
                eprintln!("  export OPENAI_API_KEY=your_key_here");
                eprintln!("  export ANTHROPIC_API_KEY=your_key_here");
            }
            
            if e.to_string().contains("Ollama") {
                eprintln!("\n{}", "💡 Tip: For Ollama, make sure it's running:".yellow());
                eprintln!("  ollama serve");
                eprintln!("  ollama pull llama2  # or another model");
            }
            
            std::process::exit(1);
        }
    }
    
    Ok(())
}

async fn run_config_command(matches: &clap::ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("edit", _)) => {
            config_edit().await?;
        }
        Some(("show", _)) => {
            config_show().await?;
        }
        Some(("reset", _)) => {
            config_reset().await?;
        }
        None => {
            println!("{}", "Configuration Management".cyan().bold());
            println!("Available commands:");
            println!("  edit  - Edit configuration interactively");
            println!("  show  - Show current configuration");
            println!("  reset - Reset configuration");
            println!("\nUse 'fs-organiser config --help' for more information");
        }
        _ => unreachable!(),
    }
    
    Ok(())
}

async fn config_edit() -> Result<()> {
    println!("{}", "🔧 Configuration Editor".cyan().bold());
    
    // Force a new provider selection
    let provider = LLMProvider::new_interactive().await?;
    
    let config = Config {
        provider: provider.get_provider().clone(),
        model_name: provider.get_model_name().to_string(),
    };
    
    config.save()?;
    println!("{}", "✅ Configuration updated successfully!".green().bold());
    
    Ok(())
}

async fn config_show() -> Result<()> {
    println!("{}", "📋 Current Configuration".cyan().bold());
    
    match Config::load()? {
        Some(config) => {
            println!("Provider: {}", format!("{:?}", config.provider).green());
            println!("Model: {}", config.model_name.green());
            
            let config_path = Config::get_config_file_path()?;
            println!("Config file: {}", config_path.display().to_string().yellow());
        }
        None => {
            println!("{}", "No configuration found. Run 'fs-organiser config edit' to create one.".yellow());
        }
    }
    
    Ok(())
}

async fn config_reset() -> Result<()> {
    use dialoguer::{theme::ColorfulTheme, Confirm};
    
    let config_path = Config::get_config_file_path()?;
    
    if !config_path.exists() {
        println!("{}", "No configuration file found.".yellow());
        return Ok(());
    }
    
    let confirmed = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Are you sure you want to reset the configuration?")
        .default(false)
        .interact()?;
    
    if confirmed {
        std::fs::remove_file(&config_path)?;
        println!("{}", "✅ Configuration reset successfully!".green().bold());
        println!("Next time you run the organizer, you'll be prompted to select a provider and model.");
    } else {
        println!("Configuration reset cancelled.");
    }
    
    Ok(())
}

async fn run_organizer(target_dir: PathBuf) -> Result<()> {
    println!("{}", "Scanning directory...".green());
    let files = walk_directory(&target_dir)?;
    
    if files.is_empty() {
        println!("{}", "No analyzable files found in the directory.".yellow());
        return Ok(());
    }
    
    println!("Found {} analyzable files\n", files.len().to_string().green());
    
    println!("{}", "Setting up AI provider...".green());
    let provider = LLMProvider::new().await?;
    
    println!("\nUsing: {} with model {}", 
        format!("{:?}", provider.get_provider()).cyan(),
        provider.get_model_name().yellow()
    );
    
    let organizer = FileOrganizer::new(provider, files, target_dir);
    organizer.analyze_and_organize().await?;
    
    Ok(())
}
