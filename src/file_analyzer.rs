use anyhow::{Result, Context};
use std::fs;
use std::path::{Path, PathBuf};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use pdf_extract::extract_text;

#[derive(Debug, Clone)]
pub enum FileContent {
    Text(String),
    Image(String), // Base64 encoded
    Pdf(String),
    Binary,
}

#[derive(Debug, Clone)]
pub struct AnalyzedFile {
    pub path: PathBuf,
    pub name: String,
    pub extension: Option<String>,
    pub content: FileContent,
    pub size: u64,
}

impl AnalyzedFile {
    pub fn new(path: PathBuf) -> Result<Self> {
        let metadata = fs::metadata(&path)?;
        let name = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let extension = path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        
        let content = Self::read_file_content(&path, extension.as_deref())?;
        
        Ok(Self {
            path,
            name,
            extension,
            content,
            size: metadata.len(),
        })
    }
    
    fn read_file_content(path: &Path, extension: Option<&str>) -> Result<FileContent> {
        match extension {
            Some("txt") | Some("md") | Some("rs") | Some("py") | Some("js") | 
            Some("ts") | Some("jsx") | Some("tsx") | Some("java") | Some("c") | 
            Some("cpp") | Some("h") | Some("hpp") | Some("go") | Some("rb") |
            Some("sh") | Some("yaml") | Some("yml") | Some("toml") | Some("json") |
            Some("xml") | Some("html") | Some("css") | Some("scss") | Some("sql") => {
                let content = fs::read_to_string(path)
                    .context("Failed to read text file")?;
                Ok(FileContent::Text(content))
            }
            Some("pdf") => {
                let text = extract_text(path)
                    .unwrap_or_else(|_| String::from("Could not extract PDF text"));
                Ok(FileContent::Pdf(text))
            }
            Some("png") | Some("jpg") | Some("jpeg") | Some("gif") | Some("bmp") | 
            Some("webp") | Some("svg") => {
                let img_data = fs::read(path)
                    .context("Failed to read image file")?;
                let base64 = STANDARD.encode(&img_data);
                Ok(FileContent::Image(base64))
            }
            _ => Ok(FileContent::Binary),
        }
    }
    
    pub fn get_content_preview(&self) -> String {
        match &self.content {
            FileContent::Text(text) => {
                let preview = text.chars().take(500).collect::<String>();
                if text.len() > 500 {
                    format!("{}...", preview)
                } else {
                    preview
                }
            }
            FileContent::Pdf(text) => {
                let preview = text.chars().take(500).collect::<String>();
                if text.len() > 500 {
                    format!("{}...", preview)
                } else {
                    preview
                }
            }
            FileContent::Image(_) => "[Image file]".to_string(),
            FileContent::Binary => "[Binary file]".to_string(),
        }
    }
    
    pub fn is_analyzable(&self) -> bool {
        !matches!(self.content, FileContent::Binary)
    }
}