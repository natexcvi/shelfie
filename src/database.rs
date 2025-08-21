use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub(crate) const DB_NAME: &str = ".fs_organizer.db";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cabinet {
    pub id: i64,
    pub name: String,
    pub description: String,
    #[serde(with = "chrono_serde")]
    pub created_at: DateTime<Utc>,
}

mod chrono_serde {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        dt.to_rfc3339().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        DateTime::parse_from_rfc3339(&s)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shelf {
    pub id: i64,
    pub cabinet_id: i64,
    pub name: String,
    pub description: String,
    #[serde(with = "chrono_serde")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: Option<i64>,
    pub shelf_id: i64,
    pub path: String,
    pub original_name: String,
    pub suggested_name: Option<String>,
    pub description: String,
    pub file_type: String,
    pub is_opaque_dir: bool,
    #[serde(with = "chrono_serde")]
    pub processed_at: DateTime<Utc>,
}

pub struct Database {
    pool: Pool<SqliteConnectionManager>,
}

impl Database {
    pub fn open_or_create(base_path: &Path) -> Result<Self> {
        let db_path = base_path.join(DB_NAME);
        let manager = SqliteConnectionManager::file(&db_path);
        let pool = Pool::new(manager).context("Failed to create connection pool")?;

        let db = Self { pool };
        db.initialize_schema()?;
        Ok(db)
    }

    pub fn exists(base_path: &Path) -> bool {
        base_path.join(DB_NAME).exists()
    }

    fn get_conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().context("Failed to get connection from pool")
    }

    fn initialize_schema(&self) -> Result<()> {
        let conn = self.get_conn()?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS cabinets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                description TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS shelves (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                cabinet_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (cabinet_id) REFERENCES cabinets(id),
                UNIQUE(cabinet_id, name)
            );

            CREATE TABLE IF NOT EXISTS items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                shelf_id INTEGER NOT NULL,
                path TEXT NOT NULL UNIQUE,
                original_name TEXT NOT NULL,
                suggested_name TEXT,
                description TEXT NOT NULL,
                file_type TEXT NOT NULL,
                is_opaque_dir BOOLEAN NOT NULL DEFAULT 0,
                processed_at TEXT NOT NULL,
                FOREIGN KEY (shelf_id) REFERENCES shelves(id)
            );

