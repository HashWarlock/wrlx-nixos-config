use super::{DbPool, LessonRecord, MemoryRecord};
use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

/// Escape a search string for safe use in FTS5 MATCH queries.
/// Wraps the input in double quotes and escapes internal quotes.
/// This treats the entire input as a literal phrase, preventing
/// FTS5 special characters (", *, :, -, (, )) from being interpreted.
fn escape_fts5_query(input: &str) -> String {
    // FTS5 treats content inside double quotes as a literal phrase
    // Internal quotes need to be doubled to escape them
    let escaped = input.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

/// Repository for memory CRUD operations
pub struct MemoryRepository {
    db: DbPool,
}

impl MemoryRepository {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }

    /// Store a new memory entry
    pub fn store(
        &self,
        layer: i32,
        content: &str,
        metadata_json: &str,
        ttl_ms: Option<i64>,
    ) -> Result<String, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now_ms = Utc::now().timestamp_millis();
        let expires_at_ms = ttl_ms.map(|ttl| now_ms + ttl);

        let conn = self.db.lock().unwrap();
        conn.execute(
            "INSERT INTO memories (id, layer, content, metadata_json, created_at_ms, last_accessed_ms, expires_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, layer, content, metadata_json, now_ms, now_ms, expires_at_ms],
        )?;

        Ok(id)
    }

    /// Query memories by layer, with optional limit
    pub fn query(
        &self,
        layers: &[i32],
        limit: i32,
    ) -> Result<Vec<MemoryRecord>, rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        let limit = if limit <= 0 { 50 } else { limit };

        if layers.is_empty() {
            // Query all layers
            let mut stmt = conn.prepare(
                "SELECT id, layer, content, metadata_json, created_at_ms, last_accessed_ms, expires_at_ms, pinned, access_count
                 FROM memories
                 ORDER BY last_accessed_ms DESC
                 LIMIT ?1"
            )?;

            let rows = stmt.query_map([limit], |row| MemoryRecord::from_row(row))?;
            rows.collect()
        } else {
            // Build dynamic IN clause
            let placeholders: String = (0..layers.len())
                .map(|i| format!("?{}", i + 1))
                .collect::<Vec<_>>()
                .join(",");

            let sql = format!(
                "SELECT id, layer, content, metadata_json, created_at_ms, last_accessed_ms, expires_at_ms, pinned, access_count
                 FROM memories
                 WHERE layer IN ({})
                 ORDER BY last_accessed_ms DESC
                 LIMIT ?{}",
                placeholders,
                layers.len() + 1
            );

            let mut stmt = conn.prepare(&sql)?;

            // Build params: layers + limit
            let mut params_vec: Vec<rusqlite::types::Value> = layers
                .iter()
                .map(|&l| rusqlite::types::Value::Integer(l as i64))
                .collect();
            params_vec.push(rusqlite::types::Value::Integer(limit as i64));

            let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter()), |row| {
                MemoryRecord::from_row(row)
            })?;
            rows.collect()
        }
    }

    /// Full-text search across memories
    pub fn search(
        &self,
        text: &str,
        layers: &[i32],
        limit: i32,
    ) -> Result<Vec<MemoryRecord>, rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        let limit = if limit <= 0 { 50 } else { limit };

        let layer_filter = if layers.is_empty() {
            String::new()
        } else {
            let placeholders: String = layers
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 2))
                .collect::<Vec<_>>()
                .join(",");
            format!("AND m.layer IN ({})", placeholders)
        };

        let sql = format!(
            "SELECT m.id, m.layer, m.content, m.metadata_json, m.created_at_ms, m.last_accessed_ms, m.expires_at_ms, m.pinned, m.access_count
             FROM memories m
             JOIN memories_fts f ON m.rowid = f.rowid
             WHERE memories_fts MATCH ?1
             {}
             ORDER BY rank
             LIMIT ?{}",
            layer_filter,
            layers.len() + 2
        );

        let mut stmt = conn.prepare(&sql)?;

        // Build params: search text + layers + limit
        let mut params_vec: Vec<rusqlite::types::Value> = vec![
            rusqlite::types::Value::Text(escape_fts5_query(text))
        ];
        for &layer in layers {
            params_vec.push(rusqlite::types::Value::Integer(layer as i64));
        }
        params_vec.push(rusqlite::types::Value::Integer(limit as i64));

        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter()), |row| {
            MemoryRecord::from_row(row)
        })?;
        rows.collect()
    }

    /// Update last accessed time and increment access count
    pub fn update_access(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        let now_ms = Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE memories SET last_accessed_ms = ?1, access_count = access_count + 1 WHERE id = ?2",
            params![now_ms, id],
        )?;
        Ok(())
    }

    /// Pin or unpin a memory
    pub fn pin(&self, id: &str, pinned: bool) -> Result<bool, rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        let affected = conn.execute(
            "UPDATE memories SET pinned = ?1 WHERE id = ?2",
            params![pinned, id],
        )?;
        Ok(affected > 0)
    }

    /// Get memories that are candidates for forgetting
    pub fn get_forget_candidates(&self) -> Result<Vec<MemoryRecord>, rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        let now_ms = Utc::now().timestamp_millis();
        let thirty_days_ms = 30 * 24 * 60 * 60 * 1000_i64;
        let thirty_days_ago = now_ms - thirty_days_ms;

        let mut stmt = conn.prepare(
            "SELECT id, layer, content, metadata_json, created_at_ms, last_accessed_ms, expires_at_ms, pinned, access_count
             FROM memories
             WHERE pinned = 0 AND (
                 (expires_at_ms IS NOT NULL AND expires_at_ms < ?1) OR
                 (layer != 3 AND last_accessed_ms < ?2)
             )
             ORDER BY last_accessed_ms ASC
             LIMIT 100"
        )?;

        let rows = stmt.query_map(params![now_ms, thirty_days_ago], |row| {
            MemoryRecord::from_row(row)
        })?;
        rows.collect()
    }

    /// Delete multiple memories by ID (only if not pinned)
    pub fn delete_many(&self, ids: &[String]) -> Result<usize, rusqlite::Error> {
        if ids.is_empty() {
            return Ok(0);
        }

        let conn = self.db.lock().unwrap();
        let placeholders: String = (0..ids.len())
            .map(|i| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");

        let sql = format!(
            "DELETE FROM memories WHERE id IN ({}) AND pinned = 0",
            placeholders
        );

        let params: Vec<&dyn rusqlite::ToSql> = ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
        let deleted = conn.execute(&sql, params.as_slice())?;
        Ok(deleted)
    }

    /// Get a single memory by ID
    #[allow(dead_code)]
    pub fn get_by_id(&self, id: &str) -> Result<Option<MemoryRecord>, rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, layer, content, metadata_json, created_at_ms, last_accessed_ms, expires_at_ms, pinned, access_count
             FROM memories
             WHERE id = ?1"
        )?;

        let mut rows = stmt.query_map([id], |row| MemoryRecord::from_row(row))?;
        match rows.next() {
            Some(Ok(record)) => Ok(Some(record)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{init_memory_db, layer};

    fn setup_test_repo() -> MemoryRepository {
        let db = init_memory_db().unwrap();
        MemoryRepository::new(db)
    }

    #[test]
    fn test_store_and_query() {
        let repo = setup_test_repo();

        let id = repo.store(
            layer::WORKING,
            "Test memory content",
            "{}",
            None,
        ).unwrap();

        assert!(!id.is_empty());

        let results = repo.query(&[], 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "Test memory content");
        assert_eq!(results[0].layer, layer::WORKING);
    }

    #[test]
    fn test_query_by_layer() {
        let repo = setup_test_repo();

        repo.store(layer::WORKING, "Working memory", "{}", None).unwrap();
        repo.store(layer::FACTS, "System fact", "{}", None).unwrap();
        repo.store(layer::PREFERENCES, "User preference", "{}", None).unwrap();

        let working = repo.query(&[layer::WORKING], 10).unwrap();
        assert_eq!(working.len(), 1);
        assert_eq!(working[0].content, "Working memory");

        let facts_and_prefs = repo.query(&[layer::FACTS, layer::PREFERENCES], 10).unwrap();
        assert_eq!(facts_and_prefs.len(), 2);
    }

    #[test]
    fn test_full_text_search() {
        let repo = setup_test_repo();

        repo.store(layer::WORKING, "NixOS configuration guide", "{}", None).unwrap();
        repo.store(layer::WORKING, "Docker container setup", "{}", None).unwrap();
        repo.store(layer::WORKING, "NixOS flake tutorial", "{}", None).unwrap();

        let results = repo.search("NixOS", &[], 10).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.content.contains("NixOS")));
    }

    #[test]
    fn test_fts5_escape_special_chars() {
        // Test the escape function directly
        assert_eq!(escape_fts5_query("simple"), "\"simple\"");
        assert_eq!(escape_fts5_query("hello world"), "\"hello world\"");
        assert_eq!(escape_fts5_query("word*"), "\"word*\"");
        assert_eq!(escape_fts5_query("foo:bar"), "\"foo:bar\"");
        assert_eq!(escape_fts5_query("-excluded"), "\"-excluded\"");
        assert_eq!(escape_fts5_query("(grouped)"), "\"(grouped)\"");
        // Quotes should be doubled
        assert_eq!(escape_fts5_query("with \"quotes\""), "\"with \"\"quotes\"\"\"");
        assert_eq!(escape_fts5_query("\""), "\"\"\"\"");
    }

    #[test]
    fn test_search_with_special_characters() {
        let repo = setup_test_repo();

        // Store content that includes special FTS5 characters
        repo.store(layer::WORKING, "Using word* wildcards", "{}", None).unwrap();
        repo.store(layer::WORKING, "Config key:value pairs", "{}", None).unwrap();
        repo.store(layer::WORKING, "Exclude -things from search", "{}", None).unwrap();
        repo.store(layer::WORKING, "Grouping (with) parentheses", "{}", None).unwrap();

        // These searches should not fail with FTS5 syntax errors
        let results = repo.search("word*", &[], 10).unwrap();
        assert_eq!(results.len(), 1);

        let results = repo.search("key:value", &[], 10).unwrap();
        assert_eq!(results.len(), 1);

        let results = repo.search("-things", &[], 10).unwrap();
        assert_eq!(results.len(), 1);

        let results = repo.search("(with)", &[], 10).unwrap();
        assert_eq!(results.len(), 1);

        // Search with quotes in content
        repo.store(layer::WORKING, "She said \"hello\"", "{}", None).unwrap();
        let results = repo.search("\"hello\"", &[], 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_pin_memory() {
        let repo = setup_test_repo();

        let id = repo.store(layer::WORKING, "Important memory", "{}", None).unwrap();

        let success = repo.pin(&id, true).unwrap();
        assert!(success);

        let record = repo.get_by_id(&id).unwrap().unwrap();
        assert!(record.pinned);

        // Unpin
        repo.pin(&id, false).unwrap();
        let record = repo.get_by_id(&id).unwrap().unwrap();
        assert!(!record.pinned);
    }

    #[test]
    fn test_delete_respects_pinned() {
        let repo = setup_test_repo();

        let id1 = repo.store(layer::WORKING, "Memory 1", "{}", None).unwrap();
        let id2 = repo.store(layer::WORKING, "Memory 2", "{}", None).unwrap();

        // Pin one
        repo.pin(&id1, true).unwrap();

        // Try to delete both
        let deleted = repo.delete_many(&[id1.clone(), id2.clone()]).unwrap();
        assert_eq!(deleted, 1); // Only unpinned one should be deleted

        // Pinned should still exist
        assert!(repo.get_by_id(&id1).unwrap().is_some());
        assert!(repo.get_by_id(&id2).unwrap().is_none());
    }

    #[test]
    fn test_forget_candidates_expired() {
        let repo = setup_test_repo();

        // Create memory with very short TTL (negative = already expired)
        let id = repo.store(layer::WORKING, "Expired content", "{}", Some(-1000)).unwrap();
        repo.store(layer::WORKING, "Valid content", "{}", None).unwrap();

        let candidates = repo.get_forget_candidates().unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, id);
    }

    #[test]
    fn test_update_access() {
        let repo = setup_test_repo();

        let id = repo.store(layer::WORKING, "Test", "{}", None).unwrap();
        let before = repo.get_by_id(&id).unwrap().unwrap();
        assert_eq!(before.access_count, 0);

        std::thread::sleep(std::time::Duration::from_millis(10));
        repo.update_access(&id).unwrap();

        let after = repo.get_by_id(&id).unwrap().unwrap();
        assert_eq!(after.access_count, 1);
        assert!(after.last_accessed_ms > before.last_accessed_ms);
    }
}

