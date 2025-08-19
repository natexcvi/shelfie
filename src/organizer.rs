use anyhow::{Result, anyhow};
use colored::*;
use dialoguer::Confirm;
use rig::client::CompletionClient;
use std::path::{Path, PathBuf};

use crate::{
    ai_structs::*,
    file_analyzer::AnalyzedFile,
    providers::LLMProvider,
};

pub struct FileOrganizer {
    provider: LLMProvider,
    files: Vec<AnalyzedFile>,
    base_path: PathBuf,
}

impl FileOrganizer {
    pub fn new(provider: LLMProvider, files: Vec<AnalyzedFile>, base_path: PathBuf) -> Self {
        Self {
            provider,
            files,
            base_path,
        }
    }
    
    pub async fn analyze_and_organize(&self) -> Result<()> {
        println!("\n{}", "Step 1: Analyzing file names...".green().bold());
        let mut files_to_rename = Vec::new();
        
        for file in &self.files {
            print!("Analyzing: {} ", file.path.display());
            
            let is_indicative = self.analyze_filename(&file).await?;
            
            if !is_indicative {
                println!("{}", "✗ Non-indicative".red());
                files_to_rename.push(file.clone());
            } else {
                println!("{}", "✓ Indicative".green());
            }
        }
        
        println!("\n{}", "Step 2: Suggesting new names for non-indicative files...".green().bold());
        let mut file_infos = Vec::new();
        
        for file in &files_to_rename {
            print!("Generating name for: {} ", file.path.display());
            let new_name = self.suggest_filename(&file).await?;
            println!("→ {}", new_name.green());
            
            let relative_path = file.path.strip_prefix(&self.base_path)
                .unwrap_or(&file.path)
                .to_string_lossy()
                .to_string();
            
            file_infos.push(FileInfo {
                path: relative_path,
                suggested_name: Some(new_name),
                file_type: format!("{:?}", file.extension),
                description: file.get_content_preview(),
            });
        }
        
        for file in &self.files {
            if !files_to_rename.iter().any(|f| f.path == file.path) {
                let relative_path = file.path.strip_prefix(&self.base_path)
                    .unwrap_or(&file.path)
                    .to_string_lossy()
                    .to_string();
                
                file_infos.push(FileInfo {
                    path: relative_path,
                    suggested_name: None,
                    file_type: format!("{:?}", file.extension),
                    description: file.get_content_preview(),
                });
            }
        }
        
        println!("\n{}", "Step 3: Creating optimal directory structure...".green().bold());
        let structure = self.create_directory_structure(&file_infos).await?;
        
        println!("\n{}", "Proposed Organization Plan:".cyan().bold());
        self.print_structure(&structure)?;
        
        let confirm = Confirm::new()
            .with_prompt("Do you want to proceed with this organization?")
            .interact()?;
        
        if confirm {
            println!("\n{}", "Step 4: Executing reorganization...".green().bold());
            self.execute_reorganization(&structure).await?;
            println!("{}", "✓ Organization complete!".green().bold());
        } else {
            println!("{}", "Organization cancelled.".yellow());
        }
        
        Ok(())
    }
    
    async fn analyze_filename(&self, file: &AnalyzedFile) -> Result<bool> {
        let prompt = format!(
            "Analyze if the filename '{}' is indicative of its content. \
            File extension: {:?}. \
            Content preview: {}. \
            Return true if the name clearly indicates what the file contains, false otherwise.",
            file.name,
            file.extension,
            file.get_content_preview()
        );
        
        let analysis = self.extract_with_prompt::<FileNameAnalysis>(&prompt).await?;
        Ok(analysis.is_indicative)
    }
    
    async fn suggest_filename(&self, file: &AnalyzedFile) -> Result<String> {
        let prompt = format!(
            "Based on this file's content, suggest a better, more descriptive filename. \
            Current name: '{}'. \
            Extension: {:?}. \
            Content: {}. \
            Provide a clear, descriptive name without the extension.",
            file.name,
            file.extension,
            file.get_content_preview()
        );
        
        let suggestion = self.extract_with_prompt::<SuggestedFileName>(&prompt).await?;
        Ok(suggestion.new_name)
    }
    
    async fn create_directory_structure(&self, files: &[FileInfo]) -> Result<DirectoryStructure> {
        let mut file_descriptions = String::new();
        for (i, file) in files.iter().enumerate() {
            // Extract just the filename for the AI prompt
            let file_path = PathBuf::from(&file.path);
            let relative_path = file_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&file.path);
            
            file_descriptions.push_str(&format!(
                "{}. File: {} (Type: {}) - {}\n",
                i + 1,
                relative_path,
                file.file_type,
                file.description.chars().take(100).collect::<String>()
            ));
        }
        
        let prompt = format!(
            "Create an optimal directory structure for organizing these files. \
            You are organizing files within a single directory.
            
            Files to organize:
            {}
            
            Please provide:
            1. A list of directory paths that should be created (e.g., ['documents', 'images', 'code/python']). Use relative paths only, no absolute paths.
            2. File placements showing where each file should go with new names. Use relative directory paths only.
            
            Group related files together and create meaningful directory names. \
            Do NOT include the parent directory name in your structure.",
            file_descriptions
        );
        
        self.extract_with_prompt::<DirectoryStructure>(&prompt).await
    }
    
    async fn extract_with_prompt<T>(&self, prompt: &str) -> Result<T>
    where
        T: schemars::JsonSchema + for<'a> serde::Deserialize<'a> + serde::Serialize + Send + Sync + 'static,
    {
        let client = self.provider.get_openai_client()?;
        let extractor = client.extractor::<T>(self.provider.get_model_name()).build();
        extractor.extract(prompt).await
            .map_err(|e| anyhow!("Extraction failed: {}", e))
    }
    
    fn print_structure(&self, structure: &DirectoryStructure) -> Result<()> {
        println!("\n{}", "Directory Structure:".cyan());
        for dir in &structure.directories {
            println!("  {}/", dir.blue());
        }
        
        println!("\n{}", "File Movements:".cyan());
        for placement in &structure.file_placements {
            let from = PathBuf::from(&placement.original_path);
            let to_dir = PathBuf::from(&placement.new_directory);
            let to_file = to_dir.join(&placement.new_name);
            
            println!("  {} → {}", 
                from.display().to_string().yellow(),
                to_file.display().to_string().green()
            );
            println!("    {}", placement.reasoning.dimmed());
        }
        
        Ok(())
    }
    
    async fn execute_reorganization(&self, structure: &DirectoryStructure) -> Result<()> {
        // Create all directories first
        for dir_path in &structure.directories {
            let full_path = self.base_path.join(dir_path);
            std::fs::create_dir_all(&full_path)?;
            println!("  {} Created directory: {}", 
                "✓".green(),
                full_path.display()
            );
        }
        
        // Move files to their new locations
        for placement in &structure.file_placements {
            let from = self.base_path.join(&placement.original_path);
            let to_dir = self.base_path.join(&placement.new_directory);
            let to_file = to_dir.join(&placement.new_name);
            
            if from.exists() {
                std::fs::create_dir_all(&to_dir)?;
                std::fs::rename(&from, &to_file)
                    .or_else(|_| -> Result<(), std::io::Error> {
                        std::fs::copy(&from, &to_file)?;
                        std::fs::remove_file(&from)?;
                        Ok(())
                    })?;
                println!("  {} Moved: {} → {}", 
                    "✓".green(),
                    from.file_name().unwrap_or_default().to_string_lossy(),
                    to_file.display()
                );
            }
        }
        
        Ok(())
    }
}