            CREATE TABLE IF NOT EXISTS processing_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_items_path ON items(path);
            CREATE INDEX IF NOT EXISTS idx_items_shelf ON items(shelf_id);
            CREATE INDEX IF NOT EXISTS idx_items_processed ON items(processed_at);
            ",
        )?;
        Ok(())
    }

    // Note: For simplicity, we'll make transaction operations work with individual connections
    // In a real application, you might want a more sophisticated transaction management system

    // Cabinet operations
    pub fn create_cabinet(&self, name: &str, description: &str) -> Result<i64> {
        let conn = self.get_conn()?;
        let created_at = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO cabinets (name, description, created_at) VALUES (?1, ?2, ?3)",
            params![name, description, created_at],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_cabinet_by_name(&self, name: &str) -> Result<Option<Cabinet>> {
        let conn = self.get_conn()?;
        let mut stmt = conn
            .prepare("SELECT id, name, description, created_at FROM cabinets WHERE name = ?1")?;

        stmt.query_row(params![name], |row| {
            Ok(Cabinet {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })
        .optional()
        .context("Failed to query cabinet")
    }

    pub fn list_cabinets(&self) -> Result<Vec<Cabinet>> {
        let conn = self.get_conn()?;
        let mut stmt = conn
            .prepare("SELECT id, name, description, created_at FROM cabinets ORDER BY name")?;

        let cabinets = stmt
            .query_map([], |row| {
                Ok(Cabinet {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                        .unwrap()
                        .with_timezone(&Utc),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(cabinets)
    }

    // Shelf operations
    pub fn create_shelf(&self, cabinet_id: i64, name: &str, description: &str) -> Result<i64> {
        let conn = self.get_conn()?;
        let created_at = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO shelves (cabinet_id, name, description, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![cabinet_id, name, description, created_at],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_shelf_by_name(&self, cabinet_id: i64, name: &str) -> Result<Option<Shelf>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, cabinet_id, name, description, created_at FROM shelves
             WHERE cabinet_id = ?1 AND name = ?2",
        )?;

        stmt.query_row(params![cabinet_id, name], |row| {
            Ok(Shelf {
                id: row.get(0)?,
                cabinet_id: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })
        .optional()
        .context("Failed to query shelf")
    }

    pub fn list_shelves(&self, cabinet_id: Option<i64>) -> Result<Vec<Shelf>> {
        let query = if let Some(cabinet_id) = cabinet_id {
            format!(
                "SELECT id, cabinet_id, name, description, created_at FROM shelves
                    WHERE cabinet_id = {} ORDER BY name",
                cabinet_id
            )
        } else {
            "SELECT id, cabinet_id, name, description, created_at FROM shelves ORDER BY cabinet_id, name".to_string()
        };

        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(&query)?;

        let shelves = stmt
            .query_map([], |row| {
                Ok(Shelf {
                    id: row.get(0)?,
                    cabinet_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .unwrap()
                        .with_timezone(&Utc),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(shelves)
    }

    // Item operations
    pub fn insert_item(&self, item: &Item) -> Result<i64> {
        let conn = self.get_conn()?;
        let processed_at = item.processed_at.to_rfc3339();
        conn.execute(
            "INSERT INTO items (shelf_id, path, original_name, suggested_name, description,
                              file_type, is_opaque_dir, processed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                item.shelf_id,
                item.path,
                item.original_name,
                item.suggested_name,
                item.description,
                item.file_type,
                item.is_opaque_dir,
                processed_at
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_item_content(
        &self,
        item_id: i64,
        description: &str,
        suggested_name: &str,
    ) -> Result<()> {
        self.get_conn()?.execute(
            "UPDATE items SET description = ?1, suggested_name = ?2, needs_content_read = 0
             WHERE id = ?3",
            params![description, suggested_name, item_id],
        )?;
        Ok(())
    }

    pub fn get_item_by_path(&self, path: &str) -> Result<Option<Item>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, shelf_id, path, original_name, suggested_name, description,
                    file_type, is_opaque_dir, processed_at
             FROM items WHERE path = ?1",
        )?;

        stmt.query_row(params![path], |row| {
            Ok(Item {
                id: Some(row.get(0)?),
                shelf_id: row.get(1)?,
                path: row.get(2)?,
                original_name: row.get(3)?,
                suggested_name: row.get(4)?,
                description: row.get(5)?,
                file_type: row.get(6)?,
                is_opaque_dir: row.get(7)?,
                processed_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })
        .optional()
        .context("Failed to query item")
    }

    pub fn list_items_needing_content(&self) -> Result<Vec<Item>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, shelf_id, path, original_name, suggested_name, description,
                    file_type, is_opaque_dir, processed_at
             FROM items WHERE needs_content_read = 1",
        )?;

        let items = stmt
            .query_map([], |row| {
                Ok(Item {
                    id: Some(row.get(0)?),
                    shelf_id: row.get(1)?,
                    path: row.get(2)?,
                    original_name: row.get(3)?,
                    suggested_name: row.get(4)?,
                    description: row.get(5)?,
                    file_type: row.get(6)?,
                    is_opaque_dir: row.get(7)?,
                    processed_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                        .unwrap()
                        .with_timezone(&Utc),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(items)
    }

    pub fn list_all_items(&self) -> Result<Vec<Item>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, shelf_id, path, original_name, suggested_name, description,
                    file_type, is_opaque_dir, processed_at
             FROM items ORDER BY shelf_id, original_name",
        )?;

        let items = stmt
            .query_map([], |row| {
                Ok(Item {
                    id: Some(row.get(0)?),
                    shelf_id: row.get(1)?,
                    path: row.get(2)?,
                    original_name: row.get(3)?,
                    suggested_name: row.get(4)?,
                    description: row.get(5)?,
                    file_type: row.get(6)?,
                    is_opaque_dir: row.get(7)?,
                    processed_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                        .unwrap()
                        .with_timezone(&Utc),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(items)
    }

    // Processing state operations
    pub fn set_processing_state(&self, key: &str, value: &str) -> Result<()> {
        self.get_conn()?.execute(
            "INSERT OR REPLACE INTO processing_state (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_processing_state(&self, key: &str) -> Result<Option<String>> {
        let conn = self.get_conn()?;
        let mut stmt = conn
            .prepare("SELECT value FROM processing_state WHERE key = ?1")?;

        stmt.query_row(params![key], |row| row.get(0))
            .optional()
            .context("Failed to query processing state")
    }

    // Helper to get all processed paths for incremental processing
    pub fn get_processed_paths(&self) -> Result<Vec<String>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare("SELECT path FROM items")?;
        let paths = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(paths)
    }

    // Update methods for plan refinement
    pub fn update_item_shelf(&self, item_id: i64, new_shelf_id: i64) -> Result<()> {
        self.get_conn()?.execute(
            "UPDATE items SET shelf_id = ?1 WHERE id = ?2",
            params![new_shelf_id, item_id],
        )?;
        Ok(())
    }

    pub fn update_cabinet(&self, cabinet_id: i64, name: &str, description: &str) -> Result<()> {
        self.get_conn()?.execute(
            "UPDATE cabinets SET name = ?1, description = ?2 WHERE id = ?3",
            params![name, description, cabinet_id],
        )?;
        Ok(())
    }

    pub fn update_shelf(&self, shelf_id: i64, name: &str, description: &str) -> Result<()> {
        self.get_conn()?.execute(
            "UPDATE shelves SET name = ?1, description = ?2 WHERE id = ?3",
            params![name, description, shelf_id],
        )?;
        Ok(())
    }

    pub fn delete_cabinet(&self, cabinet_id: i64) -> Result<()> {
        // First check if cabinet is empty
        let shelf_count: i64 = self.get_conn()?.query_row(
            "SELECT COUNT(*) FROM shelves WHERE cabinet_id = ?1",
            params![cabinet_id],
            |row| row.get(0),
        )?;

        if shelf_count > 0 {
            return Err(anyhow::anyhow!("Cannot delete cabinet: contains shelves"));
        }

        self.get_conn()?.execute(
            "DELETE FROM cabinets WHERE id = ?1",
            params![cabinet_id],
        )?;
        Ok(())
    }

    pub fn delete_shelf(&self, shelf_id: i64) -> Result<()> {
        // First check if shelf is empty
        let item_count: i64 = self.get_conn()?.query_row(
            "SELECT COUNT(*) FROM items WHERE shelf_id = ?1",
            params![shelf_id],
            |row| row.get(0),
        )?;

        if item_count > 0 {
            return Err(anyhow::anyhow!("Cannot delete shelf: contains items"));
        }

        self.get_conn()?.execute(
            "DELETE FROM shelves WHERE id = ?1",
            params![shelf_id],
        )?;
        Ok(())
    }
}
