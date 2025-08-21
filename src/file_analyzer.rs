use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Result;
use extractous::Extractor;
use tokio::{io::AsyncReadExt, time::timeout};

#[derive(Debug, Clone)]
pub enum FileContent {
    Preview(String),
    Unparsable(String),
}

#[derive(Debug, Clone)]
pub struct AnalyzedFile {
    pub path: PathBuf,
    pub name: String,
    pub extension: Option<String>,
    pub content: FileContent,
    pub detected_type: String, // mime type like "image/jpeg"
    pub size: u64,
}

impl AnalyzedFile {
    pub async fn new(path: PathBuf) -> Result<Self> {
        let metadata = tokio::fs::metadata(&path).await?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());

        // Read first few bytes to detect file type
        let mut buffer = vec![0; 512];
        let bytes_read = match tokio::fs::File::open(&path).await {
            Ok(mut file) => file.read_exact(&mut buffer).await.unwrap_or(0),
            Err(_) => 0,
        };
        buffer.truncate(bytes_read);

        let detected_type = Self::detect_file_type(&buffer, extension.as_deref());
        let content = match Self::extract_preview_from_file(&path).await {
            Ok(content) => content,
            Err(err) => FileContent::Unparsable(format!("Failed to read file content: {}", err)),
        };

        Ok(Self {
            path,
            name,
            extension,
            content,
            detected_type,
            size: metadata.len(),
        })
    }

    fn detect_file_type(buffer: &[u8], extension: Option<&str>) -> String {
        // First try to detect by content using infer
        if let Some(kind) = infer::get(buffer) {
            let mime_type = kind.mime_type();

            // If the extension is provided, check if it matches the detected type
            if let Some(ext) = extension {
                if mime_type == "application/octet-stream" && ext == "exe" {
                    return "Executable".to_string();
                }
            }

            mime_type.to_string()
        } else {
            "Unknown".to_string()
        }
    }

    async fn extract_preview_from_file(path: &Path) -> Result<FileContent> {
        let extractor = Extractor::new().set_extract_string_max_length(1000);

        let mut file = tokio::fs::File::open(path).await?;
        let mut buffer = vec![0; file.metadata().await?.len().min(1024) as usize];
        file.read_exact(&mut buffer).await?;
        let extraction_future =
            tokio::task::spawn_blocking(move || extractor.extract_bytes_to_string(&buffer));
        match timeout(Duration::from_secs(5), extraction_future).await {
            Ok(extraction_result) => match extraction_result? {
                Ok((preview, _)) => Ok(FileContent::Preview(preview)),
                Err(e) => Err(anyhow::Error::new(e)),
            },
            Err(_) => Err(anyhow::Error::msg("Timeout")),
        }
    }

    pub fn get_type_description(&self) -> String {
        return self.detected_type.to_string();
    }
}
