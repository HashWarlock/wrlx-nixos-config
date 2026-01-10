use super::{DbPool, MemoryRecord};
use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

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
            rusqlite::types::Value::Text(text.to_string())
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
