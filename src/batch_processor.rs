use anyhow::{Result, anyhow};
use chrono::Utc;
use crossbeam::channel::Receiver;
use indicatif::{ProgressBar, ProgressStyle};
use rig::client::CompletionClient;
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;
use std::collections::HashMap;
use std::sync::Arc;
use std::path::PathBuf;

use crate::{
    database::{Database, Item},
    file_analyzer::AnalyzedFile,
    models::*,
    providers::{LLMProvider, Provider},
};

pub struct BatchProcessor {
    provider: LLMProvider,
    database: Arc<Database>,
    base_path: PathBuf,
}

impl BatchProcessor {
    pub fn new(provider: LLMProvider, database: Arc<Database>, base_path: PathBuf) -> Self {
        Self {
            provider,
            database,
            base_path,
        }
    }
    
    pub async fn process_batches(
        &self,
        receiver: Receiver<ProcessingItem>,
        batch_size: usize,
        _num_workers: usize,
    ) -> Result<()> {
        let mut batch = Vec::new();
        let mut total_processed = 0;
        
        println!("\n🤖 Processing items with AI...");
        
        // Collect items into batches
        while let Ok(item) = receiver.recv() {
            batch.push(item);
            
            if batch.len() >= batch_size {
                self.process_single_batch(&batch).await?;
                total_processed += batch.len();
                batch.clear();
            }
        }
        
        // Process remaining items
        if !batch.is_empty() {
            self.process_single_batch(&batch).await?;
            total_processed += batch.len();
        }
        
        // Process items needing content read
        self.process_items_needing_content().await?;
        
        println!("✓ Processed {} items total", total_processed);
        
        Ok(())
    }
    
    async fn process_single_batch(&self, items: &[ProcessingItem]) -> Result<()> {
        // Load existing cabinets and shelves
        let cabinets = self.database.list_cabinets()?;
        let shelves = self.database.list_shelves(None)?;
        
        // Convert items to metadata for LLM
        let item_metadata: Vec<ItemMetadata> = items.iter().enumerate().map(|(idx, item)| {
            match item {
                ProcessingItem::File(file) => {
                    let name = file.name.clone();
                    let is_unclear = Self::is_filename_unclear(&name, &file.extension);
                    
                    ItemMetadata {
                        id: idx.to_string(),
                        name,
                        item_type: "file".to_string(),
                        extension: file.extension.clone(),
                        size_bytes: Some(file.size),
                        sampled_contents: None,
                        content_preview: if is_unclear { Some("[needs content read]".to_string()) } else { None },
                    }
                },
                ProcessingItem::Directory(dir) => {
                    let sampled_names: Vec<String> = dir.sampled_items.iter()
                        .map(|item| item.name.clone())
                        .collect();
                    
                    ItemMetadata {
                        id: idx.to_string(),
                        name: dir.name.clone(),
                        item_type: "directory".to_string(),
                        extension: None,
                        size_bytes: None,
                        sampled_contents: Some(sampled_names),
                        content_preview: None,
                    }
                }
            }
        }).collect();
        
        // Prepare LLM request
        let request = BatchAnalysisRequest {
            items: item_metadata,
            existing_cabinets: cabinets.iter().map(|c| CabinetInfo {
                id: c.id,
                name: c.name.clone(),
                description: c.description.clone(),
            }).collect(),
            existing_shelves: shelves.iter().map(|s| ShelfInfo {
                id: s.id,
                cabinet_id: s.cabinet_id,
                name: s.name.clone(),
                description: s.description.clone(),
            }).collect(),
        };
        
        // Call LLM for batch analysis
        let response = self.analyze_batch_with_llm(&request).await?;
        
        // Process response and update database
        self.store_batch_results(items, &response).await?;
        
        Ok(())
    }
    
    async fn analyze_batch_with_llm(&self, request: &BatchAnalysisRequest) -> Result<BatchAnalysisResponse> {
        let prompt = format!(
            "Analyze these files and directories for organization. \
            You have up to 10 cabinets (top-level containers) and up to 10 shelves per cabinet.\n\n\
            Existing Cabinets:\n{}\n\n\
            Existing Shelves:\n{}\n\n\
            Items to analyze:\n{}\n\n\
            For each item, provide:\n\
            1. A brief description (one sentence)\n\
            2. Whether the name needs improvement (suggest a better name if so)\n\
            3. For unclear files marked with '[needs content read]', set needs_content_read=true\n\
            4. For directories, determine if they're opaque (homogeneous content, generated files, etc.)\n\
            5. Assign to an existing or new cabinet and shelf\n\n\
            Guidelines:\n\
            - Group related items together\n\
            - Use existing cabinets/shelves when appropriate\n\
            - Create new ones only when necessary\n\
            - Keep names short and descriptive",
            self.format_cabinets(&request.existing_cabinets),
            self.format_shelves(&request.existing_shelves),
            self.format_items(&request.items)
        );
        
        self.extract_with_prompt::<BatchAnalysisResponse>(&prompt).await
    }
    
