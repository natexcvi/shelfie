use chrono::Utc;
use shelfie::database::{Database, Item};
use tempfile::TempDir;

fn setup_test_db() -> (TempDir, Database) {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open_or_create(temp_dir.path()).unwrap();
    (temp_dir, db)
}

#[test]
fn test_database_creation() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open_or_create(temp_dir.path()).unwrap();
    assert!(Database::exists(temp_dir.path()));
}

#[test]
fn test_create_and_get_cabinet() {
    let (_dir, db) = setup_test_db();
    
    let cabinet_id = db.create_cabinet("Test Cabinet", "A test cabinet description").unwrap();
    assert!(cabinet_id > 0);
    
    let cabinet = db.get_cabinet_by_name("Test Cabinet").unwrap().unwrap();
    assert_eq!(cabinet.name, "Test Cabinet");
    assert_eq!(cabinet.description, "A test cabinet description");
    assert_eq!(cabinet.id, cabinet_id);
}

#[test]
fn test_cabinet_unique_constraint() {
    let (_dir, db) = setup_test_db();
    
    db.create_cabinet("Unique Cabinet", "First").unwrap();
    let result = db.create_cabinet("Unique Cabinet", "Second");
    
    assert!(result.is_err());
}

#[test]
fn test_list_cabinets() {
    let (_dir, db) = setup_test_db();
    
    db.create_cabinet("Cabinet A", "Description A").unwrap();
    db.create_cabinet("Cabinet B", "Description B").unwrap();
    db.create_cabinet("Cabinet C", "Description C").unwrap();
    
    let cabinets = db.list_cabinets().unwrap();
    assert_eq!(cabinets.len(), 3);
    
    let names: Vec<String> = cabinets.iter().map(|c| c.name.clone()).collect();
    assert!(names.contains(&"Cabinet A".to_string()));
    assert!(names.contains(&"Cabinet B".to_string()));
    assert!(names.contains(&"Cabinet C".to_string()));
}

#[test]
fn test_create_and_get_shelf() {
    let (_dir, db) = setup_test_db();
    
    let cabinet_id = db.create_cabinet("Parent Cabinet", "Parent").unwrap();
    let shelf_id = db.create_shelf(cabinet_id, "Test Shelf", "A test shelf").unwrap();
    assert!(shelf_id > 0);
    
    let shelf = db.get_shelf_by_name(cabinet_id, "Test Shelf").unwrap().unwrap();
    assert_eq!(shelf.name, "Test Shelf");
    assert_eq!(shelf.description, "A test shelf");
    assert_eq!(shelf.cabinet_id, cabinet_id);
    assert_eq!(shelf.id, shelf_id);
}

#[test]
fn test_shelf_unique_within_cabinet() {
    let (_dir, db) = setup_test_db();
    
    let cabinet1_id = db.create_cabinet("Cabinet 1", "First").unwrap();
    let cabinet2_id = db.create_cabinet("Cabinet 2", "Second").unwrap();
    
    db.create_shelf(cabinet1_id, "Shelf Name", "In cabinet 1").unwrap();
    db.create_shelf(cabinet2_id, "Shelf Name", "In cabinet 2").unwrap();
    
    let result = db.create_shelf(cabinet1_id, "Shelf Name", "Duplicate in cabinet 1");
    assert!(result.is_err());
}

#[test]
fn test_list_shelves() {
    let (_dir, db) = setup_test_db();
    
    let cabinet1_id = db.create_cabinet("Cabinet 1", "First").unwrap();
    let cabinet2_id = db.create_cabinet("Cabinet 2", "Second").unwrap();
    
    db.create_shelf(cabinet1_id, "Shelf 1A", "First shelf in cabinet 1").unwrap();
    db.create_shelf(cabinet1_id, "Shelf 1B", "Second shelf in cabinet 1").unwrap();
    db.create_shelf(cabinet2_id, "Shelf 2A", "First shelf in cabinet 2").unwrap();
    
    let all_shelves = db.list_shelves(None).unwrap();
    assert_eq!(all_shelves.len(), 3);
    
    let cabinet1_shelves = db.list_shelves(Some(cabinet1_id)).unwrap();
    assert_eq!(cabinet1_shelves.len(), 2);
    
    let cabinet2_shelves = db.list_shelves(Some(cabinet2_id)).unwrap();
    assert_eq!(cabinet2_shelves.len(), 1);
}

