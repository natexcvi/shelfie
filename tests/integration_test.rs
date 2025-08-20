use anyhow::Result;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

use fs_organiser::{
    file_analyzer::AnalyzedFile,
};

#[test]
fn test_file_organization_integration() -> Result<()> {
    let test_dir = Path::new("test_dir");
    if !test_dir.exists() {
        println!("Skipping integration test - test_dir not found");
        return Ok(());
    }

    // Create a temporary directory and copy test files
    let temp_dir = TempDir::new()?;
    let test_dir_path = temp_dir.path();

    // Copy test files from test_dir to temp directory
    copy_test_files_to_temp(test_dir_path)?;

    // Scan for files in the temp directory
    let files = scan_directory(test_dir_path)?;
    
    // Verify we found the expected files
    assert!(!files.is_empty(), "Should find files in test directory");
    
    // Look for specific test files
    let file_names: Vec<String> = files.iter()
        .map(|f| f.name.clone())
        .collect();
    
    println!("Found files: {:?}", file_names);
    
    // We expect to find files like doc123, file1, recipe, 12, and the PDF
    assert!(file_names.iter().any(|name| name.contains("doc123")), "Should find doc123 file");
    assert!(file_names.iter().any(|name| name.contains("file1")), "Should find file1 file");
    assert!(file_names.iter().any(|name| name.contains("recipe")), "Should find recipe file");

    // Test that files are properly analyzed
    for file in &files {
        assert!(file.is_analyzable(), "File {} should be analyzable", file.name);
        
        // Test content preview generation
        let preview = file.get_content_preview();
        assert!(!preview.is_empty(), "Content preview should not be empty");
        
        // Test type description
        let type_desc = file.get_type_description();
        assert!(!type_desc.is_empty(), "Type description should not be empty");
    }

    // Test file organization logic without AI calls
    // Verify that different file types are detected correctly
    let text_files: Vec<_> = files.iter()
        .filter(|f| matches!(f.detected_type, fs_organiser::file_analyzer::DetectedFileType::Text))
        .collect();
    
    let image_files: Vec<_> = files.iter()
        .filter(|f| matches!(f.detected_type, fs_organiser::file_analyzer::DetectedFileType::Image(_)))
        .collect();
    
    let pdf_files: Vec<_> = files.iter()
        .filter(|f| matches!(f.detected_type, fs_organiser::file_analyzer::DetectedFileType::Pdf))
        .collect();

    assert!(!text_files.is_empty(), "Should have at least one text file");
    println!("✓ Found {} text files, {} images, {} PDFs", 
             text_files.len(), image_files.len(), pdf_files.len());

    // Test that relative paths can be computed correctly
    for file in &files {
        let relative_path = file.path
            .strip_prefix(test_dir_path)
            .unwrap_or(&file.path)
            .to_string_lossy()
            .to_string();
        
        assert!(!relative_path.is_empty(), "Relative path should not be empty");
        assert!(!relative_path.starts_with('/'), "Relative path should not start with /");
    }

    println!("✓ File organization integration tests passed");
    Ok(())
}

#[tokio::test]
async fn test_file_type_detection() -> Result<()> {
    let test_dir = Path::new("test_dir");
    if !test_dir.exists() {
        println!("Skipping file type detection test - test_dir not found");
        return Ok(());
    }

    // Test different file types
    let files = scan_directory(test_dir)?;
    
    let mut found_text = false;
    let mut found_image = false;
    let mut found_pdf = false;
    let mut found_markdown = false;

    for file in files {
        match &file.detected_type {
            fs_organiser::file_analyzer::DetectedFileType::Text => {
                found_text = true;
                println!("✓ Found text file: {}", file.name);
            },
            fs_organiser::file_analyzer::DetectedFileType::Image(mime) => {
                found_image = true;
                println!("✓ Found image file: {} ({})", file.name, mime);
            },
            fs_organiser::file_analyzer::DetectedFileType::Pdf => {
                found_pdf = true;
                println!("✓ Found PDF file: {}", file.name);
            },
            _ => {
                // Check if this is a markdown file based on extension
                if file.extension.as_ref().map(|e| e == "md").unwrap_or(false) {
                    found_markdown = true;
                    println!("✓ Found markdown file: {}", file.name);
                }
            }
        }
    }

    // Verify we detected the expected file types from test_dir
    assert!(found_text, "Should detect at least one text file");
    if found_image { println!("✓ Image detection working"); }
    if found_pdf { println!("✓ PDF detection working"); }
    if found_markdown { println!("✓ Markdown detection working"); }
    println!("File type detection tests completed");
    
    Ok(())
}

