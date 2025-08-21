use anyhow::{Context, Result};
use colored::*;
use dialoguer::Confirm;

use indicatif::{ProgressBar, ProgressStyle};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{sync::Semaphore, task::JoinSet};

use walkdir::WalkDir;

use crate::{
    batch_processor::BatchProcessor,
    database::{Database, DB_NAME},
    file_analyzer::{AnalyzedFile, FileContent},
    models::{
        CabinetPlan, EnrichedDirectory, EnrichedFile, FileMovement, OrganizationPlan,
        ProcessingItem, SampledItem, ShelfPlan,
    },
    providers::LLMProvider,
};

pub struct FileOrganizer {
    provider: LLMProvider,
    base_path: PathBuf,
    database: Arc<Database>,
}

impl FileOrganizer {
    pub fn new(provider: LLMProvider, base_path: PathBuf) -> Result<Self> {
        let database = Arc::new(Database::open_or_create(&base_path)?);
        Ok(Self {
            provider,
            base_path,
            database,
        })
    }

    pub async fn analyze_and_organize(&self, max_depth: usize, auto_confirm: bool) -> Result<()> {
        // Check if database exists for resuming
        if Database::exists(&self.base_path) {
            println!(
                "📁 Found existing organization database - processing new/modified items only"
            );
        }

        // Step 1: Scan directory and collect items
        println!("\n{}", "Step 1: Scanning directory...".green().bold());
        let items = self.collect_items(max_depth).await?;

        if items.is_empty() {
            println!("✓ All items already processed or no new items found");
            return Ok(());
        }

        println!("✓ Found {} items to process", items.len());

        // Step 2: Process with AI in batches
        println!("\n{}", "Step 2: Analyzing with AI...".green().bold());
        let batch_processor = BatchProcessor::new(self.provider.clone(), self.base_path.clone());

        batch_processor.process_items_sequentially(items).await?;

        // Step 3: Generate organization plan
        println!(
            "\n{}",
            "Step 3: Creating organization plan...".green().bold()
        );
        let plan = self.create_organization_plan()?;

        println!("\n{}", "Proposed Organization Plan:".cyan().bold());
        self.print_plan(&plan)?;

        let confirm = if auto_confirm {
            println!("{}", "Auto-confirming organization plan...".yellow());
            true
        } else {
            Confirm::new()
                .with_prompt("Do you want to proceed with this organization?")
                .interact()?
        };

        if confirm {
            println!("\n{}", "Step 4: Executing reorganization...".green().bold());
            self.execute_plan(&plan).await?;
            println!("{}", "✓ Organization complete!".green().bold());
        } else {
            println!("{}", "Organization cancelled.".yellow());
        }

        Ok(())
    }

