use anyhow::{anyhow, Result};
use colored::*;
use dialoguer::Confirm;
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use rig::client::CompletionClient;
use std::path::PathBuf;

use crate::{
    ai_structs::*,
    file_analyzer::AnalyzedFile,
    providers::{LLMProvider, Provider},
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

        // Analyze filenames concurrently
        let analysis_results = self.analyze_filenames_concurrent().await?;

        let files_to_rename: Vec<_> = self
            .files
            .iter()
            .zip(analysis_results.iter())
            .filter_map(|(file, &is_indicative)| {
                if !is_indicative {
                    Some(file.clone())
                } else {
                    None
                }
            })
            .collect();

        println!(
            "\nAnalysis complete: {} files need renaming, {} files have good names",
            files_to_rename.len().to_string().red(),
            (self.files.len() - files_to_rename.len())
                .to_string()
                .green()
        );

        println!(
            "\n{}",
            "Step 2: Generating new names for files...".green().bold()
        );

        // Generate new names concurrently
        let new_names = if !files_to_rename.is_empty() {
            self.suggest_filenames_concurrent(&files_to_rename).await?
        } else {
            Vec::new()
        };

        // Build file info structures
        let mut file_infos = Vec::new();

        // Add files that need renaming
        for (file, new_name) in files_to_rename.iter().zip(new_names.iter()) {
            let relative_path = file
                .path
                .strip_prefix(&self.base_path)
                .unwrap_or(&file.path)
                .to_string_lossy()
                .to_string();

            file_infos.push(FileInfo {
                path: relative_path,
                suggested_name: Some(new_name.clone()),
                file_type: file.get_type_description(),
                description: file.get_content_preview(),
            });
        }

        // Add files that don't need renaming
        for file in &self.files {
            if !files_to_rename.iter().any(|f| f.path == file.path) {
                let relative_path = file
                    .path
                    .strip_prefix(&self.base_path)
                    .unwrap_or(&file.path)
                    .to_string_lossy()
                    .to_string();

                file_infos.push(FileInfo {
                    path: relative_path,
                    suggested_name: None,
                    file_type: file.get_type_description(),
                    description: file.get_content_preview(),
                });
            }
        }

        println!(
            "\n\n{}",
            "Step 3: Creating optimal directory structure..."
                .green()
                .bold()
        );
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

    async fn analyze_filenames_concurrent(&self) -> Result<Vec<bool>> {
        let progress_bar = ProgressBar::new(self.files.len() as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        progress_bar.set_message("Analyzing filenames...");

        let results = stream::iter(&self.files)
            .map(|file| {
                let progress_bar = progress_bar.clone();
                async move {
                    let result = self.analyze_filename(file).await;
                    progress_bar.inc(1);
                    result
                }
            })
            .buffer_unordered(10) // Limit to 10 concurrent requests
            .collect::<Vec<_>>()
            .await;

        progress_bar.finish_with_message("Filename analysis complete!");

        // Convert Results to Vec<bool>
        let mut final_results = Vec::new();
        for result in results {
            final_results.push(result?);
        }

        Ok(final_results)
    }

    async fn suggest_filenames_concurrent(&self, files: &[AnalyzedFile]) -> Result<Vec<String>> {
        if files.is_empty() {
            return Ok(Vec::new());
        }

        let progress_bar = ProgressBar::new(files.len() as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        progress_bar.set_message("Generating new names...");

        let results = stream::iter(files)
            .map(|file| {
                let progress_bar = progress_bar.clone();
                async move {
                    let result = self.suggest_filename(file).await;
                    progress_bar.inc(1);
                    result
                }
            })
            .buffer_unordered(10) // Limit to 10 concurrent requests
            .collect::<Vec<_>>()
            .await;

        progress_bar.finish_with_message("Name generation complete!");

        // Convert Results to Vec<String>
        let mut final_results = Vec::new();
        for result in results {
            final_results.push(result?);
        }

        Ok(final_results)
    }

    async fn analyze_filename(&self, file: &AnalyzedFile) -> Result<bool> {
        let relative_path = file
            .path
            .strip_prefix(&self.base_path)
            .unwrap_or(&file.path)
            .to_string_lossy()
            .to_string();

        let prompt = format!(
            "Analyze if the filename '{}' is indicative of its content. \
            File path: {}. \
            Detected file type: {}. \
            File extension: {:?}. \
            Content preview: {}. \
            Return true if the name clearly indicates what the file contains, false otherwise.",
            file.name,
            relative_path,
            file.get_type_description(),
            file.extension,
            file.get_content_preview()
        );

        let analysis = self
            .extract_with_prompt::<FileNameAnalysis>(&prompt)
            .await?;
        Ok(analysis.is_indicative)
    }

    async fn suggest_filename(&self, file: &AnalyzedFile) -> Result<String> {
        let prompt = format!(
            "Based on this file's content, suggest a better, more descriptive filename. \
            Current name: '{}'. \
            Detected file type: {}. \
            Extension: {:?}. \
            Content: {}. \
            Provide a clear, descriptive name without the extension.",
            file.name,
            file.get_type_description(),
            file.extension,
            file.get_content_preview()
        );

        let suggestion = self
            .extract_with_prompt::<SuggestedFileName>(&prompt)
            .await?;
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

        self.extract_with_prompt::<DirectoryStructure>(&prompt)
            .await
    }

    async fn extract_with_prompt<T>(&self, prompt: &str) -> Result<T>
    where
        T: schemars::JsonSchema
            + for<'a> serde::Deserialize<'a>
            + serde::Serialize
            + Send
            + Sync
            + 'static,
    {
        match self.provider.get_provider() {
            Provider::OpenAI => {
                let client = self.provider.get_openai_client()?;
                let extractor = client
                    .extractor::<T>(self.provider.get_model_name())
                    .build();
                extractor
                    .extract(prompt)
                    .await
                    .map_err(|e| anyhow!("Extraction failed: {}", e))
            }
            Provider::Anthropic => {
                let client = self.provider.get_anthropic_client()?;
                let extractor = client
                    .extractor::<T>(self.provider.get_model_name())
                    .build();
                extractor
                    .extract(prompt)
                    .await
                    .map_err(|e| anyhow!("Extraction failed: {}", e))
            }
            Provider::Ollama => {
                let client = self.provider.get_ollama_client()?;
                let extractor = client
                    .extractor::<T>(self.provider.get_model_name())
                    .build();
                extractor
                    .extract(prompt)
                    .await
                    .map_err(|e| anyhow!("Extraction failed: {}", e))
            }
        }
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

            println!(
                "  {} → {}",
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
            println!(
                "  {} Created directory: {}",
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
                std::fs::rename(&from, &to_file).or_else(|_| -> Result<(), std::io::Error> {
                    std::fs::copy(&from, &to_file)?;
                    std::fs::remove_file(&from)?;
                    Ok(())
                })?;
                println!(
                    "  {} Moved: {} → {}",
                    "✓".green(),
                    from.file_name().unwrap_or_default().to_string_lossy(),
                    to_file.display()
                );
            }
        }

        Ok(())
    }
}
