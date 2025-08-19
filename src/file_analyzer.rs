use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use pdf_extract::extract_text;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum FileContent {
    Text(String),
    Image(String), // Base64 encoded
    Pdf(String),
    Audio(String),   // Audio file metadata
    Video(String),   // Video file metadata
    Archive(String), // Archive file metadata
    Binary,
}

#[derive(Debug, Clone)]
pub enum DetectedFileType {
    Text,
    Image(String), // MIME type like "image/jpeg"
    Pdf,
    Audio(String),   // MIME type like "audio/mp3"
    Video(String),   // MIME type like "video/mp4"
    Archive(String), // MIME type like "application/zip"
    Binary,
}

#[derive(Debug, Clone)]
pub struct AnalyzedFile {
    pub path: PathBuf,
    pub name: String,
    pub extension: Option<String>,
    pub content: FileContent,
    pub detected_type: DetectedFileType,
    pub size: u64,
}

impl AnalyzedFile {
    pub fn new(path: PathBuf) -> Result<Self> {
        let metadata = fs::metadata(&path)?;
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
        let mut buffer = vec![0; 8192]; // Read up to 8KB for type detection
        let bytes_read = match fs::File::open(&path) {
            Ok(mut file) => {
                use std::io::Read;
                file.read(&mut buffer).unwrap_or(0)
            }
            Err(_) => 0,
        };
        buffer.truncate(bytes_read);

        let detected_type = Self::detect_file_type(&buffer, extension.as_deref());
        let content = Self::read_file_content(&path, &detected_type)?;

        Ok(Self {
            path,
            name,
            extension,
            content,
            detected_type,
            size: metadata.len(),
        })
    }

    fn detect_file_type(buffer: &[u8], extension: Option<&str>) -> DetectedFileType {
        // First try to detect by content using infer
        if let Some(kind) = infer::get(buffer) {
            let mime_type = kind.mime_type();

            match mime_type {
                // Images
                mime if mime.starts_with("image/") => DetectedFileType::Image(mime.to_string()),

                // Audio
                mime if mime.starts_with("audio/") => DetectedFileType::Audio(mime.to_string()),

                // Video
                mime if mime.starts_with("video/") => DetectedFileType::Video(mime.to_string()),

                // PDFs
                "application/pdf" => DetectedFileType::Pdf,

                // Archives
                "application/zip"
                | "application/x-rar-compressed"
                | "application/x-tar"
                | "application/gzip"
                | "application/x-7z-compressed" => DetectedFileType::Archive(mime_type.to_string()),

                _ => DetectedFileType::Binary,
            }
        } else {
            // Fallback to extension-based detection for text files
            match extension {
                Some("txt") | Some("md") | Some("rs") | Some("py") | Some("js") | Some("ts")
                | Some("jsx") | Some("tsx") | Some("java") | Some("c") | Some("cpp")
                | Some("h") | Some("hpp") | Some("go") | Some("rb") | Some("sh") | Some("yaml")
                | Some("yml") | Some("toml") | Some("json") | Some("xml") | Some("html")
                | Some("css") | Some("scss") | Some("sql") | Some("csv") | Some("log")
                | Some("conf") | Some("cfg") | Some("ini") => {
                    // Verify it's actually text by checking for null bytes
                    if buffer.iter().any(|&b| b == 0) {
                        DetectedFileType::Binary
                    } else {
                        DetectedFileType::Text
                    }
                }
                Some("pdf") => DetectedFileType::Pdf,
                _ => DetectedFileType::Binary,
            }
        }
    }

    fn read_file_content(path: &Path, detected_type: &DetectedFileType) -> Result<FileContent> {
        match detected_type {
            DetectedFileType::Text => {
                let content = fs::read_to_string(path).context("Failed to read text file")?;
                Ok(FileContent::Text(content))
            }
            DetectedFileType::Pdf => {
                let text = extract_text(path)
                    .unwrap_or_else(|_| String::from("Could not extract PDF text"));
                Ok(FileContent::Pdf(text))
            }
            DetectedFileType::Image(_mime_type) => {
                let img_data = fs::read(path).context("Failed to read image file")?;
                let base64 = STANDARD.encode(&img_data);
                Ok(FileContent::Image(base64))
            }
            DetectedFileType::Audio(mime_type) => {
                Ok(FileContent::Audio(format!("Audio file: {}", mime_type)))
            }
            DetectedFileType::Video(mime_type) => {
                Ok(FileContent::Video(format!("Video file: {}", mime_type)))
            }
            DetectedFileType::Archive(mime_type) => {
                Ok(FileContent::Archive(format!("Archive file: {}", mime_type)))
            }
            DetectedFileType::Binary => Ok(FileContent::Binary),
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
            FileContent::Image(_) => format!("[Image file: {}]", self.get_type_description()),
            FileContent::Audio(desc) => format!("[{}]", desc),
            FileContent::Video(desc) => format!("[{}]", desc),
            FileContent::Archive(desc) => format!("[{}]", desc),
            FileContent::Binary => "[Binary file]".to_string(),
        }
    }

    pub fn get_type_description(&self) -> String {
        match &self.detected_type {
            DetectedFileType::Text => "Text file".to_string(),
            DetectedFileType::Image(mime) => format!("Image ({})", mime),
            DetectedFileType::Pdf => "PDF document".to_string(),
            DetectedFileType::Audio(mime) => format!("Audio ({})", mime),
            DetectedFileType::Video(mime) => format!("Video ({})", mime),
            DetectedFileType::Archive(mime) => format!("Archive ({})", mime),
            DetectedFileType::Binary => "Binary file".to_string(),
        }
    }

    pub fn is_analyzable(&self) -> bool {
        matches!(
            self.content,
            FileContent::Text(_)
                | FileContent::Pdf(_)
                | FileContent::Image(_)
                | FileContent::Audio(_)
                | FileContent::Video(_)
                | FileContent::Archive(_)
        )
    }
}