#[test]
fn test_insert_and_get_item() {
    let (_dir, db) = setup_test_db();
    
    let cabinet_id = db.create_cabinet("Item Cabinet", "For items").unwrap();
    let shelf_id = db.create_shelf(cabinet_id, "Item Shelf", "For items").unwrap();
    
    let item = Item {
        id: None,
        shelf_id,
        path: "/test/path/file.txt".to_string(),
        original_name: "file.txt".to_string(),
        suggested_name: Some("better_name.txt".to_string()),
        description: "A test file".to_string(),
        file_type: "text/plain".to_string(),
        is_opaque_dir: false,
        processed_at: Utc::now(),
    };
    
    let item_id = db.insert_item(&item).unwrap();
    assert!(item_id > 0);
    
    let retrieved = db.get_item_by_path("/test/path/file.txt").unwrap().unwrap();
    assert_eq!(retrieved.shelf_id, shelf_id);
    assert_eq!(retrieved.original_name, "file.txt");
    assert_eq!(retrieved.suggested_name, Some("better_name.txt".to_string()));
    assert_eq!(retrieved.description, "A test file");
    assert!(!retrieved.is_opaque_dir);
}

#[test]
fn test_item_unique_path_constraint() {
    let (_dir, db) = setup_test_db();
    
    let cabinet_id = db.create_cabinet("Cabinet", "Test").unwrap();
    let shelf_id = db.create_shelf(cabinet_id, "Shelf", "Test").unwrap();
    
    let item = Item {
        id: None,
        shelf_id,
        path: "/unique/path".to_string(),
        original_name: "file.txt".to_string(),
        suggested_name: None,
        description: "First".to_string(),
        file_type: "text".to_string(),
        is_opaque_dir: false,
        processed_at: Utc::now(),
    };
    
    db.insert_item(&item).unwrap();
    
    let duplicate = Item {
        id: None,
        shelf_id,
        path: "/unique/path".to_string(),
        original_name: "other.txt".to_string(),
        suggested_name: None,
        description: "Second".to_string(),
        file_type: "text".to_string(),
        is_opaque_dir: false,
        processed_at: Utc::now(),
    };
    
    let result = db.insert_item(&duplicate);
    assert!(result.is_err());
}

#[test]
fn test_list_items_by_shelf() {
    let (_dir, db) = setup_test_db();
    
    let cabinet_id = db.create_cabinet("Cabinet", "Test").unwrap();
    let shelf1_id = db.create_shelf(cabinet_id, "Shelf 1", "First").unwrap();
    let shelf2_id = db.create_shelf(cabinet_id, "Shelf 2", "Second").unwrap();
    
    for i in 0..3 {
        let item = Item {
            id: None,
            shelf_id: shelf1_id,
            path: format!("/shelf1/file{}.txt", i),
            original_name: format!("file{}.txt", i),
            suggested_name: None,
            description: format!("File {}", i),
            file_type: "text".to_string(),
            is_opaque_dir: false,
            processed_at: Utc::now(),
        };
        db.insert_item(&item).unwrap();
    }
    
    for i in 0..2 {
        let item = Item {
            id: None,
            shelf_id: shelf2_id,
            path: format!("/shelf2/file{}.txt", i),
            original_name: format!("file{}.txt", i),
            suggested_name: None,
            description: format!("File {}", i),
            file_type: "text".to_string(),
            is_opaque_dir: false,
            processed_at: Utc::now(),
        };
        db.insert_item(&item).unwrap();
    }
    
    let all_items = db.get_processed_paths().unwrap();
    assert_eq!(all_items.len(), 5);
}

