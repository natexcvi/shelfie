use anyhow::{Result, Context};
use crossbeam::queue::SegQueue;
use crossbeam::channel::{unbounded, Sender, Receiver};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;
use indicatif::{ProgressBar, ProgressStyle};

use crate::{
    database::Database,
    file_analyzer::AnalyzedFile,
    models::{ProcessingItem, EnrichedFile, EnrichedDirectory, SampledItem},
};

pub struct ConcurrentProcessor {
    work_queue: Arc<SegQueue<PathBuf>>,
    channel_sender: Sender<ProcessingItem>,
    channel_receiver: Receiver<ProcessingItem>,
    processed_paths: Vec<String>,
    items_processed: Arc<AtomicUsize>,
    items_total: Arc<AtomicUsize>,
}

impl ConcurrentProcessor {
    pub fn new(base_path: &Path) -> Result<Self> {
        // Load processed paths from database if it exists
        let processed_paths = if Database::exists(base_path) {
            let db = Database::open_or_create(base_path)?;
            db.get_processed_paths().unwrap_or_default()
        } else {
            Vec::new()
        };
        
        let (sender, receiver) = unbounded();
        
        Ok(Self {
            work_queue: Arc::new(SegQueue::new()),
            channel_sender: sender,
            channel_receiver: receiver,
            processed_paths,
            items_processed: Arc::new(AtomicUsize::new(0)),
            items_total: Arc::new(AtomicUsize::new(0)),
        })
    }
    
    pub fn initialize_queue(&self, base_path: &Path, max_depth: usize) -> Result<()> {
        println!("📂 Scanning directory for items to process...");
        
        // Collect all items first to know the total count
        let all_items: Vec<PathBuf> = WalkDir::new(base_path)
            .max_depth(max_depth)
            .into_iter()
            .filter_map(|entry| {
                entry.ok().and_then(|e| {
                    let path = e.path().to_path_buf();
                    
                    // Skip if already processed
                    let path_str = path.to_string_lossy().to_string();
                    if self.processed_paths.contains(&path_str) {
                        return None;
                    }
                    
                    // Skip hidden files and the database file
                    if let Some(name) = path.file_name() {
                        let name_str = name.to_string_lossy();
                        if name_str.starts_with('.') || name_str == ".fs_organizer.db" {
                            return None;
                        }
                    }
                    
                    // Skip the base path itself
                    if path == base_path {
                        return None;
                    }
                    
                    Some(path)
                })
            })
            .collect();
        
        let total = all_items.len();
        self.items_total.store(total, Ordering::Relaxed);
        
        // Add all items to the queue
        for path in all_items {
            self.work_queue.push(path);
        }
        
        if total == 0 {
            println!("✓ All items already processed or no new items found");
        } else {
            println!("✓ Found {} items to process", total);
        }
        
        Ok(())
    }
    
    pub fn run_file_analysis_workers(&self, num_workers: usize) -> Result<()> {
        if self.items_total.load(Ordering::Relaxed) == 0 {
            return Ok(());
        }
        
        let progress_bar = ProgressBar::new(self.items_total.load(Ordering::Relaxed) as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );
        progress_bar.set_message("Analyzing files and directories...");
        
        let work_queue = Arc::clone(&self.work_queue);
        let sender = self.channel_sender.clone();
        let items_processed = Arc::clone(&self.items_processed);
        let pb = Arc::new(progress_bar);
        
        // Use rayon's thread pool for parallel processing
        rayon::scope(|s| {
            for _ in 0..num_workers {
                let queue = Arc::clone(&work_queue);
                let sender = sender.clone();
                let items_processed = Arc::clone(&items_processed);
                let pb = Arc::clone(&pb);
                
                s.spawn(move |_| {
                    while let Some(path) = queue.pop() {
                        let result = if path.is_file() {
                            Self::process_file(&path)
                        } else if path.is_dir() {
                            Self::process_directory(&path)
                        } else {
                            continue;
                        };
                        
                        if let Ok(item) = result {
                            let _ = sender.send(item);
                        }
                        
                        items_processed.fetch_add(1, Ordering::Relaxed);
                        pb.inc(1);
                    }
                });
            }
        });
        
        pb.finish_with_message("✓ File analysis complete");
        
        Ok(())
    }
    
    fn process_file(path: &Path) -> Result<ProcessingItem> {
        let analyzed = AnalyzedFile::new(path.to_path_buf())
            .context("Failed to analyze file")?;
        
        let enriched = EnrichedFile {
            path: path.to_path_buf(),
            name: analyzed.name.clone(),
            extension: analyzed.extension.clone(),
            file_type: analyzed.get_type_description(),
            size: analyzed.size,
            content_preview: None, // Will be filled later if needed
        };
        
        Ok(ProcessingItem::File(enriched))
    }
    
    fn process_directory(path: &Path) -> Result<ProcessingItem> {
        const SAMPLE_SIZE: usize = 20;
        
        let name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        // Sample items from the directory
        let mut sampled_items = Vec::new();
        let mut count = 0;
        
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.filter_map(Result::ok) {
                if count >= SAMPLE_SIZE {
                    break;
                }
                
                let entry_path = entry.path();
                if let Some(entry_name) = entry_path.file_name().and_then(|n| n.to_str()) {
                    // Skip hidden files
                    if entry_name.starts_with('.') {
                        continue;
                    }
                    
                    let is_file = entry_path.is_file();
                    let extension = if is_file {
                        entry_path.extension()
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
    
    pub fn get_receiver(&self) -> Receiver<ProcessingItem> {
        self.channel_receiver.clone()
    }
    
    pub fn items_remaining(&self) -> usize {
        self.items_total.load(Ordering::Relaxed) - self.items_processed.load(Ordering::Relaxed)
    }
    
    // Check if a directory is likely opaque based on name patterns
    pub fn is_likely_opaque_directory(name: &str, sampled_items: &[SampledItem]) -> bool {
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
            let numbered_pattern = sampled_items.iter()
                .filter(|item| {
                    // Check for patterns like file_001, screenshot_1, log_2024
                    item.name.chars().any(|c| c.is_ascii_digit())
                })
                .count();
            
            // If more than 80% of items have numbers, likely homogeneous
            if numbered_pattern as f32 / sampled_items.len() as f32 > 0.8 {
                // Check if extensions are mostly the same
                let extensions: Vec<_> = sampled_items.iter()
                    .filter_map(|item| item.extension.as_ref())
                    .collect();
                
                if !extensions.is_empty() {
                    let first_ext = &extensions[0];
                    let same_ext_count = extensions.iter()
                        .filter(|&ext| ext == first_ext)
                        .count();
                    
                    if same_ext_count as f32 / extensions.len() as f32 > 0.8 {
                        return true;
                    }
                }
            }
        }
        
        false
    }
}