#[test]
fn test_file_content_preview() -> Result<()> {
    let test_dir = Path::new("test_dir");
    if !test_dir.exists() {
        println!("Skipping content preview test - test_dir not found");
        return Ok(());
    }

    // Test that we can generate content previews for different file types
    let test_file = test_dir.join("doc123.txt");
    if test_file.exists() {
        let analyzed_file = AnalyzedFile::new(test_file)?;
        let preview = analyzed_file.get_content_preview();
        
        assert!(!preview.is_empty(), "Content preview should not be empty");
        assert!(preview.contains("Meeting notes") || preview.len() <= 500, 
               "Preview should contain expected content or be truncated");
        
        println!("✓ Content preview test passed: {}", preview);
    }

    Ok(())
}

// Helper function to scan directory and return analyzed files
fn scan_directory(dir_path: &Path) -> Result<Vec<AnalyzedFile>> {
    use walkdir::WalkDir;
    
    let mut files = Vec::new();
    
    for entry in WalkDir::new(dir_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            match AnalyzedFile::new(entry.path().to_path_buf()) {
                Ok(file) => files.push(file),
                Err(e) => println!("Warning: Could not analyze file {}: {}", 
                                 entry.path().display(), e),
            }
        }
    }
    
    Ok(files)
}

// Helper function to copy test files to temporary directory
fn copy_test_files_to_temp(temp_dir: &Path) -> Result<()> {
    let source_dir = Path::new("test_dir");
    if !source_dir.exists() {
        return Ok(()); // Skip if test_dir doesn't exist
    }

    use walkdir::WalkDir;
    
    for entry in WalkDir::new(source_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let source_path = entry.path();
        let relative_path = source_path.strip_prefix(source_dir)?;
        let dest_path = temp_dir.join(relative_path);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest_path)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(source_path, &dest_path)?;
        }
    }

    Ok(())
}

#[test]
fn test_pdf_panic_handling() -> Result<()> {
    let test_dir = Path::new("test_dir");
    if !test_dir.exists() {
        println!("Skipping PDF panic handling test - test_dir not found");
        return Ok(());
    }

    // Test PDF processing with the real PDF file
    let pdf_file = test_dir.join("2502.08966v2.pdf");
    if pdf_file.exists() {
        // This should not panic even if the PDF extraction fails
        match AnalyzedFile::new(pdf_file) {
            Ok(analyzed_file) => {
                println!("✓ PDF file processed successfully without panicking");
                let preview = analyzed_file.get_content_preview();
                assert!(!preview.is_empty(), "PDF should have some content or fallback message");
                
                // Verify it's recognized as a PDF
                assert!(matches!(analyzed_file.detected_type, 
                    fs_organiser::file_analyzer::DetectedFileType::Pdf));
                    
                println!("✓ PDF content preview: {}", 
                    if preview.len() > 100 { 
                        format!("{}...", &preview[..100])
                    } else { 
                        preview.to_string()
                    });
            }
            Err(e) => {
                panic!("PDF processing should not return an error: {}", e);
            }
        }
    } else {
        println!("Skipping PDF panic test - no PDF file found in test_dir");
    }

    Ok(())
}