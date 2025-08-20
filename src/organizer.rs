use anyhow::{Result, Context};
use colored::*;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::sync::Arc;

use crate::{
    batch_processor::BatchProcessor,
    concurrent_processor::ConcurrentProcessor,
    database::Database,
    models::{OrganizationPlan, CabinetPlan, ShelfPlan, FileMovement},
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

    pub async fn analyze_and_organize(&self, max_depth: usize) -> Result<()> {
        // Check if database exists for resuming
        if Database::exists(&self.base_path) {
            println!("📁 Found existing organization database - processing new/modified items only");
        }
        
        // Step 1: Initialize concurrent processor
        println!("\n{}", "Step 1: Initializing concurrent processing...".green().bold());
        let processor = ConcurrentProcessor::new(&self.base_path)?;
        processor.initialize_queue(&self.base_path, max_depth)?;
        
        // Step 2: Run file analysis workers
        println!("\n{}", "Step 2: Analyzing files and directories...".green().bold());
        processor.run_file_analysis_workers(10)?;
        
        // Step 3: Process batches with LLM
        println!("\n{}", "Step 3: Organizing with AI...".green().bold());
        let batch_processor = BatchProcessor::new(
            self.provider.clone(),
            Arc::clone(&self.database),
            self.base_path.clone(),
        );
        
        batch_processor.process_batches(
            processor.get_receiver(),
            100,  // batch size
            10,   // num workers
        ).await?;
        
        // Step 4: Generate organization plan
        println!("\n{}", "Step 4: Creating organization plan...".green().bold());
        let plan = self.create_organization_plan()?;
        
        println!("\n{}", "Proposed Organization Plan:".cyan().bold());
        self.print_plan(&plan)?;
        
        let confirm = Confirm::new()
            .with_prompt("Do you want to proceed with this organization?")
            .interact()?;
        
        if confirm {
            println!("\n{}", "Step 5: Executing reorganization...".green().bold());
            self.execute_plan(&plan).await?;
            println!("{}", "✓ Organization complete!".green().bold());
        } else {
            println!("{}", "Organization cancelled.".yellow());
        }
        
        Ok(())
    }
    
    fn create_organization_plan(&self) -> Result<OrganizationPlan> {
        let cabinets = self.database.list_cabinets()?;
        let shelves = self.database.list_shelves(None)?;
        let items = self.database.list_all_items()?;
        
        let mut cabinet_plans = Vec::new();
        
        for cabinet in &cabinets {
            let cabinet_shelves = shelves.iter()
                .filter(|s| s.cabinet_id == cabinet.id)
                .collect::<Vec<_>>();
            
            let mut shelf_plans = Vec::new();
            
            for shelf in cabinet_shelves {
                let item_count = items.iter()
                    .filter(|i| i.shelf_id == shelf.id)
                    .count();
                
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
            let shelf = shelves.iter()
                .find(|s| s.id == item.shelf_id)
                .context("Shelf not found for item")?;
            
            let cabinet = cabinets.iter()
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
            println!("  🗄  {} - {}", cabinet.name.blue().bold(), cabinet.description);
            
            for shelf in &cabinet.shelves {
                println!("      📁 {} ({} items) - {}", 
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
            let from_name = movement.from.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            
            let default_name = from_name.to_string();
            let to_name = movement.new_name.as_ref()
                .unwrap_or(&default_name);
            
            println!("  {} → {}/{}/{}", 
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
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("#>-")
        );
        
        // Create cabinet and shelf directories
        pb.set_message("Creating directory structure...");
        
        for cabinet in &plan.cabinets {
            let cabinet_path = self.base_path.join(&cabinet.name);
            std::fs::create_dir_all(&cabinet_path)?;
            
            for shelf in &cabinet.shelves {
                let shelf_path = cabinet_path.join(&shelf.name);
                std::fs::create_dir_all(&shelf_path)?;
            }
            
            pb.inc(1);
        }
        
        // Move files
        pb.set_message("Moving files...");
        
        for movement in &plan.movements {
            let to_dir = self.base_path
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
                std::fs::create_dir_all(&to_dir)?;
                
                // Try rename first, fall back to copy+delete
                std::fs::rename(&movement.from, &to_file)
                    .or_else(|_| -> Result<(), std::io::Error> {
                        std::fs::copy(&movement.from, &to_file)?;
                        std::fs::remove_file(&movement.from)?;
                        Ok(())
                    })?;
                
                pb.set_message(format!("Moved: {}", 
                    movement.from.file_name()
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