    async fn store_batch_results(&self, items: &[ProcessingItem], response: &BatchAnalysisResponse) -> Result<()> {
        let mut cabinet_cache: HashMap<String, i64> = HashMap::new();
        let mut shelf_cache: HashMap<(i64, String), i64> = HashMap::new();
        
        for (item, analysis) in items.iter().zip(response.items.iter()) {
            // Get or create cabinet
            let cabinet_id = match &analysis.cabinet {
                CabinetAssignment::Existing { id } => *id,
                CabinetAssignment::New { name, description } => {
                    if let Some(&id) = cabinet_cache.get(name) {
                        id
                    } else {
                        let id = self.database.create_cabinet(name, description)?;
                        cabinet_cache.insert(name.clone(), id);
                        id
                    }
                }
            };
            
            // Get or create shelf
            let shelf_id = match &analysis.shelf {
                ShelfAssignment::Existing { id } => *id,
                ShelfAssignment::New { name, description } => {
                    let key = (cabinet_id, name.clone());
                    if let Some(&id) = shelf_cache.get(&key) {
                        id
                    } else {
                        let id = self.database.create_shelf(cabinet_id, name, description)?;
                        shelf_cache.insert(key, id);
                        id
                    }
                }
            };
            
            // Create item record
            let (path, original_name, file_type) = match item {
                ProcessingItem::File(file) => (
                    file.path.to_string_lossy().to_string(),
                    file.name.clone(),
                    file.file_type.clone(),
                ),
                ProcessingItem::Directory(dir) => (
                    dir.path.to_string_lossy().to_string(),
                    dir.name.clone(),
                    "directory".to_string(),
                ),
            };
            
            let db_item = Item {
                id: None,
                shelf_id,
                path,
                original_name,
                suggested_name: analysis.suggested_name.clone(),
                description: analysis.description.clone(),
                file_type,
                is_opaque_dir: analysis.is_opaque_directory,
                needs_content_read: analysis.needs_content_read,
                processed_at: Utc::now(),
            };
            
            self.database.insert_item(&db_item)?;
        }
        
        Ok(())
    }
    
    async fn process_items_needing_content(&self) -> Result<()> {
        let items_needing_content = self.database.list_items_needing_content()?;
        
        if items_needing_content.is_empty() {
            return Ok(());
        }
        
        println!("\n📖 Reading content for {} unclear files...", items_needing_content.len());
        
        let progress_bar = ProgressBar::new(items_needing_content.len() as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );
        
        for item in items_needing_content {
            progress_bar.set_message(format!("Reading: {}", item.original_name));
            
            // Read file content
            let path = PathBuf::from(&item.path);
            if let Ok(analyzed) = AnalyzedFile::new(path) {
                let content_preview = analyzed.get_content_preview();
                
                // Analyze with content
                let prompt = format!(
                    "Based on this file's content, provide:\n\
                    1. A clear, descriptive one-sentence summary\n\
                    2. A better filename (without extension)\n\n\
                    File: {}\n\
                    Type: {}\n\
                    Content preview:\n{}\n",
                    item.original_name,
                    item.file_type,
                    content_preview
                );
                
                #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
                #[schemars(deny_unknown_fields)]
                struct ContentAnalysis {
                    description: String,
                    suggested_name: String,
                }
                
                if let Ok(analysis) = self.extract_with_prompt::<ContentAnalysis>(&prompt).await {
                    self.database.update_item_content(
                        item.id.unwrap(),
                        &analysis.description,
                        &analysis.suggested_name
                    )?;
                }
            }
            
            progress_bar.inc(1);
        }
        
        progress_bar.finish_with_message("✓ Content analysis complete");
        
        Ok(())
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
    
    fn is_filename_unclear(name: &str, _extension: &Option<String>) -> bool {
        // Check if filename is too generic or unclear
        const UNCLEAR_PATTERNS: &[&str] = &[
            "doc", "file", "document", "data", "temp", "tmp", "test",
            "untitled", "new", "copy", "backup", "old", "final", "draft"
        ];
        
        let name_lower = name.to_lowercase();
        
        // Check if name is just numbers or very short
        if name.len() <= 3 || name.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
        
        // Check if name matches unclear patterns
        if UNCLEAR_PATTERNS.iter().any(|&pattern| {
            name_lower.starts_with(pattern) || name_lower == pattern
        }) {
            return true;
        }
        
        // Check if name is like "file1", "doc123", etc.
        if name.len() < 10 && name.chars().any(|c| c.is_ascii_digit()) {
            let alpha_count = name.chars().filter(|c| c.is_alphabetic()).count();
            let digit_count = name.chars().filter(|c| c.is_ascii_digit()).count();
            if alpha_count <= 4 && digit_count > 0 {
                return true;
            }
        }
        
        false
    }
    
    fn format_cabinets(&self, cabinets: &[CabinetInfo]) -> String {
        if cabinets.is_empty() {
            "None yet".to_string()
        } else {
            cabinets.iter()
                .map(|c| format!("- {} (ID: {}): {}", c.name, c.id, c.description))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
    
    fn format_shelves(&self, shelves: &[ShelfInfo]) -> String {
        if shelves.is_empty() {
            "None yet".to_string()
        } else {
            shelves.iter()
                .map(|s| format!("- Cabinet {}, {} (ID: {}): {}", 
                    s.cabinet_id, s.name, s.id, s.description))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
    
    fn format_items(&self, items: &[ItemMetadata]) -> String {
        items.iter()
            .map(|item| {
                let mut desc = format!("{}: {} ({})", item.id, item.name, item.item_type);
                if let Some(ext) = &item.extension {
                    desc.push_str(&format!(".{}", ext));
                }
                if let Some(size) = item.size_bytes {
                    desc.push_str(&format!(", {} bytes", size));
                }
                if let Some(sampled) = &item.sampled_contents {
                    let sample = sampled.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
                    desc.push_str(&format!(", contains: [{}...]", sample));
                }
                if let Some(preview) = &item.content_preview {
                    desc.push_str(&format!(", {}", preview));
                }
                desc
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}