    async fn collect_items(&self, max_depth: usize) -> Result<Vec<ProcessingItem>> {
        let processed_paths = self.database.get_processed_paths().unwrap_or_default();
        let mut join_set = JoinSet::new();
        const MAX_CONCURRENCY: usize = 10;

        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENCY));

        let progress_bar = ProgressBar::new_spinner();
        progress_bar.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        progress_bar.set_message("Scanning files...");
        progress_bar.enable_steady_tick(Duration::from_millis(200));

        for entry in WalkDir::new(&self.base_path).max_depth(max_depth) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path().to_path_buf();

            // Skip if already processed
            let path_str = path.to_string_lossy().to_string();
            if processed_paths.contains(&path_str) {
                continue;
            }

            // Skip hidden files and the database file
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.') || name_str == DB_NAME {
                    continue;
                }
            }

            // Skip the base path itself
            if path == self.base_path {
                continue;
            }

            let semaphore = Arc::clone(&semaphore);
            if path.is_file() {
                join_set.spawn(async move {
                    let _permit = semaphore.acquire().await?;
                    Self::process_file_static(&path).await
                });
            } else if path.is_dir() {
                join_set.spawn(async move {
                    let _permit = semaphore.acquire().await?;
                    Self::process_directory_static(&path).await
                });
            }
        }

        progress_bar.set_style(ProgressStyle::default_bar().template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
        )?);
        progress_bar.set_length(join_set.len() as u64);

        let mut items = Vec::new();

        while let Some(result) = join_set.join_next().await {
            items.push(result??);
            progress_bar.inc(1);
        }

        progress_bar.finish_with_message("✓ Scan complete");
        Ok(items)
    }

    async fn process_file_static(path: &std::path::Path) -> Result<ProcessingItem> {
        // eprintln!("Processing file: {:?}", path);
        let analyzed = AnalyzedFile::new(path.to_path_buf())
            .await
            .context("Failed to analyze file")?;

        let enriched = EnrichedFile {
            path: path.to_path_buf(),
            name: analyzed.name.clone(),
            extension: analyzed.extension.clone(),
            file_type: analyzed.get_type_description(),
            size: analyzed.size,
            content_preview: if let FileContent::Preview(content) = analyzed.content {
                Some(content)
            } else {
                None
            },
        };

        Ok(ProcessingItem::File(enriched))
    }

    async fn process_directory_static(path: &std::path::Path) -> Result<ProcessingItem> {
        // eprintln!("Processing directory: {:?}", path);
        const SAMPLE_SIZE: usize = 20;

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut sampled_items = Vec::new();
        let mut count = 0;

        if let Ok(mut entries) = tokio::fs::read_dir(path).await {
            while let Some(entry) = entries.next_entry().await.ok().flatten() {
                if count >= SAMPLE_SIZE {
                    break;
                }

                let entry_path = entry.path();
                if let Some(entry_name) = entry_path.file_name().and_then(|n| n.to_str()) {
                    if entry_name.starts_with('.') {
                        continue;
                    }

                    let is_file = entry_path.is_file();
                    let extension = if is_file {
                        entry_path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    };

                    sampled_items.push(SampledItem {
                        name: entry_name.to_string(),
                        is_file,
                        extension,
                    });

                    count += 1;
                }
            }
        }

        let enriched = EnrichedDirectory {
            path: path.to_path_buf(),
            name,
            sampled_items,
        };

        Ok(ProcessingItem::Directory(enriched))
    }

    fn create_organization_plan(&self) -> Result<OrganizationPlan> {
        let database = &self.database;
        let cabinets = database.list_cabinets()?;
        let shelves = database.list_shelves(None)?;
        let items = database.list_all_items()?;

        let mut cabinet_plans = Vec::new();

        for cabinet in &cabinets {
            let cabinet_shelves = shelves
                .iter()
                .filter(|s| s.cabinet_id == cabinet.id)
                .collect::<Vec<_>>();

            let mut shelf_plans = Vec::new();

            for shelf in cabinet_shelves {
                let item_count = items.iter().filter(|i| i.shelf_id == shelf.id).count();

                shelf_plans.push(ShelfPlan {
                    name: shelf.name.clone(),
                    description: shelf.description.clone(),
                    item_count,
                });
            }

            cabinet_plans.push(CabinetPlan {
                name: cabinet.name.clone(),
                description: cabinet.description.clone(),
                shelves: shelf_plans,
            });
        }

        let mut movements = Vec::new();

        for item in items {
            let shelf = shelves
                .iter()
                .find(|s| s.id == item.shelf_id)
                .context("Shelf not found for item")?;

            let cabinet = cabinets
                .iter()
                .find(|c| c.id == shelf.cabinet_id)
                .context("Cabinet not found for shelf")?;

            let from = PathBuf::from(&item.path);

            movements.push(FileMovement {
                from: from.clone(),
                to_cabinet: cabinet.name.clone(),
                to_shelf: shelf.name.clone(),
                new_name: item.suggested_name.clone(),
                reasoning: item.description.clone(),
            });
        }

        Ok(OrganizationPlan {
            cabinets: cabinet_plans,
            movements,
        })
    }

    fn print_plan(&self, plan: &OrganizationPlan) -> Result<()> {
        println!("\n{}", "Cabinet Structure:".cyan());

        for cabinet in &plan.cabinets {
            println!(
                "  🗄  {} - {}",
                cabinet.name.blue().bold(),
                cabinet.description
            );

            for shelf in &cabinet.shelves {
                println!(
                    "      📁 {} ({} items) - {}",
                    shelf.name.green(),
                    shelf.item_count,
                    shelf.description.dimmed()
                );
            }
        }

        println!("\n{}", "File Movements:".cyan());

        let display_limit = 20;
        let total = plan.movements.len();

        for (idx, movement) in plan.movements.iter().take(display_limit).enumerate() {
            let from_name = movement
                .from
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let default_name = from_name.to_string();
            let to_name = movement.new_name.as_ref().unwrap_or(&default_name);

            println!(
                "  {} → {}/{}/{}",
                from_name.yellow(),
                movement.to_cabinet.blue(),
                movement.to_shelf.green(),
                if movement.new_name.is_some() {
                    to_name.cyan().to_string()
                } else {
                    to_name.to_string()
                }
            );

            if idx < 5 || total <= display_limit {
                println!("    {}", movement.reasoning.dimmed());
            }
        }

        if total > display_limit {
            println!("  ... and {} more files", total - display_limit);
        }

        Ok(())
    }

    async fn execute_plan(&self, plan: &OrganizationPlan) -> Result<()> {
        let total_operations = plan.cabinets.len() + plan.movements.len();

        if total_operations == 0 {
            println!("{}", "No operations to perform.".yellow());
            return Ok(());
        }

        let pb = ProgressBar::new(total_operations as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
                )
                .unwrap()
                .progress_chars("#>-"),
        );

        // Create cabinet and shelf directories
        pb.set_message("Creating directory structure...");

        for cabinet in &plan.cabinets {
            let cabinet_path = self.base_path.join(&cabinet.name);
            tokio::fs::create_dir_all(&cabinet_path).await?;

            for shelf in &cabinet.shelves {
                let shelf_path = cabinet_path.join(&shelf.name);
                tokio::fs::create_dir_all(&shelf_path).await?;
            }

            pb.inc(1);
        }

        // Move files
        pb.set_message("Moving files...");

        for movement in &plan.movements {
            let to_dir = self
                .base_path
                .join(&movement.to_cabinet)
                .join(&movement.to_shelf);

            let file_name = if let Some(new_name) = &movement.new_name {
                new_name.clone()
            } else if let Some(name) = movement.from.file_name().and_then(|n| n.to_str()) {
                name.to_string()
            } else {
                "unknown".to_string()
            };

            // Add extension if present
            let final_name = if let Some(ext) = movement.from.extension() {
                format!("{}.{}", file_name, ext.to_string_lossy())
            } else {
                file_name
            };

            let to_file = to_dir.join(final_name);

            if movement.from.exists() {
                tokio::fs::create_dir_all(&to_dir).await?;

                // Try rename first, fall back to copy+delete
                tokio::fs::rename(&movement.from, &to_file).await.or_else(
                    |_| -> Result<(), std::io::Error> {
                        std::fs::copy(&movement.from, &to_file)?;
                        std::fs::remove_file(&movement.from)?;
                        Ok(())
                    },
                )?;

                pb.set_message(format!(
                    "Moved: {}",
                    movement
                        .from
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                ));
            }

            pb.inc(1);
        }

        pb.finish_with_message(format!("✓ Reorganized {} items", total_operations));

        Ok(())
    }
}