/// Repository for lessons learned CRUD operations
pub struct LessonsRepository {
    db: DbPool,
}

impl LessonsRepository {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }

    /// Store a new lesson learned
    pub fn store(
        &self,
        trigger_pattern: &str,
        solution: &str,
        context: &str,
    ) -> Result<String, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now_ms = Utc::now().timestamp_millis();

        let conn = self.db.lock().unwrap();
        conn.execute(
            "INSERT INTO lessons_learned (id, trigger_pattern, solution, context, created_at_ms, last_used_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, trigger_pattern, solution, context, now_ms, now_ms],
        )?;

        Ok(id)
    }

    /// Search for lessons matching a trigger pattern
    pub fn search(&self, query: &str, limit: i32) -> Result<Vec<LessonRecord>, rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        let limit = if limit <= 0 { 10 } else { limit };

        let mut stmt = conn.prepare(
            "SELECT l.id, l.trigger_pattern, l.solution, l.context, l.success_count, l.failure_count, l.created_at_ms, l.last_used_ms
             FROM lessons_learned l
             JOIN lessons_fts f ON l.rowid = f.rowid
             WHERE lessons_fts MATCH ?1
             ORDER BY l.success_count DESC, rank
             LIMIT ?2"
        )?;

        let escaped_query = escape_fts5_query(query);
        let rows = stmt.query_map(params![escaped_query, limit], |row| LessonRecord::from_row(row))?;
        rows.collect()
    }

    /// Find lessons by exact trigger pattern match
    pub fn find_by_trigger(&self, trigger_pattern: &str) -> Result<Option<LessonRecord>, rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, trigger_pattern, solution, context, success_count, failure_count, created_at_ms, last_used_ms
             FROM lessons_learned
             WHERE trigger_pattern = ?1
             ORDER BY success_count DESC
             LIMIT 1"
        )?;

        let mut rows = stmt.query_map([trigger_pattern], |row| LessonRecord::from_row(row))?;
        match rows.next() {
            Some(Ok(record)) => Ok(Some(record)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    /// Record a successful use of a lesson
    pub fn record_success(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        let now_ms = Utc::now().timestamp_millis();
        let affected = conn.execute(
            "UPDATE lessons_learned SET success_count = success_count + 1, last_used_ms = ?1 WHERE id = ?2",
            params![now_ms, id],
        )?;
        Ok(affected > 0)
    }

    /// Record a failed use of a lesson
    pub fn record_failure(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        let now_ms = Utc::now().timestamp_millis();
        let affected = conn.execute(
            "UPDATE lessons_learned SET failure_count = failure_count + 1, last_used_ms = ?1 WHERE id = ?2",
            params![now_ms, id],
        )?;
        Ok(affected > 0)
    }

    /// Get all lessons ordered by confidence score
    pub fn get_all(&self, limit: i32) -> Result<Vec<LessonRecord>, rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        let limit = if limit <= 0 { 50 } else { limit };

        let mut stmt = conn.prepare(
            "SELECT id, trigger_pattern, solution, context, success_count, failure_count, created_at_ms, last_used_ms
             FROM lessons_learned
             ORDER BY (CAST(success_count AS REAL) / MAX(success_count + failure_count, 1)) DESC, last_used_ms DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map([limit], |row| LessonRecord::from_row(row))?;
        rows.collect()
    }

    /// Delete a lesson by ID
    pub fn delete(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        let affected = conn.execute("DELETE FROM lessons_learned WHERE id = ?1", [id])?;
        Ok(affected > 0)
    }

    /// Get a lesson by ID
    pub fn get_by_id(&self, id: &str) -> Result<Option<LessonRecord>, rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, trigger_pattern, solution, context, success_count, failure_count, created_at_ms, last_used_ms
             FROM lessons_learned
             WHERE id = ?1"
        )?;

        let mut rows = stmt.query_map([id], |row| LessonRecord::from_row(row))?;
        match rows.next() {
            Some(Ok(record)) => Ok(Some(record)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    /// Update an existing lesson's solution
    pub fn update_solution(&self, id: &str, solution: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        let now_ms = Utc::now().timestamp_millis();
        let affected = conn.execute(
            "UPDATE lessons_learned SET solution = ?1, last_used_ms = ?2 WHERE id = ?3",
            params![solution, now_ms, id],
        )?;
        Ok(affected > 0)
    }
}

#[cfg(test)]
mod lessons_tests {
    use super::*;
    use crate::db::init_memory_db;

    fn setup_lessons_repo() -> LessonsRepository {
        let db = init_memory_db().unwrap();
        LessonsRepository::new(db)
    }

    #[test]
    fn test_store_and_search_lesson() {
        let repo = setup_lessons_repo();

        let id = repo.store(
            "install vscode",
            "Use vscode-fhs package on NixOS for better compatibility",
            "{}",
        ).unwrap();

        assert!(!id.is_empty());

        let results = repo.search("vscode", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].solution.contains("vscode-fhs"));
    }

    #[test]
    fn test_find_by_trigger() {
        let repo = setup_lessons_repo();

        repo.store("install chrome", "Use google-chrome package", "{}").unwrap();
        repo.store("install firefox", "Use firefox package", "{}").unwrap();

        let lesson = repo.find_by_trigger("install chrome").unwrap();
        assert!(lesson.is_some());
        assert!(lesson.unwrap().solution.contains("google-chrome"));

        let none = repo.find_by_trigger("install opera").unwrap();
        assert!(none.is_none());
    }

    #[test]
    fn test_success_failure_tracking() {
        let repo = setup_lessons_repo();

        let id = repo.store("test pattern", "test solution", "{}").unwrap();

        // Record successes
        repo.record_success(&id).unwrap();
        repo.record_success(&id).unwrap();
        repo.record_failure(&id).unwrap();

        let lessons = repo.get_all(10).unwrap();
        assert_eq!(lessons.len(), 1);
        assert_eq!(lessons[0].success_count, 3); // 1 initial + 2 recorded
        assert_eq!(lessons[0].failure_count, 1);

        // Confidence should be 3/4 = 0.75
        assert!((lessons[0].confidence() - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_update_solution() {
        let repo = setup_lessons_repo();

        let id = repo.store("pattern", "old solution", "{}").unwrap();
        repo.update_solution(&id, "new improved solution").unwrap();

        let lesson = repo.find_by_trigger("pattern").unwrap().unwrap();
        assert_eq!(lesson.solution, "new improved solution");
    }

    #[test]
    fn test_lessons_search_with_special_characters() {
        let repo = setup_lessons_repo();

        // Store lessons with special FTS5 characters
        repo.store("use glob*", "Use * wildcards for matching", "{}").unwrap();
        repo.store("config key:value", "Set key:value in config", "{}").unwrap();
        repo.store("-flag option", "Use -flag for options", "{}").unwrap();

        // These searches should not fail with FTS5 syntax errors
        let results = repo.search("glob*", 10).unwrap();
        assert_eq!(results.len(), 1);

        let results = repo.search("key:value", 10).unwrap();
        assert_eq!(results.len(), 1);

        let results = repo.search("-flag", 10).unwrap();
        assert_eq!(results.len(), 1);

        // Search with quotes
        repo.store("say \"hello\"", "greeting solution", "{}").unwrap();
        let results = repo.search("\"hello\"", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_get_by_id() {
        let repo = setup_lessons_repo();

        // Store a lesson
        let id = repo.store(
            "test trigger",
            "test solution",
            "{}",
        ).unwrap();

        // Get by ID should find it
        let lesson = repo.get_by_id(&id).unwrap();
        assert!(lesson.is_some());
        let lesson = lesson.unwrap();
        assert_eq!(lesson.id, id);
        assert_eq!(lesson.trigger_pattern, "test trigger");
        assert_eq!(lesson.solution, "test solution");

        // Get non-existent ID should return None
        let none = repo.get_by_id("non-existent-id").unwrap();
        assert!(none.is_none());
    }
}
