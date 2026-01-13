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
#[allow(dead_code)]
pub mod layer {
    pub const UNSPECIFIED: i32 = 0;
    pub const WORKING: i32 = 1;
    pub const ARCHIVE: i32 = 2;
    pub const FACTS: i32 = 3;
    pub const PREFERENCES: i32 = 4;
}

/// A lesson learned record as stored in the database
/// Tracks problem -> solution mappings that the agent has discovered
#[derive(Debug, Clone)]
pub struct LessonRecord {
    pub id: String,
    pub trigger_pattern: String,
    pub solution: String,
    pub context: String,
    pub success_count: i32,
    pub failure_count: i32,
    pub created_at_ms: i64,
    pub last_used_ms: i64,
}

impl LessonRecord {
    /// Parse a LessonRecord from a database row
    /// Expected column order: id, trigger_pattern, solution, context,
    /// success_count, failure_count, created_at_ms, last_used_ms
    pub fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            trigger_pattern: row.get(1)?,
            solution: row.get(2)?,
            context: row.get(3)?,
            success_count: row.get(4)?,
            failure_count: row.get(5)?,
            created_at_ms: row.get(6)?,
            last_used_ms: row.get(7)?,
        })
    }

    /// Calculate confidence score (0.0 - 1.0) based on success/failure ratio
    pub fn confidence(&self) -> f64 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            return 0.5; // No data, neutral confidence
        }
        self.success_count as f64 / total as f64
    }
}
