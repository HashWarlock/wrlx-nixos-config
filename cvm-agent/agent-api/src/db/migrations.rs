use rusqlite::{Connection, Result};

/// Run all database migrations
pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        -- Memory storage table
        CREATE TABLE IF NOT EXISTS memories (
            id TEXT PRIMARY KEY,
            layer INTEGER NOT NULL,
            content TEXT NOT NULL,
            metadata_json TEXT DEFAULT '{}',
            created_at_ms INTEGER NOT NULL,
            last_accessed_ms INTEGER NOT NULL,
            expires_at_ms INTEGER,
            pinned INTEGER DEFAULT 0,
            access_count INTEGER DEFAULT 0
        );

        -- Indexes for common queries
        CREATE INDEX IF NOT EXISTS idx_memories_layer ON memories(layer);
        CREATE INDEX IF NOT EXISTS idx_memories_expires ON memories(expires_at_ms);
        CREATE INDEX IF NOT EXISTS idx_memories_pinned ON memories(pinned);
        CREATE INDEX IF NOT EXISTS idx_memories_last_accessed ON memories(last_accessed_ms);

        -- Full-text search virtual table
        CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
            content,
            content='memories',
            content_rowid='rowid'
        );

        -- Triggers to keep FTS in sync with main table
        CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
            INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
        END;

        CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
            INSERT INTO memories_fts(memories_fts, rowid, content) VALUES('delete', old.rowid, old.content);
        END;

        CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
            INSERT INTO memories_fts(memories_fts, rowid, content) VALUES('delete', old.rowid, old.content);
            INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
        END;
        "
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_run_successfully() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        // Verify table exists
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='memories'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_migrations_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap(); // Should not fail
    }
}
