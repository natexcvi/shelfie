use anyhow::{Result, anyhow};
use chrono::Utc;

use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::{collections::HashMap, time::Duration};

use crate::{
    database::{Database, Item},
    models::*,
    providers::LLMProvider,
};

pub struct BatchProcessor {
    provider: LLMProvider,
    base_path: PathBuf,
}

impl BatchProcessor {
    pub fn new(provider: LLMProvider, base_path: PathBuf) -> Self {
        Self {
            provider,
            base_path,
        }
    }

    pub async fn process_items_sequentially(&self, items: Vec<ProcessingItem>) -> Result<()> {
        let database = Database::open_or_create(&self.base_path)?;

        // Process in batches
        let batch_size = 10;
        let total_batches = (items.len() + batch_size - 1) / batch_size;

        println!(
            "🤖 Processing {} items in {} batches",
            items.len(),
            total_batches
        );

        let progress_bar = ProgressBar::new(total_batches as u64);
        progress_bar.enable_steady_tick(Duration::from_millis(200));
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
                )
                .unwrap(),
        );

        for batch in items.chunks(batch_size) {
            Self::process_single_batch_static(&self.provider, &database, batch.to_vec()).await?;
            progress_bar.inc(1);
        }

        progress_bar.finish_with_message("✓ Batch processing complete");

        Ok(())
    }

    async fn process_single_batch_static(
        provider: &LLMProvider,
        database: &Database,
        items: Vec<ProcessingItem>,
    ) -> Result<()> {
        // Load existing cabinets and shelves
        let cabinets = database.list_cabinets()?;
        let shelves = database.list_shelves(None)?;

        // Convert items to metadata for LLM
        let item_metadata: Vec<ItemMetadata> = items
            .iter()
            .enumerate()
            .map(|(idx, item)| match item {
                ProcessingItem::File(file) => {
                    let name = file.name.clone();

                    ItemMetadata {
                        id: idx.to_string(),
                        name,
                        item_type: "file".to_string(),
                        extension: file.extension.clone().unwrap_or_default(),
                        size_bytes: file.size,
                        sampled_contents: vec![], // Empty for files
                        content_preview: file.content_preview.clone().unwrap_or("".into()),
                    }
                }
                ProcessingItem::Directory(dir) => {
                    let sampled_names: Vec<String> = dir
                        .sampled_items
                        .iter()
                        .map(|item| item.name.clone())
                        .collect();

                    let is_opaque = Self::is_likely_opaque_directory(&dir.name, &dir.sampled_items);

                    ItemMetadata {
                        id: idx.to_string(),
                        name: dir.name.clone(),
                        item_type: if is_opaque {
                            "likely_opaque_directory"
                        } else {
                            "directory"
                        }
                        .to_string(),
                        extension: "".to_string(), // Empty for directories
                        size_bytes: 0,             // 0 for directories
                        sampled_contents: sampled_names,
                        content_preview: "".to_string(), // Empty for directories
                    }
                }
            })
            .collect();

        // Prepare LLM request
        let request = BatchAnalysisRequest {
            items: item_metadata,
            existing_cabinets: cabinets
                .iter()
                .map(|c| CabinetInfo {
                    id: c.id,
                    name: c.name.clone(),
                    description: c.description.clone(),
                })
                .collect(),
            existing_shelves: shelves
                .iter()
                .map(|s| ShelfInfo {
                    id: s.id,
                    cabinet_id: s.cabinet_id,
                    name: s.name.clone(),
                    description: s.description.clone(),
                })
                .collect(),
        };

        // Call LLM for batch analysis
        let response = Self::analyze_batch_with_llm_static(provider, &request).await?;

        // Process response and update database
        Self::store_batch_results_static(database, &items, &response).await?;

        Ok(())
    }

    async fn analyze_batch_with_llm_static(
        provider: &LLMProvider,
        request: &BatchAnalysisRequest,
    ) -> Result<BatchAnalysisResponse> {
        let prompt = format!(
            "Analyze these files and directories for organization. \
            You have up to 10 cabinets (top-level containers) and up to 10 shelves per cabinet.\n\n\
            Existing Cabinets:\n{}\n\n\
            Existing Shelves:\n{}\n\n\
            Items to analyze:\n{}\n\n\
            For each item, provide:\n\
            1. A brief description (one sentence)\n\
            2. A suggested_name (better name if needed, or empty string if current name is fine)\n\
            3. For directories, determine if they're opaque (homogeneous content, generated files, etc.)\n\
            4. Assign to an existing or new cabinet and shelf\n\n\
            For cabinet and shelf assignments:\n\
            - To use existing: set assignment_type='existing', existing_id to the ID, new_name='' and new_description=''\n\
            - To create new: set assignment_type='new', existing_id=0, new_name and new_description to actual values\n\n\
            Guidelines:\n\
            - Group related items together\n\
            - Use existing cabinets/shelves when appropriate\n\
            - Create new ones only when necessary\n\
            - Keep names short and descriptive\n\
            - Do not treat non-English items any differently\n",
            Self::format_cabinets(&request.existing_cabinets),
            Self::format_shelves(&request.existing_shelves),
            Self::format_items(&request.items)
        );

        Self::extract_with_prompt_static(provider, &prompt).await
    }

    async fn store_batch_results_static(
        database: &Database,
        items: &[ProcessingItem],
        response: &BatchAnalysisResponse,
    ) -> Result<()> {
        let mut cabinet_cache: HashMap<String, i64> = HashMap::new();
        let mut shelf_cache: HashMap<(i64, String), i64> = HashMap::new();

        for (item, analysis) in items.iter().zip(response.items.iter()) {
            // Get or create cabinet
            let cabinet_id = match analysis.cabinet.assignment_type.as_str() {
                "existing" => {
                    if analysis.cabinet.existing_id == 0 {
                        return Err(anyhow!(
                            "existing_id cannot be 0 for existing cabinet assignment"
                        ));
                    }
                    analysis.cabinet.existing_id
                }
                "new" => {
                    if analysis.cabinet.new_name.is_empty()
                        || analysis.cabinet.new_description.is_empty()
                    {
                        return Err(anyhow!(
                            "new_name and new_description cannot be empty for new cabinet assignment"
                        ));
                    }

                    let name = &analysis.cabinet.new_name;
                    let description = &analysis.cabinet.new_description;

                    if let Some(&id) = cabinet_cache.get(name) {
                        id
                    } else {
                        let id = database.create_cabinet(name, description)?;
                        cabinet_cache.insert(name.clone(), id);
                        id
                    }
                }
                _ => {
                    return Err(anyhow!(
                        "Invalid cabinet assignment_type: must be 'existing' or 'new'"
                    ));
                }
            };

            // Get or create shelf
            let shelf_id = match analysis.shelf.assignment_type.as_str() {
                "existing" => {
                    if analysis.shelf.existing_id == 0 {
                        return Err(anyhow!(
                            "existing_id cannot be 0 for existing shelf assignment"
                        ));
                    }
                    analysis.shelf.existing_id
                }
                "new" => {
                    if analysis.shelf.new_name.is_empty()
                        || analysis.shelf.new_description.is_empty()
                    {
                        return Err(anyhow!(
                            "new_name and new_description cannot be empty for new shelf assignment"
                        ));
                    }

                    let name = &analysis.shelf.new_name;
                    let description = &analysis.shelf.new_description;

                    let key = (cabinet_id, name.clone());
                    if let Some(&id) = shelf_cache.get(&key) {
                        id
                    } else {
                        let id = database.create_shelf(cabinet_id, name, description)?;
                        shelf_cache.insert(key, id);
                        id
                    }
                }
                _ => {
                    return Err(anyhow!(
                        "Invalid shelf assignment_type: must be 'existing' or 'new'"
                    ));
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

            let is_opaque_dir = match item {
                ProcessingItem::Directory(dir) => {
                    Self::is_likely_opaque_directory(&dir.name, &dir.sampled_items)
                }
                _ => false,
            };

            let suggested_name = if analysis.suggested_name.is_empty() {
                None
            } else {
                Some(analysis.suggested_name.clone())
            };

            let db_item = Item {
                id: None,
                shelf_id,
                path,
                original_name,
                suggested_name,
                description: analysis.description.clone(),
                file_type,
                is_opaque_dir,
                processed_at: Utc::now(),
            };

            database.insert_item(&db_item)?;
        }

        Ok(())
    }

    async fn extract_with_prompt_static<T>(provider: &LLMProvider, prompt: &str) -> Result<T>
    where
        T: schemars::JsonSchema
            + for<'a> serde::Deserialize<'a>
            + serde::Serialize
            + Send
            + Sync
            + 'static,
    {
        provider.extract(prompt).await
    }

    fn format_cabinets(cabinets: &[CabinetInfo]) -> String {
        if cabinets.is_empty() {
            "None yet".to_string()
        } else {
            cabinets
                .iter()
                .map(|c| format!("- {} (ID: {}): {}", c.name, c.id, c.description))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    fn format_shelves(shelves: &[ShelfInfo]) -> String {
        if shelves.is_empty() {
            "None yet".to_string()
        } else {
            shelves
                .iter()
                .map(|s| {
                    format!(
                        "- Cabinet {}, {} (ID: {}): {}",
                        s.cabinet_id, s.name, s.id, s.description
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    fn format_items(items: &[ItemMetadata]) -> String {
        items
            .iter()
            .map(|item| {
                let mut desc = format!("{}: {} ({})", item.id, item.name, item.item_type);
                if !item.extension.is_empty() {
                    desc.push_str(&format!(".{}", item.extension));
                }
                if item.size_bytes > 0 {
                    desc.push_str(&format!(", {} bytes", item.size_bytes));
                }
                if !item.sampled_contents.is_empty() {
                    let sample = item
                        .sampled_contents
                        .iter()
                        .take(5)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ");
                    desc.push_str(&format!(", contains: [{}...]", sample));
                }
                if !item.content_preview.is_empty() {
                    desc.push_str(&format!(", {}", item.content_preview));
                }
                desc
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn is_likely_opaque_directory(
        name: &str,
        sampled_items: &[crate::models::SampledItem],
    ) -> bool {
        // Known opaque directory patterns
        const OPAQUE_PATTERNS: &[&str] = &[
            "node_modules",
            "__pycache__",
            ".git",
            ".svn",
            "target",
            "dist",
            "build",
            "out",
            ".idea",
            ".vscode",
            "vendor",
            "deps",
            ".cache",
            "tmp",
            "temp",
        ];

        // Check if name matches known patterns
        if OPAQUE_PATTERNS.iter().any(|&pattern| name == pattern) {
            return true;
        }

        // Check for homogeneous numbered content
        if sampled_items.len() >= 5 {
            let numbered_pattern = sampled_items
                .iter()
                .filter(|item| {
                    // Check for patterns like file_001, screenshot_1, log_2024
                    item.name.chars().any(|c| c.is_ascii_digit())
                })
                .count();

            // If more than 80% of items have numbers, likely homogeneous
            if numbered_pattern as f32 / sampled_items.len() as f32 > 0.8 {
                // Check if extensions are mostly the same
                let extensions: Vec<_> = sampled_items
                    .iter()
                    .filter_map(|item| item.extension.as_ref())
                    .collect();

                if !extensions.is_empty() {
                    let first_ext = &extensions[0];
                    let same_ext_count = extensions.iter().filter(|&ext| ext == first_ext).count();

                    if same_ext_count as f32 / extensions.len() as f32 > 0.8 {
                        return true;
                    }
                }
            }
        }

        false
    }
}
