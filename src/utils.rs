use anyhow::Result;
use colored::*;
use std::path::Path;
use walkdir::WalkDir;
use crate::file_analyzer::AnalyzedFile;

pub fn walk_directory(path: &Path) -> Result<Vec<AnalyzedFile>> {
    let mut files = Vec::new();
    
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        match AnalyzedFile::new(entry.path().to_path_buf()) {
            Ok(file) => {
                if file.is_analyzable() {
                    files.push(file);
                }
            }
            Err(e) => {
                eprintln!("{}: Failed to analyze {}: {}", 
                    "Warning".yellow(), 
                    entry.path().display(), 
                    e
                );
            }
        }
    }
    
    Ok(files)
}

pub fn print_tree(path: &Path, prefix: &str, is_last: bool) {
    let name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    
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