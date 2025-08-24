use super::{AnalyzedFile, FileContent};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

async fn create_test_file(dir: &TempDir, name: &str, content: &[u8]) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, content).unwrap();
    path
}

#[tokio::test]
async fn test_analyze_text_file() {
    let temp_dir = TempDir::new().unwrap();
    let path = create_test_file(&temp_dir, "test.txt", b"Hello, world!").await;

    let analyzed = AnalyzedFile::new(path.clone()).await.unwrap();

    assert_eq!(analyzed.name, "test");
    assert_eq!(analyzed.extension, Some("txt".to_string()));
    assert_eq!(analyzed.size, 13);
    assert!(analyzed.path == path);

    match analyzed.content {
        FileContent::Preview(content) => {
            assert!(content.contains("Hello, world!"));
        }
        FileContent::Unparsable(_) => panic!("Text file should be parsable"),
    }
}

#[tokio::test]
async fn test_analyze_image_file() {
    let analyzed = AnalyzedFile::new("test_dir/12.png".into()).await.unwrap();

    assert_eq!(analyzed.name, "12");
    assert_eq!(analyzed.extension, Some("png".to_string()));
    assert_eq!(analyzed.size, 75037);
    assert_eq!(analyzed.detected_type, "image/png");
}

#[tokio::test]
async fn test_analyze_rust_file() {
    let temp_dir = TempDir::new().unwrap();
    let rust_code = b"fn main() {\n    println!(\"Hello, Rust!\");\n}";
    let path = create_test_file(&temp_dir, "main.rs", rust_code).await;

    let analyzed = AnalyzedFile::new(path.clone()).await.unwrap();

    assert_eq!(analyzed.name, "main");
    assert_eq!(analyzed.extension, Some("rs".to_string()));
    assert_eq!(analyzed.size, rust_code.len() as u64);

    match analyzed.content {
        FileContent::Preview(content) => {
            assert!(content.contains("main") || content.contains("Hello"));
        }
        _ => {}
    }
}

#[tokio::test]
async fn test_analyze_json_file() {
    let temp_dir = TempDir::new().unwrap();
    let json_content = br#"{"name": "test", "version": "1.0.0"}"#;
    let path = create_test_file(&temp_dir, "package.json", json_content).await;

    let analyzed = AnalyzedFile::new(path.clone()).await.unwrap();

    assert_eq!(analyzed.name, "package");
    assert_eq!(analyzed.extension, Some("json".to_string()));
    assert_eq!(analyzed.size, json_content.len() as u64);
}

#[tokio::test]
async fn test_analyze_binary_file() {
    let temp_dir = TempDir::new().unwrap();
    let binary_content = vec![0x00, 0xFF, 0x42, 0x13, 0x37];
    let path = create_test_file(&temp_dir, "binary.dat", &binary_content).await;

    let analyzed = AnalyzedFile::new(path.clone()).await.unwrap();

    assert_eq!(analyzed.name, "binary");
    assert_eq!(analyzed.extension, Some("dat".to_string()));
    assert_eq!(analyzed.size, binary_content.len() as u64);
}

#[tokio::test]
async fn test_file_type_detection() {
    let pdf_path = "test_dir/2502.08966v2.pdf".into();
    let pdf_analyzed = AnalyzedFile::new(pdf_path).await.unwrap();
    assert_eq!(pdf_analyzed.detected_type, "application/pdf");
}

#[tokio::test]
async fn test_get_type_description() {
    let temp_dir = TempDir::new().unwrap();
    let path = create_test_file(&temp_dir, "test.txt", b"content").await;

    let analyzed = AnalyzedFile::new(path).await.unwrap();
    let type_desc = analyzed.get_type_description();

    assert!(!type_desc.is_empty());
}

#[tokio::test]
async fn test_analyze_markdown_file() {
    let temp_dir = TempDir::new().unwrap();
    let md_content = b"# Title\n\n## Subtitle\n\nSome content here.";
    let path = create_test_file(&temp_dir, "README.md", md_content).await;

    let analyzed = AnalyzedFile::new(path.clone()).await.unwrap();

    assert_eq!(analyzed.name, "README");
    assert_eq!(analyzed.extension, Some("md".to_string()));

    match analyzed.content {
        FileContent::Preview(content) => {
            assert!(
                content.contains("Title")
                    || content.contains("Subtitle")
                    || content.contains("content")
            );
        }
        _ => {}
    }
}

#[tokio::test]
async fn test_analyze_empty_file() {
    let temp_dir = TempDir::new().unwrap();
    let path = create_test_file(&temp_dir, "empty.txt", b"").await;

    let analyzed = AnalyzedFile::new(path.clone()).await.unwrap();

    assert_eq!(analyzed.name, "empty");
    assert_eq!(analyzed.extension, Some("txt".to_string()));
    assert_eq!(analyzed.size, 0);
}

#[tokio::test]
async fn test_analyze_large_filename() {
    let temp_dir = TempDir::new().unwrap();
    let long_name = "very_long_filename_with_multiple_parts_and_extensions.tar.gz";
    let path = create_test_file(&temp_dir, long_name, b"compressed data").await;

    let analyzed = AnalyzedFile::new(path.clone()).await.unwrap();

    assert_eq!(
        analyzed.name,
        "very_long_filename_with_multiple_parts_and_extensions.tar"
    );
    assert_eq!(analyzed.extension, Some("gz".to_string()));
}
