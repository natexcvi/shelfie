use crate::file_analyzer::AnalyzedFile;
use anyhow::Result;
use colored::*;
use std::path::Path;

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
