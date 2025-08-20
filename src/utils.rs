use crate::file_analyzer::AnalyzedFile;
use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use walkdir::WalkDir;

pub fn walk_directory(path: &Path, max_depth: usize) -> Result<Vec<AnalyzedFile>> {
    // First, count total files to set up progress bar
    let total_files: usize = WalkDir::new(path)
        .max_depth(max_depth)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count();

    if total_files == 0 {
        return Ok(Vec::new());
    }

    // Create progress bar
    let pb = ProgressBar::new(total_files as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message("Analyzing files...");

    let mut files = Vec::new();
    let mut analyzed_count = 0;

    for entry in WalkDir::new(path)
        .max_depth(max_depth)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        match AnalyzedFile::new(entry.path().to_path_buf()) {
            Ok(file) => {
                if file.is_analyzable() {
                    analyzed_count += 1;
                    files.push(file);
                }
            }
            Err(e) => {
                // Store error messages to display after progress bar
                eprintln!(
                    "\n{}: Failed to analyze {}: {}",
                    "Warning".yellow(),
                    entry.path().display(),
                    e
                );
            }
        }
        pb.inc(1);
    }

    pb.finish_with_message(format!("✓ Analyzed {} files", analyzed_count));

    Ok(files)
}

pub fn print_tree(path: &Path, prefix: &str, is_last: bool) {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    let connector = if is_last { "└── " } else { "├── " };
    println!("{}{}{}", prefix, connector, name.blue());

    if path.is_dir() {
        let new_prefix = if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        if let Ok(entries) = std::fs::read_dir(path) {
            let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            entries.sort_by_key(|e| e.path());

            let count = entries.len();
            for (i, entry) in entries.iter().enumerate() {
                print_tree(&entry.path(), &new_prefix, i == count - 1);
            }
        }
    }
}

pub fn get_existing_directories(path: &Path) -> Result<Vec<String>> {
    let mut directories = Vec::new();
    
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    // Skip hidden directories (starting with .)
                    if !name.starts_with('.') {
                        directories.push(name.to_string());
                    }
                }
            }
        }
    }
    
    directories.sort();
    Ok(directories)
}

pub fn format_size(size: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut size = size as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}
