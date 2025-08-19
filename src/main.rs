mod ai_structs;
mod file_analyzer;
mod organizer;
mod providers;
mod utils;

use anyhow::Result;
use clap::{Arg, Command};
use colored::*;
use std::path::PathBuf;

use crate::{
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
        .get_matches();

    let target_dir = PathBuf::from(matches.get_one::<String>("directory").unwrap());
    
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