#[test]
fn test_update_item_content() {
    let (_dir, db) = setup_test_db();
    
    let cabinet_id = db.create_cabinet("Cabinet", "Test").unwrap();
    let shelf_id = db.create_shelf(cabinet_id, "Shelf", "Test").unwrap();
    
    let item = Item {
        id: None,
        shelf_id,
        path: "/test/file.txt".to_string(),
        original_name: "file.txt".to_string(),
        suggested_name: None,
        description: "Original description".to_string(),
        file_type: "text".to_string(),
        is_opaque_dir: false,
        processed_at: Utc::now(),
    };
    
    let item_id = db.insert_item(&item).unwrap();
    
    db.update_item_content(
        item_id,
        "Updated description",
        "new_name.txt"
    ).unwrap();
    
    let updated = db.get_item_by_path("/test/file.txt").unwrap().unwrap();
    assert_eq!(updated.description, "Updated description");
    assert_eq!(updated.suggested_name, Some("new_name.txt".to_string()));
}

#[test]
fn test_get_processed_paths() {
    let (_dir, db) = setup_test_db();
    
    let cabinet_id = db.create_cabinet("Cabinet", "Test").unwrap();
    let shelf_id = db.create_shelf(cabinet_id, "Shelf", "Test").unwrap();
    
    let paths = vec!["/path1.txt", "/path2.txt", "/dir/path3.txt"];
    
    for path in &paths {
        let item = Item {
            id: None,
            shelf_id,
            path: path.to_string(),
            original_name: "file.txt".to_string(),
            suggested_name: None,
            description: "Test".to_string(),
            file_type: "text".to_string(),
            is_opaque_dir: false,
            processed_at: Utc::now(),
        };
        db.insert_item(&item).unwrap();
    }
    
    let processed = db.get_processed_paths().unwrap();
    assert_eq!(processed.len(), 3);
    
    for path in paths {
        assert!(processed.contains(&path.to_string()));
    }
}

#[test]
fn test_opaque_directory_flag() {
    let (_dir, db) = setup_test_db();
    
    let cabinet_id = db.create_cabinet("Cabinet", "Test").unwrap();
    let shelf_id = db.create_shelf(cabinet_id, "Shelf", "Test").unwrap();
    
    let opaque_dir = Item {
        id: None,
        shelf_id,
        path: "/node_modules".to_string(),
        original_name: "node_modules".to_string(),
        suggested_name: None,
        description: "Node dependencies".to_string(),
        file_type: "directory".to_string(),
        is_opaque_dir: true,
        processed_at: Utc::now(),
    };
    
    db.insert_item(&opaque_dir).unwrap();
    
    let retrieved = db.get_item_by_path("/node_modules").unwrap().unwrap();
    assert!(retrieved.is_opaque_dir);
    assert_eq!(retrieved.file_type, "directory");
}

#[test]
fn test_foreign_key_constraints() {
    let (_dir, db) = setup_test_db();
    
    let invalid_cabinet_id = 99999;
    let result = db.create_shelf(invalid_cabinet_id, "Orphan Shelf", "No parent");
    assert!(result.is_err());
    
    let cabinet_id = db.create_cabinet("Cabinet", "Test").unwrap();
    let shelf_id = db.create_shelf(cabinet_id, "Shelf", "Test").unwrap();
    
    let invalid_shelf_id = 99999;
    let item = Item {
        id: None,
        shelf_id: invalid_shelf_id,
        path: "/test.txt".to_string(),
        original_name: "test.txt".to_string(),
        suggested_name: None,
        description: "Test".to_string(),
        file_type: "text".to_string(),
        is_opaque_dir: false,
        processed_at: Utc::now(),
    };
    
    let result = db.insert_item(&item);
    assert!(result.is_err());
}