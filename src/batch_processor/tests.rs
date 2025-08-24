use super::*;
use crate::database::Database;
use crate::providers::LLMProvider;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_complete_organization_workflow() {
    // Create a temporary directory with real files
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path().to_path_buf();

    // Create a realistic project structure
    fs::create_dir_all(base_path.join("src")).unwrap();
    fs::write(
        base_path.join("src/main.rs"),
        "fn main() {\n    println!(\"Hello, world!\");\n}",
    )
    .unwrap();
    fs::write(
        base_path.join("src/lib.rs"),
        "pub mod utils;\npub mod models;\n\npub use models::*;",
    )
    .unwrap();
    fs::write(
        base_path.join("README.md"),
        "# Test Project\n\nThis is a test Rust project for the organizer.",
    )
    .unwrap();
    fs::write(
        base_path.join("Cargo.toml"),
        "[package]\nname = \"test-project\"\nversion = \"0.1.0\"\nedition = \"2021\"",
    )
    .unwrap();

    // Create items to process (simulating what the organizer would create)
    let items = vec![
        ProcessingItem::File(EnrichedFile {
            path: base_path.join("src/main.rs"),
            name: "main.rs".to_string(),
            extension: Some("rs".to_string()),
            file_type: "text/rust".to_string(),
            size: fs::metadata(base_path.join("src/main.rs")).unwrap().len(),
            content_preview: Some("fn main() { println!(\"Hello, world!\"); }".to_string()),
        }),
        ProcessingItem::File(EnrichedFile {
            path: base_path.join("src/lib.rs"),
            name: "lib.rs".to_string(),
            extension: Some("rs".to_string()),
            file_type: "text/rust".to_string(),
            size: fs::metadata(base_path.join("src/lib.rs")).unwrap().len(),
            content_preview: Some("pub mod utils; pub mod models;".to_string()),
        }),
        ProcessingItem::File(EnrichedFile {
            path: base_path.join("README.md"),
            name: "README.md".to_string(),
            extension: Some("md".to_string()),
            file_type: "text/markdown".to_string(),
            size: fs::metadata(base_path.join("README.md")).unwrap().len(),
            content_preview: Some("# Test Project\n\nThis is a test Rust project".to_string()),
        }),
    ];

    // Create mock response
    let mock_response = BatchAnalysisResponse {
        items: vec![
            ItemAnalysis {
                id: "0".to_string(),
                description: "Main Rust application entry point".to_string(),
                suggested_name: "".to_string(),
                is_opaque_directory: false,
                cabinet: CabinetAssignment {
                    assignment_type: "new".to_string(),
                    existing_id: 0,
                    new_name: "Source Code".to_string(),
                    new_description: "Application source files".to_string(),
                },
                shelf: ShelfAssignment {
                    assignment_type: "new".to_string(),
                    existing_id: 0,
                    new_name: "Core".to_string(),
                    new_description: "Main application code".to_string(),
                },
            },
            ItemAnalysis {
                id: "1".to_string(),
                description: "Library module with shared functionality".to_string(),
                suggested_name: "".to_string(),
                is_opaque_directory: false,
                cabinet: CabinetAssignment {
                    assignment_type: "existing".to_string(),
                    existing_id: 1, // Will be the Source Code cabinet
                    new_name: "".to_string(),
                    new_description: "".to_string(),
                },
                shelf: ShelfAssignment {
                    assignment_type: "existing".to_string(),
                    existing_id: 1, // Will be the Core shelf
                    new_name: "".to_string(),
                    new_description: "".to_string(),
                },
            },
            ItemAnalysis {
                id: "2".to_string(),
                description: "Project documentation and README".to_string(),
                suggested_name: "README.md".to_string(),
                is_opaque_directory: false,
                cabinet: CabinetAssignment {
                    assignment_type: "new".to_string(),
                    existing_id: 0,
                    new_name: "Documentation".to_string(),
                    new_description: "Project documentation files".to_string(),
                },
                shelf: ShelfAssignment {
                    assignment_type: "new".to_string(),
                    existing_id: 0,
                    new_name: "Root Docs".to_string(),
                    new_description: "Main documentation files".to_string(),
                },
            },
        ],
    };

    // Create mock LLM provider with the response
    let mock_provider = LLMProvider::new_mock(vec![serde_json::to_string(&mock_response).unwrap()]);

    // Create batch processor with mock provider
    let batch_processor = BatchProcessor::new(mock_provider, base_path.clone());

    // Run the actual batch processing
    let result = batch_processor.process_items_sequentially(items).await;

    // Verify processing succeeded
    assert!(
        result.is_ok(),
        "Batch processing should succeed: {:?}",
        result
    );

    // Verify database was created and populated
    assert!(Database::exists(&base_path), "Database should exist");
    let database = Database::open_or_create(&base_path).unwrap();

    // Check that cabinets were created
    let cabinets = database.list_cabinets().unwrap();
    assert_eq!(cabinets.len(), 2, "Should have 2 cabinets");

    let source_cabinet = cabinets
        .iter()
        .find(|c| c.name == "Source Code")
        .expect("Should have Source Code cabinet");
    let docs_cabinet = cabinets
        .iter()
        .find(|c| c.name == "Documentation")
        .expect("Should have Documentation cabinet");

    // Check that shelves were created
    let source_shelves = database.list_shelves(Some(source_cabinet.id)).unwrap();
    assert_eq!(source_shelves.len(), 1, "Source Code should have 1 shelf");
    assert_eq!(source_shelves[0].name, "Core");

    let docs_shelves = database.list_shelves(Some(docs_cabinet.id)).unwrap();
    assert_eq!(docs_shelves.len(), 1, "Documentation should have 1 shelf");
    assert_eq!(docs_shelves[0].name, "Root Docs");

    // Check that items were processed and stored
    let processed_paths = database.get_processed_paths().unwrap();
    assert_eq!(processed_paths.len(), 3, "Should have processed 3 items");

    let main_rs_path = base_path.join("src/main.rs").to_string_lossy().to_string();
    let lib_rs_path = base_path.join("src/lib.rs").to_string_lossy().to_string();
    let readme_path = base_path.join("README.md").to_string_lossy().to_string();

    assert!(
        processed_paths.contains(&main_rs_path),
        "Should have processed main.rs"
    );
    assert!(
        processed_paths.contains(&lib_rs_path),
        "Should have processed lib.rs"
    );
    assert!(
        processed_paths.contains(&readme_path),
        "Should have processed README.md"
    );

    // Verify specific item details
    let main_item = database
        .get_item_by_path(&main_rs_path)
        .unwrap()
        .expect("main.rs should be stored");
    assert_eq!(main_item.original_name, "main.rs");
    assert_eq!(main_item.description, "Main Rust application entry point");
    assert_eq!(main_item.file_type, "text/rust");
    assert!(!main_item.is_opaque_dir);
    assert_eq!(main_item.shelf_id, source_shelves[0].id);

    let readme_item = database
        .get_item_by_path(&readme_path)
        .unwrap()
        .expect("README.md should be stored");
    assert_eq!(readme_item.original_name, "README.md");
    assert_eq!(readme_item.suggested_name, Some("README.md".to_string()));
    assert_eq!(readme_item.description, "Project documentation and README");
    assert_eq!(readme_item.file_type, "text/markdown");
    assert_eq!(readme_item.shelf_id, docs_shelves[0].id);

    // Test resume capability - verify we can restart
    let new_db = Database::open_or_create(&base_path).unwrap();
    let resume_paths = new_db.get_processed_paths().unwrap();
    assert_eq!(
        resume_paths.len(),
        3,
        "Resume should find same processed items"
    );

    // Verify database state remains consistent after restart
    let final_cabinets = new_db.list_cabinets().unwrap();
    assert_eq!(
        final_cabinets.len(),
        2,
        "Should still have 2 cabinets after restart"
    );
}
