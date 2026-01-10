use rusqlite::{Result, Row};

/// A memory record as stored in the database
#[derive(Debug, Clone)]
pub struct MemoryRecord {
    pub id: String,
    pub layer: i32,
    pub content: String,
    pub metadata_json: String,
    pub created_at_ms: i64,
    pub last_accessed_ms: i64,
    pub expires_at_ms: Option<i64>,
    pub pinned: bool,
    pub access_count: i32,
}

impl MemoryRecord {
    /// Parse a MemoryRecord from a database row
    /// Expected column order: id, layer, content, metadata_json, created_at_ms,
    /// last_accessed_ms, expires_at_ms, pinned, access_count
    pub fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            layer: row.get(1)?,
            content: row.get(2)?,
            metadata_json: row.get(3)?,
            created_at_ms: row.get(4)?,
            last_accessed_ms: row.get(5)?,
            expires_at_ms: row.get(6)?,
            pinned: row.get(7)?,
            access_count: row.get(8)?,
        })
    }
}

/// Memory layer constants matching proto enum values
pub mod layer {
    pub const UNSPECIFIED: i32 = 0;
    pub const WORKING: i32 = 1;
    pub const ARCHIVE: i32 = 2;
    pub const FACTS: i32 = 3;
    pub const PREFERENCES: i32 = 4;
}
