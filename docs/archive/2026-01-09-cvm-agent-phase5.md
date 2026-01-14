# CVM Agent Phase 5: Memory & Polish - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add persistent memory system with active forgetting, implement adaptive UI states, and add robust error handling and recovery.

**Architecture:** SQLite-backed memory storage with four distinct layers (working context, conversation archive, system facts, learned preferences). Active forgetting uses a hybrid approach with automatic cleanup for stale data and human confirmation for important memories. UI adapts between hidden, quick input, and expanded panel states based on context.

**Tech Stack:** Rust + SQLite (rusqlite), React state machine for UI, gRPC streaming for memory queries

---

## Task 1: Add Memory Proto Definitions

**Files:**
- Modify: `cvm-agent/proto/agent.proto`

**Step 1: Write the proto definition tests**

We'll verify by checking generated code compiles. First, let's add the Memory proto definitions.

**Step 2: Add Memory message types and service**

Add to `agent.proto`:

```protobuf
// Memory Layer Types
enum MemoryLayer {
  MEMORY_LAYER_UNSPECIFIED = 0;
  MEMORY_LAYER_WORKING = 1;      // Current session context
  MEMORY_LAYER_ARCHIVE = 2;      // Summarized past conversations
  MEMORY_LAYER_FACTS = 3;        // System state facts
  MEMORY_LAYER_PREFERENCES = 4;  // Learned user preferences
}

// Memory Entry
message MemoryEntry {
  string id = 1;
  MemoryLayer layer = 2;
  string content = 3;
  map<string, string> metadata = 4;
  google.protobuf.Timestamp created_at = 5;
  google.protobuf.Timestamp last_accessed = 6;
  google.protobuf.Timestamp expires_at = 7;
  bool pinned = 8;
  int32 access_count = 9;
}

// Query memories
message MemoryQuery {
  repeated MemoryLayer layers = 1;  // Empty = all layers
  string search_text = 2;           // Full-text search
  map<string, string> filters = 3;  // Metadata filters
  int32 limit = 4;                  // Max results (default 50)
}

message MemoryResponse {
  repeated MemoryEntry entries = 1;
  int32 total_count = 2;
}

// Store memory
message StoreMemoryRequest {
  MemoryLayer layer = 1;
  string content = 2;
  map<string, string> metadata = 3;
  google.protobuf.Duration ttl = 4;  // Optional time-to-live
}

message StoreMemoryResponse {
  string id = 1;
}

// Forgetting candidates
message ForgetCandidate {
  MemoryEntry entry = 1;
  string reason = 2;  // Why this is a candidate
  bool auto_forget = 3;  // True if will be auto-deleted
}

message ForgetCandidatesResponse {
  repeated ForgetCandidate candidates = 1;
}

message ForgetDecision {
  repeated string forget_ids = 1;  // IDs to forget
  repeated string keep_ids = 2;    // IDs to keep (resets decay)
}

// Pin/unpin memories
message PinRequest {
  string id = 1;
  bool pinned = 2;
}

// Memory service
service MemoryService {
  // Query memories with filters
  rpc Query(MemoryQuery) returns (MemoryResponse);

  // Store a new memory
  rpc Store(StoreMemoryRequest) returns (StoreMemoryResponse);

  // Get memories that are candidates for forgetting
  rpc GetForgetCandidates(google.protobuf.Empty) returns (ForgetCandidatesResponse);

  // Confirm which candidates to forget/keep
  rpc ConfirmForget(ForgetDecision) returns (google.protobuf.Empty);

  // Pin or unpin a memory
  rpc Pin(PinRequest) returns (google.protobuf.Empty);

  // Refresh system facts (re-query and update)
  rpc RefreshFacts(google.protobuf.Empty) returns (MemoryResponse);
}
```

**Step 3: Run proto compilation**

Run: `cd cvm-agent && cargo build`
Expected: Builds successfully with new proto types

**Step 4: Commit**

```bash
git add cvm-agent/proto/agent.proto
git commit -m "proto: add MemoryService definitions for Phase 5"
```

---

## Task 2: Set Up SQLite Database Schema

**Files:**
- Create: `cvm-agent/agent-api/src/db/mod.rs`
- Create: `cvm-agent/agent-api/src/db/schema.rs`
- Create: `cvm-agent/agent-api/src/db/migrations.rs`
- Modify: `cvm-agent/agent-api/Cargo.toml`

**Step 1: Add rusqlite dependency**

Add to `agent-api/Cargo.toml`:

```toml
[dependencies]
rusqlite = { version = "0.31", features = ["bundled"] }
```

**Step 2: Create db module structure**

Create `agent-api/src/db/mod.rs`:

```rust
mod migrations;
mod schema;

pub use migrations::run_migrations;
pub use schema::*;

use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub type DbPool = Arc<Mutex<Connection>>;

pub fn init_db(path: &str) -> Result<DbPool, rusqlite::Error> {
    let conn = Connection::open(path)?;
    run_migrations(&conn)?;
    Ok(Arc::new(Mutex::new(conn)))
}
```

**Step 3: Create schema definitions**

Create `agent-api/src/db/schema.rs`:

```rust
use rusqlite::{params, Connection, Result, Row};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct MemoryRecord {
    pub id: String,
    pub layer: i32,
    pub content: String,
    pub metadata_json: String,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub pinned: bool,
    pub access_count: i32,
}

impl MemoryRecord {
    pub fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            layer: row.get(1)?,
            content: row.get(2)?,
            metadata_json: row.get(3)?,
            created_at: row.get(4)?,
            last_accessed: row.get(5)?,
            expires_at: row.get(6)?,
            pinned: row.get(7)?,
            access_count: row.get(8)?,
        })
    }
}
```

**Step 4: Create migrations**

Create `agent-api/src/db/migrations.rs`:

```rust
use rusqlite::{Connection, Result};

pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS memories (
            id TEXT PRIMARY KEY,
            layer INTEGER NOT NULL,
            content TEXT NOT NULL,
            metadata_json TEXT DEFAULT '{}',
            created_at TEXT NOT NULL,
            last_accessed TEXT NOT NULL,
            expires_at TEXT,
            pinned INTEGER DEFAULT 0,
            access_count INTEGER DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_memories_layer ON memories(layer);
        CREATE INDEX IF NOT EXISTS idx_memories_expires ON memories(expires_at);
        CREATE INDEX IF NOT EXISTS idx_memories_pinned ON memories(pinned);

        -- Full-text search virtual table
        CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
            content,
            content='memories',
            content_rowid='rowid'
        );

        -- Triggers to keep FTS in sync
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
```

**Step 5: Run build to verify**

Run: `cd cvm-agent && cargo build`
Expected: Compiles successfully

**Step 6: Commit**

```bash
git add cvm-agent/agent-api/src/db/ cvm-agent/agent-api/Cargo.toml
git commit -m "feat(memory): add SQLite database schema and migrations"
```

---

## Task 3: Implement Memory Repository

**Files:**
- Create: `cvm-agent/agent-api/src/db/repository.rs`
- Modify: `cvm-agent/agent-api/src/db/mod.rs`

**Step 1: Write repository tests first**

Add to end of `repository.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> DbPool {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        Arc::new(Mutex::new(conn))
    }

    #[test]
    fn test_store_and_query() {
        let db = setup_test_db();
        let repo = MemoryRepository::new(db);

        let id = repo.store(
            1, // WORKING layer
            "Test memory content",
            "{}",
            None,
        ).unwrap();

        let results = repo.query(&[], None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "Test memory content");
    }

    #[test]
    fn test_full_text_search() {
        let db = setup_test_db();
        let repo = MemoryRepository::new(db);

        repo.store(1, "NixOS configuration guide", "{}", None).unwrap();
        repo.store(1, "Docker container setup", "{}", None).unwrap();

        let results = repo.search("NixOS", &[], 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("NixOS"));
    }

    #[test]
    fn test_pin_memory() {
        let db = setup_test_db();
        let repo = MemoryRepository::new(db);

        let id = repo.store(1, "Important memory", "{}", None).unwrap();
        repo.pin(&id, true).unwrap();

        let results = repo.query(&[], None, 10).unwrap();
        assert!(results[0].pinned);
    }

    #[test]
    fn test_forget_candidates() {
        let db = setup_test_db();
        let repo = MemoryRepository::new(db);

        // Create expired memory
        let past = Utc::now() - chrono::Duration::days(1);
        repo.store_with_expiry(1, "Expired content", "{}", Some(past)).unwrap();

        // Create valid memory
        repo.store(1, "Valid content", "{}", None).unwrap();

        let candidates = repo.get_forget_candidates().unwrap();
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].content.contains("Expired"));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd cvm-agent && cargo test memory`
Expected: FAIL (repository not implemented)

**Step 3: Implement repository**

Create `agent-api/src/db/repository.rs`:

```rust
use super::{DbPool, MemoryRecord};
use chrono::{DateTime, Duration, Utc};
use rusqlite::params;
use uuid::Uuid;

pub struct MemoryRepository {
    db: DbPool,
}

impl MemoryRepository {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }

    pub fn store(
        &self,
        layer: i32,
        content: &str,
        metadata_json: &str,
        ttl: Option<Duration>,
    ) -> Result<String, rusqlite::Error> {
        let expires_at = ttl.map(|d| Utc::now() + d);
        self.store_with_expiry(layer, content, metadata_json, expires_at)
    }

    pub fn store_with_expiry(
        &self,
        layer: i32,
        content: &str,
        metadata_json: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<String, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let conn = self.db.lock().unwrap();
        conn.execute(
            "INSERT INTO memories (id, layer, content, metadata_json, created_at, last_accessed, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, layer, content, metadata_json, now, now, expires_at],
        )?;

        Ok(id)
    }

    pub fn query(
        &self,
        layers: &[i32],
        metadata_filter: Option<&str>,
        limit: i32,
    ) -> Result<Vec<MemoryRecord>, rusqlite::Error> {
        let conn = self.db.lock().unwrap();

        let layer_filter = if layers.is_empty() {
            "1=1".to_string()
        } else {
            let placeholders: Vec<_> = layers.iter().map(|_| "?").collect();
            format!("layer IN ({})", placeholders.join(","))
        };

        let sql = format!(
            "SELECT id, layer, content, metadata_json, created_at, last_accessed, expires_at, pinned, access_count
             FROM memories
             WHERE {}
             ORDER BY last_accessed DESC
             LIMIT ?",
            layer_filter
        );

        let mut stmt = conn.prepare(&sql)?;

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = layers
            .iter()
            .map(|l| Box::new(*l) as Box<dyn rusqlite::ToSql>)
            .collect();
        params_vec.push(Box::new(limit));

        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())), |row| {
            MemoryRecord::from_row(row)
        })?;

        rows.collect()
    }

    pub fn search(
        &self,
        text: &str,
        layers: &[i32],
        limit: i32,
    ) -> Result<Vec<MemoryRecord>, rusqlite::Error> {
        let conn = self.db.lock().unwrap();

        let layer_filter = if layers.is_empty() {
            "".to_string()
        } else {
            let placeholders: Vec<_> = layers.iter().map(|_| "?").collect();
            format!("AND m.layer IN ({})", placeholders.join(","))
        };

        let sql = format!(
            "SELECT m.id, m.layer, m.content, m.metadata_json, m.created_at, m.last_accessed, m.expires_at, m.pinned, m.access_count
             FROM memories m
             JOIN memories_fts f ON m.rowid = f.rowid
             WHERE memories_fts MATCH ?
             {}
             ORDER BY rank
             LIMIT ?",
            layer_filter
        );

        let mut stmt = conn.prepare(&sql)?;

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(text.to_string())];
        for l in layers {
            params_vec.push(Box::new(*l));
        }
        params_vec.push(Box::new(limit));

        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())), |row| {
            MemoryRecord::from_row(row)
        })?;

        rows.collect()
    }

    pub fn update_access(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        conn.execute(
            "UPDATE memories SET last_accessed = ?, access_count = access_count + 1 WHERE id = ?",
            params![Utc::now(), id],
        )?;
        Ok(())
    }

    pub fn pin(&self, id: &str, pinned: bool) -> Result<(), rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        conn.execute(
            "UPDATE memories SET pinned = ? WHERE id = ?",
            params![pinned, id],
        )?;
        Ok(())
    }

    pub fn get_forget_candidates(&self) -> Result<Vec<MemoryRecord>, rusqlite::Error> {
        let conn = self.db.lock().unwrap();
        let now = Utc::now();
        let thirty_days_ago = now - Duration::days(30);

        let mut stmt = conn.prepare(
            "SELECT id, layer, content, metadata_json, created_at, last_accessed, expires_at, pinned, access_count
             FROM memories
             WHERE pinned = 0 AND (
                 expires_at < ? OR
                 (layer != 3 AND last_accessed < ?)  -- Not system facts and old
             )
             ORDER BY last_accessed ASC
             LIMIT 100"
        )?;

        let rows = stmt.query_map(params![now, thirty_days_ago], |row| {
            MemoryRecord::from_row(row)
        })?;

        rows.collect()
    }

    pub fn delete_many(&self, ids: &[String]) -> Result<usize, rusqlite::Error> {
        if ids.is_empty() {
            return Ok(0);
        }

        let conn = self.db.lock().unwrap();
        let placeholders: Vec<_> = ids.iter().map(|_| "?").collect();
        let sql = format!(
            "DELETE FROM memories WHERE id IN ({}) AND pinned = 0",
            placeholders.join(",")
        );

        let params: Vec<&dyn rusqlite::ToSql> = ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
        let deleted = conn.execute(&sql, params.as_slice())?;
        Ok(deleted)
    }

    pub fn refresh_stale_facts<F>(&self, refresh_fn: F) -> Result<Vec<MemoryRecord>, rusqlite::Error>
    where
        F: Fn(&str) -> Option<String>,
    {
        // Get system facts layer (3)
        let facts = self.query(&[3], None, 100)?;
        let mut refreshed = Vec::new();

        for fact in facts {
            if let Some(key) = fact.metadata_json.strip_prefix(r#"{"key":""#).and_then(|s| s.strip_suffix(r#""}"#)) {
                if let Some(new_value) = refresh_fn(key) {
                    let conn = self.db.lock().unwrap();
                    conn.execute(
                        "UPDATE memories SET content = ?, last_accessed = ? WHERE id = ?",
                        params![new_value, Utc::now(), fact.id],
                    )?;
                    let mut updated = fact;
                    updated.content = new_value;
                    refreshed.push(updated);
                }
            }
        }

        Ok(refreshed)
    }
}
```

**Step 4: Update db/mod.rs**

```rust
mod migrations;
mod repository;
mod schema;

pub use migrations::run_migrations;
pub use repository::MemoryRepository;
pub use schema::*;

use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub type DbPool = Arc<Mutex<Connection>>;

pub fn init_db(path: &str) -> Result<DbPool, rusqlite::Error> {
    let conn = Connection::open(path)?;
    run_migrations(&conn)?;
    Ok(Arc::new(Mutex::new(conn)))
}
```

**Step 5: Run tests**

Run: `cd cvm-agent && cargo test memory`
Expected: All tests pass

**Step 6: Commit**

```bash
git add cvm-agent/agent-api/src/db/
git commit -m "feat(memory): implement MemoryRepository with SQLite backend"
```

---

## Task 4: Implement MemoryService gRPC Handler

**Files:**
- Create: `cvm-agent/agent-api/src/services/memory.rs`
- Modify: `cvm-agent/agent-api/src/services/mod.rs`
- Modify: `cvm-agent/agent-api/src/main.rs`

**Step 1: Create the memory service implementation**

Create `agent-api/src/services/memory.rs`:

```rust
use crate::db::{MemoryRepository, DbPool};
use crate::proto::cvm::agent::{
    memory_service_server::MemoryService as MemoryServiceTrait,
    ForgetCandidatesResponse, ForgetCandidate, ForgetDecision,
    MemoryEntry, MemoryLayer, MemoryQuery, MemoryResponse,
    PinRequest, StoreMemoryRequest, StoreMemoryResponse,
};
use chrono::{Duration, Utc};
use prost_types::{Duration as ProtoDuration, Timestamp};
use tonic::{Request, Response, Status};

pub struct MemoryServiceImpl {
    repo: MemoryRepository,
}

impl MemoryServiceImpl {
    pub fn new(db: DbPool) -> Self {
        Self {
            repo: MemoryRepository::new(db),
        }
    }

    fn record_to_entry(&self, rec: crate::db::MemoryRecord) -> MemoryEntry {
        MemoryEntry {
            id: rec.id,
            layer: rec.layer,
            content: rec.content,
            metadata: serde_json::from_str(&rec.metadata_json).unwrap_or_default(),
            created_at: Some(datetime_to_timestamp(rec.created_at)),
            last_accessed: Some(datetime_to_timestamp(rec.last_accessed)),
            expires_at: rec.expires_at.map(datetime_to_timestamp),
            pinned: rec.pinned,
            access_count: rec.access_count,
        }
    }
}

fn datetime_to_timestamp(dt: chrono::DateTime<Utc>) -> Timestamp {
    Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

fn proto_duration_to_chrono(d: &ProtoDuration) -> Duration {
    Duration::seconds(d.seconds) + Duration::nanoseconds(d.nanos as i64)
}

#[tonic::async_trait]
impl MemoryServiceTrait for MemoryServiceImpl {
    async fn query(
        &self,
        request: Request<MemoryQuery>,
    ) -> Result<Response<MemoryResponse>, Status> {
        let req = request.into_inner();
        let layers: Vec<i32> = req.layers.iter().map(|l| *l as i32).collect();
        let limit = if req.limit > 0 { req.limit } else { 50 };

        let records = if !req.search_text.is_empty() {
            self.repo.search(&req.search_text, &layers, limit)
        } else {
            self.repo.query(&layers, None, limit)
        }
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let entries: Vec<MemoryEntry> = records
            .into_iter()
            .map(|r| self.record_to_entry(r))
            .collect();

        Ok(Response::new(MemoryResponse {
            entries,
            total_count: 0, // Could add count query
        }))
    }

    async fn store(
        &self,
        request: Request<StoreMemoryRequest>,
    ) -> Result<Response<StoreMemoryResponse>, Status> {
        let req = request.into_inner();
        let metadata_json = serde_json::to_string(&req.metadata)
            .map_err(|e| Status::invalid_argument(format!("Invalid metadata: {}", e)))?;

        let ttl = req.ttl.map(|d| proto_duration_to_chrono(&d));

        let id = self.repo
            .store(req.layer as i32, &req.content, &metadata_json, ttl)
            .map_err(|e| Status::internal(format!("Store error: {}", e)))?;

        Ok(Response::new(StoreMemoryResponse { id }))
    }

    async fn get_forget_candidates(
        &self,
        _request: Request<()>,
    ) -> Result<Response<ForgetCandidatesResponse>, Status> {
        let records = self.repo
            .get_forget_candidates()
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let candidates: Vec<ForgetCandidate> = records
            .into_iter()
            .map(|r| {
                let reason = if r.expires_at.map(|e| e < Utc::now()).unwrap_or(false) {
                    "Expired".to_string()
                } else {
                    "Not accessed in 30+ days".to_string()
                };
                let auto_forget = r.layer == 1 || r.expires_at.is_some(); // Working layer or expired

                ForgetCandidate {
                    entry: Some(self.record_to_entry(r)),
                    reason,
                    auto_forget,
                }
            })
            .collect();

        Ok(Response::new(ForgetCandidatesResponse { candidates }))
    }

    async fn confirm_forget(
        &self,
        request: Request<ForgetDecision>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();

        // Update kept items (reset decay)
        for id in &req.keep_ids {
            self.repo.update_access(id)
                .map_err(|e| Status::internal(format!("Update error: {}", e)))?;
        }

        // Delete forgotten items
        self.repo
            .delete_many(&req.forget_ids)
            .map_err(|e| Status::internal(format!("Delete error: {}", e)))?;

        Ok(Response::new(()))
    }

    async fn pin(
        &self,
        request: Request<PinRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        self.repo
            .pin(&req.id, req.pinned)
            .map_err(|e| Status::internal(format!("Pin error: {}", e)))?;

        Ok(Response::new(()))
    }

    async fn refresh_facts(
        &self,
        _request: Request<()>,
    ) -> Result<Response<MemoryResponse>, Status> {
        // Define fact refreshers
        let refresh_fn = |key: &str| -> Option<String> {
            match key {
                "nixos_generation" => {
                    std::process::Command::new("nixos-rebuild")
                        .args(["list-generations"])
                        .output()
                        .ok()
                        .and_then(|o| String::from_utf8(o.stdout).ok())
                        .map(|s| s.lines().last().unwrap_or("").to_string())
                }
                "git_status" => {
                    std::process::Command::new("git")
                        .args(["status", "--porcelain"])
                        .output()
                        .ok()
                        .and_then(|o| String::from_utf8(o.stdout).ok())
                }
                _ => None,
            }
        };

        let refreshed = self.repo
            .refresh_stale_facts(refresh_fn)
            .map_err(|e| Status::internal(format!("Refresh error: {}", e)))?;

        let entries: Vec<MemoryEntry> = refreshed
            .into_iter()
            .map(|r| self.record_to_entry(r))
            .collect();

        Ok(Response::new(MemoryResponse {
            entries,
            total_count: entries.len() as i32,
        }))
    }
}
```

**Step 2: Update services/mod.rs**

Add to `services/mod.rs`:

```rust
mod memory;
pub use memory::MemoryServiceImpl;
```

**Step 3: Register service in main.rs**

Add to server initialization in `main.rs`:

```rust
use crate::services::MemoryServiceImpl;
use crate::db::init_db;
use crate::proto::cvm::agent::memory_service_server::MemoryServiceServer;

// In main():
let db = init_db("./data/agent.db").expect("Failed to init database");
let memory_service = MemoryServiceImpl::new(db.clone());

// Add to Server::builder()
.add_service(MemoryServiceServer::new(memory_service))
```

**Step 4: Build to verify**

Run: `cd cvm-agent && cargo build`
Expected: Compiles successfully

**Step 5: Commit**

```bash
git add cvm-agent/agent-api/src/services/memory.rs cvm-agent/agent-api/src/services/mod.rs cvm-agent/agent-api/src/main.rs
git commit -m "feat(memory): implement MemoryService gRPC handler"
```

---

## Task 5: Implement Active Forgetting System

**Files:**
- Create: `cvm-agent/agent-api/src/services/forgetting.rs`
- Modify: `cvm-agent/agent-api/src/services/mod.rs`
- Modify: `cvm-agent/agent-api/src/main.rs`

**Step 1: Write tests for forgetting triggers**

Add to `forgetting.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forgetting_trigger_rebuild() {
        let trigger = ForgettingTrigger::NixosRebuild { success: true };
        let actions = trigger.get_actions();
        assert!(actions.contains(&ForgettingAction::PurgePreviousGeneration));
    }

    #[test]
    fn test_forgetting_trigger_git_commit() {
        let trigger = ForgettingTrigger::GitCommit { hash: "abc123".into() };
        let actions = trigger.get_actions();
        assert!(actions.contains(&ForgettingAction::ArchivePreCommitState));
    }

    #[test]
    fn test_auto_forget_classification() {
        assert!(should_auto_forget(MemoryLayer::Working, false, true));  // Working, not pinned, expired
        assert!(!should_auto_forget(MemoryLayer::Preferences, false, false));  // Preferences need confirmation
        assert!(!should_auto_forget(MemoryLayer::Archive, true, true));  // Pinned items never auto-forget
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd cvm-agent && cargo test forgetting`
Expected: FAIL

**Step 3: Implement forgetting system**

Create `agent-api/src/services/forgetting.rs`:

```rust
use crate::db::{DbPool, MemoryRepository};
use crate::proto::cvm::agent::MemoryLayer;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Triggers that initiate forgetting actions
#[derive(Debug, Clone)]
pub enum ForgettingTrigger {
    NixosRebuild { success: bool },
    GitCommit { hash: String },
    GitPush { branch: String },
    ErrorResolved { error_id: String },
    PreferenceContradicted { preference_id: String, count: i32 },
    SystemFactStale { fact_key: String },
    ScheduledSweep,
}

/// Actions to take in response to triggers
#[derive(Debug, Clone, PartialEq)]
pub enum ForgettingAction {
    PurgePreviousGeneration,
    ArchivePreCommitState,
    SummarizeAndArchiveError,
    RemovePreference,
    RefreshFact,
    QueueForUserConfirmation,
    AutoDelete,
}

impl ForgettingTrigger {
    pub fn get_actions(&self) -> Vec<ForgettingAction> {
        match self {
            ForgettingTrigger::NixosRebuild { success: true } => {
                vec![ForgettingAction::PurgePreviousGeneration]
            }
            ForgettingTrigger::NixosRebuild { success: false } => vec![],
            ForgettingTrigger::GitCommit { .. } | ForgettingTrigger::GitPush { .. } => {
                vec![ForgettingAction::ArchivePreCommitState]
            }
            ForgettingTrigger::ErrorResolved { .. } => {
                vec![ForgettingAction::SummarizeAndArchiveError]
            }
            ForgettingTrigger::PreferenceContradicted { count, .. } if *count >= 3 => {
                vec![ForgettingAction::RemovePreference]
            }
            ForgettingTrigger::PreferenceContradicted { .. } => vec![],
            ForgettingTrigger::SystemFactStale { .. } => {
                vec![ForgettingAction::RefreshFact]
            }
            ForgettingTrigger::ScheduledSweep => {
                vec![ForgettingAction::QueueForUserConfirmation]
            }
        }
    }
}

/// Determine if a memory should be auto-forgotten without user confirmation
pub fn should_auto_forget(layer: MemoryLayer, pinned: bool, expired: bool) -> bool {
    if pinned {
        return false;
    }

    match layer {
        MemoryLayer::Working => expired, // Working context can auto-expire
        MemoryLayer::Facts => expired,   // Stale facts can auto-refresh
        MemoryLayer::Archive => false,   // Archive needs confirmation
        MemoryLayer::Preferences => false, // Preferences need confirmation
        MemoryLayer::Unspecified => expired,
    }
}

/// Background task that handles forgetting triggers
pub struct ForgettingManager {
    repo: MemoryRepository,
    trigger_rx: mpsc::Receiver<ForgettingTrigger>,
}

impl ForgettingManager {
    pub fn new(db: DbPool, trigger_rx: mpsc::Receiver<ForgettingTrigger>) -> Self {
        Self {
            repo: MemoryRepository::new(db),
            trigger_rx,
        }
    }

    pub async fn run(mut self) {
        info!("Forgetting manager started");

        while let Some(trigger) = self.trigger_rx.recv().await {
            let actions = trigger.get_actions();
            for action in actions {
                if let Err(e) = self.execute_action(&trigger, &action).await {
                    warn!("Forgetting action failed: {:?} - {}", action, e);
                }
            }
        }
    }

    async fn execute_action(
        &self,
        trigger: &ForgettingTrigger,
        action: &ForgettingAction,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match action {
            ForgettingAction::PurgePreviousGeneration => {
                info!("Purging previous NixOS generation memory");
                // Delete memories with metadata containing old generation
                let candidates = self.repo.get_forget_candidates()?;
                let to_delete: Vec<_> = candidates
                    .iter()
                    .filter(|m| m.metadata_json.contains("generation"))
                    .map(|m| m.id.clone())
                    .collect();
                self.repo.delete_many(&to_delete)?;
            }
            ForgettingAction::ArchivePreCommitState => {
                info!("Archiving pre-commit state");
                // Move working context to archive with summary
                // Implementation depends on summarization strategy
            }
            ForgettingAction::SummarizeAndArchiveError => {
                if let ForgettingTrigger::ErrorResolved { error_id } = trigger {
                    info!("Archiving resolved error: {}", error_id);
                    // Could summarize error context and move to archive
                }
            }
            ForgettingAction::RemovePreference => {
                if let ForgettingTrigger::PreferenceContradicted { preference_id, .. } = trigger {
                    info!("Removing contradicted preference: {}", preference_id);
                    self.repo.delete_many(&[preference_id.clone()])?;
                }
            }
            ForgettingAction::RefreshFact => {
                if let ForgettingTrigger::SystemFactStale { fact_key } = trigger {
                    info!("Refreshing stale fact: {}", fact_key);
                    self.repo.refresh_stale_facts(|k| {
                        if k == fact_key {
                            // Return refreshed value based on key
                            None // Actual implementation queries system
                        } else {
                            None
                        }
                    })?;
                }
            }
            ForgettingAction::QueueForUserConfirmation => {
                info!("Queuing candidates for user confirmation");
                // Candidates are queried via GetForgetCandidates RPC
            }
            ForgettingAction::AutoDelete => {
                info!("Auto-deleting expired entries");
                let candidates = self.repo.get_forget_candidates()?;
                let to_delete: Vec<_> = candidates
                    .iter()
                    .filter(|m| {
                        let layer = match m.layer {
                            1 => MemoryLayer::Working,
                            2 => MemoryLayer::Archive,
                            3 => MemoryLayer::Facts,
                            4 => MemoryLayer::Preferences,
                            _ => MemoryLayer::Unspecified,
                        };
                        let expired = m.expires_at.map(|e| e < chrono::Utc::now()).unwrap_or(false);
                        should_auto_forget(layer, m.pinned, expired)
                    })
                    .map(|m| m.id.clone())
                    .collect();
                self.repo.delete_many(&to_delete)?;
            }
        }
        Ok(())
    }
}

/// Create a channel for sending forgetting triggers
pub fn create_trigger_channel() -> (mpsc::Sender<ForgettingTrigger>, mpsc::Receiver<ForgettingTrigger>) {
    mpsc::channel(100)
}
```

**Step 4: Update services/mod.rs**

```rust
mod forgetting;
pub use forgetting::{ForgettingManager, ForgettingTrigger, create_trigger_channel};
```

**Step 5: Initialize in main.rs**

Add to `main.rs`:

```rust
use crate::services::{create_trigger_channel, ForgettingManager};

// In main():
let (trigger_tx, trigger_rx) = create_trigger_channel();
let forgetting_manager = ForgettingManager::new(db.clone(), trigger_rx);

// Spawn background task
tokio::spawn(forgetting_manager.run());

// Pass trigger_tx to services that need to trigger forgetting
```

**Step 6: Run tests**

Run: `cd cvm-agent && cargo test forgetting`
Expected: All tests pass

**Step 7: Commit**

```bash
git add cvm-agent/agent-api/src/services/forgetting.rs cvm-agent/agent-api/src/services/mod.rs cvm-agent/agent-api/src/main.rs
git commit -m "feat(memory): implement active forgetting system with triggers"
```

---

## Task 6: Regenerate TypeScript Client for Memory

**Files:**
- Modify: `cvm-agent/web-ui/src/grpc/client.ts` (regenerated)
- Modify: `cvm-agent/web-ui/package.json` (if needed)

**Step 1: Regenerate TypeScript types from proto**

Run: `cd cvm-agent && buf generate proto/`
Expected: Updated TypeScript client in web-ui/src/grpc/

**Step 2: Verify types exist**

Check that `MemoryService`, `MemoryEntry`, `ForgetCandidate` types are generated.

Run: `cd cvm-agent/web-ui && npm run typecheck`
Expected: No type errors

**Step 3: Commit**

```bash
git add cvm-agent/web-ui/src/grpc/
git commit -m "chore: regenerate TypeScript client for MemoryService"
```

---

## Task 7: Implement Adaptive UI State Machine

**Files:**
- Create: `cvm-agent/web-ui/src/hooks/useOverlayState.ts`
- Modify: `cvm-agent/web-ui/src/components/Overlay.tsx`

**Step 1: Write tests for state transitions**

Create `cvm-agent/web-ui/src/hooks/useOverlayState.test.ts`:

```typescript
import { describe, it, expect } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useOverlayState, OverlayState, OverlayEvent } from './useOverlayState';

describe('useOverlayState', () => {
  it('starts in hidden state', () => {
    const { result } = renderHook(() => useOverlayState());
    expect(result.current.state).toBe('hidden');
  });

  it('transitions to quick-input on toggle', () => {
    const { result } = renderHook(() => useOverlayState());
    act(() => result.current.send('TOGGLE'));
    expect(result.current.state).toBe('quick-input');
  });

  it('transitions to expanded on approval-needed', () => {
    const { result } = renderHook(() => useOverlayState());
    act(() => result.current.send('TOGGLE'));
    act(() => result.current.send('APPROVAL_NEEDED'));
    expect(result.current.state).toBe('expanded');
  });

  it('transitions to hidden on escape from quick-input', () => {
    const { result } = renderHook(() => useOverlayState());
    act(() => result.current.send('TOGGLE'));
    act(() => result.current.send('ESCAPE'));
    expect(result.current.state).toBe('hidden');
  });

  it('auto-expands on error', () => {
    const { result } = renderHook(() => useOverlayState());
    act(() => result.current.send('TOGGLE'));
    act(() => result.current.send('ERROR'));
    expect(result.current.state).toBe('expanded');
  });
});
```

**Step 2: Run tests to verify they fail**

Run: `cd cvm-agent/web-ui && npm test -- useOverlayState`
Expected: FAIL

**Step 3: Implement state machine hook**

Create `cvm-agent/web-ui/src/hooks/useOverlayState.ts`:

```typescript
import { useReducer, useCallback, useEffect } from 'react';

export type OverlayState = 'hidden' | 'quick-input' | 'expanded';

export type OverlayEvent =
  | 'TOGGLE'           // cmd+shift+space or FAB click
  | 'ESCAPE'           // Escape key or click outside
  | 'EXPAND'           // Manual expand
  | 'COLLAPSE'         // Manual collapse back to quick-input
  | 'APPROVAL_NEEDED'  // Risky operation needs approval
  | 'ERROR'            // Error occurred
  | 'LONG_CONVERSATION'// Conversation exceeds threshold
  | 'STREAMING_START'  // Long streaming output started
  | 'HIDE';            // Force hide

interface StateConfig {
  on: Partial<Record<OverlayEvent, OverlayState>>;
}

const stateConfig: Record<OverlayState, StateConfig> = {
  hidden: {
    on: {
      TOGGLE: 'quick-input',
    },
  },
  'quick-input': {
    on: {
      TOGGLE: 'hidden',
      ESCAPE: 'hidden',
      HIDE: 'hidden',
      EXPAND: 'expanded',
      APPROVAL_NEEDED: 'expanded',
      ERROR: 'expanded',
      LONG_CONVERSATION: 'expanded',
      STREAMING_START: 'expanded',
    },
  },
  expanded: {
    on: {
      TOGGLE: 'hidden',
      ESCAPE: 'quick-input',
      HIDE: 'hidden',
      COLLAPSE: 'quick-input',
    },
  },
};

function reducer(state: OverlayState, event: OverlayEvent): OverlayState {
  const config = stateConfig[state];
  const nextState = config.on[event];
  return nextState ?? state;
}

export interface UseOverlayStateReturn {
  state: OverlayState;
  send: (event: OverlayEvent) => void;
  isVisible: boolean;
  isExpanded: boolean;
}

export function useOverlayState(): UseOverlayStateReturn {
  const [state, dispatch] = useReducer(reducer, 'hidden');

  const send = useCallback((event: OverlayEvent) => {
    dispatch(event);
  }, []);

  // Global keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // cmd+shift+space (Mac) or ctrl+shift+space (Windows/Linux)
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.code === 'Space') {
        e.preventDefault();
        send('TOGGLE');
      }
      // Escape to dismiss/collapse
      if (e.key === 'Escape') {
        send('ESCAPE');
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [send]);

  return {
    state,
    send,
    isVisible: state !== 'hidden',
    isExpanded: state === 'expanded',
  };
}
```

**Step 4: Run tests**

Run: `cd cvm-agent/web-ui && npm test -- useOverlayState`
Expected: All tests pass

**Step 5: Update Overlay.tsx to use state machine**

Modify `cvm-agent/web-ui/src/components/Overlay.tsx`:

```typescript
import React from 'react';
import { useOverlayState } from '../hooks/useOverlayState';
import { ChatPanel } from './ChatPanel';

export function Overlay() {
  const { state, send, isVisible, isExpanded } = useOverlayState();

  if (!isVisible) {
    return (
      <button
        onClick={() => send('TOGGLE')}
        className="fixed bottom-4 right-4 w-12 h-12 rounded-full bg-indigo-600 text-white shadow-lg hover:bg-indigo-700 flex items-center justify-center"
        aria-label="Open agent"
      >
        <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 10h.01M12 10h.01M16 10h.01M9 16H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-5l-5 5v-5z" />
        </svg>
      </button>
    );
  }

  return (
    <div
      className={`fixed transition-all duration-200 ${
        isExpanded
          ? 'inset-y-0 right-0 w-96 bg-gray-900/95'
          : 'bottom-4 right-4 w-80 bg-gray-900/90 rounded-lg'
      }`}
    >
      <ChatPanel
        expanded={isExpanded}
        onExpand={() => send('EXPAND')}
        onCollapse={() => send('COLLAPSE')}
        onClose={() => send('HIDE')}
        onApprovalNeeded={() => send('APPROVAL_NEEDED')}
        onError={() => send('ERROR')}
      />
    </div>
  );
}
```

**Step 6: Commit**

```bash
git add cvm-agent/web-ui/src/hooks/useOverlayState.ts cvm-agent/web-ui/src/hooks/useOverlayState.test.ts cvm-agent/web-ui/src/components/Overlay.tsx
git commit -m "feat(ui): implement adaptive overlay state machine"
```

---

## Task 8: Implement Memory UI Components

**Files:**
- Create: `cvm-agent/web-ui/src/components/MemoryPanel.tsx`
- Create: `cvm-agent/web-ui/src/components/ForgetConfirmDialog.tsx`
- Create: `cvm-agent/web-ui/src/hooks/useMemory.ts`

**Step 1: Create memory hook**

Create `cvm-agent/web-ui/src/hooks/useMemory.ts`:

```typescript
import { useState, useCallback } from 'react';
import { createPromiseClient } from '@connectrpc/connect';
import { createGrpcWebTransport } from '@connectrpc/connect-web';
import { MemoryService } from '../grpc/agent_connect';
import type { MemoryEntry, ForgetCandidate, MemoryLayer } from '../grpc/agent_pb';

const transport = createGrpcWebTransport({
  baseUrl: import.meta.env.VITE_API_URL || 'http://localhost:8080',
});

const client = createPromiseClient(MemoryService, transport);

export function useMemory() {
  const [memories, setMemories] = useState<MemoryEntry[]>([]);
  const [forgetCandidates, setForgetCandidates] = useState<ForgetCandidate[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const query = useCallback(async (layers?: MemoryLayer[], searchText?: string) => {
    setLoading(true);
    setError(null);
    try {
      const response = await client.query({
        layers: layers || [],
        searchText: searchText || '',
        limit: 50,
      });
      setMemories(response.entries);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Query failed');
    } finally {
      setLoading(false);
    }
  }, []);

  const store = useCallback(async (layer: MemoryLayer, content: string, metadata?: Record<string, string>) => {
    try {
      const response = await client.store({
        layer,
        content,
        metadata: metadata || {},
      });
      return response.id;
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Store failed');
      return null;
    }
  }, []);

  const getForgetCandidates = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await client.getForgetCandidates({});
      setForgetCandidates(response.candidates);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to get candidates');
    } finally {
      setLoading(false);
    }
  }, []);

  const confirmForget = useCallback(async (forgetIds: string[], keepIds: string[]) => {
    try {
      await client.confirmForget({ forgetIds, keepIds });
      // Refresh candidates
      await getForgetCandidates();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Forget failed');
    }
  }, [getForgetCandidates]);

  const pin = useCallback(async (id: string, pinned: boolean) => {
    try {
      await client.pin({ id, pinned });
      // Refresh memories
      await query();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Pin failed');
    }
  }, [query]);

  return {
    memories,
    forgetCandidates,
    loading,
    error,
    query,
    store,
    getForgetCandidates,
    confirmForget,
    pin,
  };
}
```

**Step 2: Create ForgetConfirmDialog**

Create `cvm-agent/web-ui/src/components/ForgetConfirmDialog.tsx`:

```typescript
import React, { useState } from 'react';
import type { ForgetCandidate } from '../grpc/agent_pb';

interface Props {
  candidates: ForgetCandidate[];
  onConfirm: (forgetIds: string[], keepIds: string[]) => void;
  onCancel: () => void;
}

export function ForgetConfirmDialog({ candidates, onConfirm, onCancel }: Props) {
  const [decisions, setDecisions] = useState<Record<string, 'forget' | 'keep'>>(() => {
    const initial: Record<string, 'forget' | 'keep'> = {};
    candidates.forEach((c) => {
      if (c.entry) {
        initial[c.entry.id] = c.autoForget ? 'forget' : 'keep';
      }
    });
    return initial;
  });

  const handleConfirm = () => {
    const forgetIds: string[] = [];
    const keepIds: string[] = [];
    Object.entries(decisions).forEach(([id, decision]) => {
      if (decision === 'forget') {
        forgetIds.push(id);
      } else {
        keepIds.push(id);
      }
    });
    onConfirm(forgetIds, keepIds);
  };

  const toggleDecision = (id: string) => {
    setDecisions((prev) => ({
      ...prev,
      [id]: prev[id] === 'forget' ? 'keep' : 'forget',
    }));
  };

  const needsConfirmation = candidates.filter((c) => !c.autoForget);
  const autoForgetCount = candidates.filter((c) => c.autoForget).length;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-gray-800 rounded-lg p-6 max-w-lg w-full max-h-[80vh] overflow-y-auto">
        <h2 className="text-xl font-semibold text-white mb-4">Memory Cleanup</h2>

        {autoForgetCount > 0 && (
          <p className="text-gray-400 mb-4">
            {autoForgetCount} expired items will be automatically removed.
          </p>
        )}

        {needsConfirmation.length > 0 && (
          <>
            <p className="text-gray-300 mb-4">
              The following memories haven't been accessed recently. Choose which to keep or forget:
            </p>

            <div className="space-y-3 mb-6">
              {needsConfirmation.map((candidate) => {
                const entry = candidate.entry;
                if (!entry) return null;

                return (
                  <div
                    key={entry.id}
                    className={`p-3 rounded border ${
                      decisions[entry.id] === 'forget'
                        ? 'border-red-500/50 bg-red-900/20'
                        : 'border-green-500/50 bg-green-900/20'
                    }`}
                  >
                    <div className="flex items-start justify-between">
                      <div className="flex-1">
                        <p className="text-gray-200 text-sm line-clamp-2">{entry.content}</p>
                        <p className="text-gray-500 text-xs mt-1">{candidate.reason}</p>
                      </div>
                      <button
                        onClick={() => toggleDecision(entry.id)}
                        className={`ml-3 px-3 py-1 rounded text-sm ${
                          decisions[entry.id] === 'forget'
                            ? 'bg-red-600 text-white'
                            : 'bg-green-600 text-white'
                        }`}
                      >
                        {decisions[entry.id] === 'forget' ? 'Forget' : 'Keep'}
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          </>
        )}

        <div className="flex justify-end gap-3">
          <button
            onClick={onCancel}
            className="px-4 py-2 bg-gray-600 text-white rounded hover:bg-gray-500"
          >
            Cancel
          </button>
          <button
            onClick={handleConfirm}
            className="px-4 py-2 bg-indigo-600 text-white rounded hover:bg-indigo-500"
          >
            Confirm
          </button>
        </div>
      </div>
    </div>
  );
}
```

**Step 3: Create MemoryPanel**

Create `cvm-agent/web-ui/src/components/MemoryPanel.tsx`:

```typescript
import React, { useEffect, useState } from 'react';
import { useMemory } from '../hooks/useMemory';
import { ForgetConfirmDialog } from './ForgetConfirmDialog';
import { MemoryLayer } from '../grpc/agent_pb';

const LAYER_LABELS: Record<number, string> = {
  1: 'Working',
  2: 'Archive',
  3: 'Facts',
  4: 'Preferences',
};

export function MemoryPanel() {
  const {
    memories,
    forgetCandidates,
    loading,
    error,
    query,
    getForgetCandidates,
    confirmForget,
    pin,
  } = useMemory();

  const [showForgetDialog, setShowForgetDialog] = useState(false);
  const [searchText, setSearchText] = useState('');
  const [selectedLayer, setSelectedLayer] = useState<MemoryLayer | null>(null);

  useEffect(() => {
    query(selectedLayer ? [selectedLayer] : undefined, searchText || undefined);
  }, [selectedLayer, searchText, query]);

  const handleCheckForget = async () => {
    await getForgetCandidates();
    if (forgetCandidates.length > 0) {
      setShowForgetDialog(true);
    }
  };

  const handleConfirmForget = async (forgetIds: string[], keepIds: string[]) => {
    await confirmForget(forgetIds, keepIds);
    setShowForgetDialog(false);
  };

  return (
    <div className="p-4">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold text-white">Memory</h3>
        <button
          onClick={handleCheckForget}
          className="text-sm text-gray-400 hover:text-white"
        >
          Check for cleanup
        </button>
      </div>

      <div className="flex gap-2 mb-4">
        <input
          type="text"
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          placeholder="Search memories..."
          className="flex-1 bg-gray-700 text-white rounded px-3 py-2 text-sm"
        />
        <select
          value={selectedLayer ?? ''}
          onChange={(e) => setSelectedLayer(e.target.value ? Number(e.target.value) as MemoryLayer : null)}
          className="bg-gray-700 text-white rounded px-3 py-2 text-sm"
        >
          <option value="">All layers</option>
          <option value="1">Working</option>
          <option value="2">Archive</option>
          <option value="3">Facts</option>
          <option value="4">Preferences</option>
        </select>
      </div>

      {error && (
        <div className="text-red-400 text-sm mb-4">{error}</div>
      )}

      {loading ? (
        <div className="text-gray-400 text-center py-8">Loading...</div>
      ) : memories.length === 0 ? (
        <div className="text-gray-400 text-center py-8">No memories found</div>
      ) : (
        <div className="space-y-2">
          {memories.map((memory) => (
            <div
              key={memory.id}
              className="bg-gray-700 rounded p-3"
            >
              <div className="flex items-start justify-between">
                <div className="flex-1">
                  <div className="flex items-center gap-2 mb-1">
                    <span className="text-xs bg-gray-600 text-gray-300 px-2 py-0.5 rounded">
                      {LAYER_LABELS[memory.layer] || 'Unknown'}
                    </span>
                    {memory.pinned && (
                      <span className="text-xs text-yellow-400">Pinned</span>
                    )}
                  </div>
                  <p className="text-gray-200 text-sm">{memory.content}</p>
                </div>
                <button
                  onClick={() => pin(memory.id, !memory.pinned)}
                  className={`ml-2 p-1 rounded ${
                    memory.pinned ? 'text-yellow-400' : 'text-gray-500 hover:text-gray-300'
                  }`}
                  title={memory.pinned ? 'Unpin' : 'Pin'}
                >
                  <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
                    <path d="M5 5a2 2 0 012-2h6a2 2 0 012 2v2h2a1 1 0 011 1v1a1 1 0 01-.293.707L15 12.414V17a1 1 0 01-1 1h-4a1 1 0 01-1-1v-4.586l-2.707-2.707A1 1 0 016 9V8a1 1 0 011-1h2V5z" />
                  </svg>
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {showForgetDialog && (
        <ForgetConfirmDialog
          candidates={forgetCandidates}
          onConfirm={handleConfirmForget}
          onCancel={() => setShowForgetDialog(false)}
        />
      )}
    </div>
  );
}
```

**Step 4: Run typecheck**

Run: `cd cvm-agent/web-ui && npm run typecheck`
Expected: No errors

**Step 5: Commit**

```bash
git add cvm-agent/web-ui/src/hooks/useMemory.ts cvm-agent/web-ui/src/components/MemoryPanel.tsx cvm-agent/web-ui/src/components/ForgetConfirmDialog.tsx
git commit -m "feat(ui): add memory panel with forget confirmation dialog"
```

---

## Task 9: Implement Error Recovery System

**Files:**
- Create: `cvm-agent/agent-api/src/services/recovery.rs`
- Modify: `cvm-agent/agent-api/src/services/mod.rs`
- Create: `cvm-agent/web-ui/src/components/ErrorRecovery.tsx`

**Step 1: Write recovery tests**

Add to `recovery.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_error() {
        let error = AgentError::new(ErrorKind::NixosBuild, "build failed");
        assert!(error.is_recoverable());
        assert!(error.suggested_actions().contains(&RecoveryAction::Rollback));
    }

    #[test]
    fn test_retry_policy() {
        let policy = RetryPolicy::exponential(3, Duration::from_secs(1));
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.delay_for_attempt(1), Duration::from_secs(1));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_secs(2));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd cvm-agent && cargo test recovery`
Expected: FAIL

**Step 3: Implement recovery system**

Create `agent-api/src/services/recovery.rs`:

```rust
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorKind {
    // NixOS errors
    NixosBuild,
    NixosRollback,
    NixosSwitch,

    // Git errors
    GitConflict,
    GitPush,
    GitAuth,

    // Shell errors
    ShellTimeout,
    ShellPermission,

    // GUI errors
    GuiElement,
    GuiAction,

    // Network errors
    ApiTimeout,
    ApiAuth,

    // General
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryAction {
    Retry,
    Rollback,
    ManualIntervention,
    RestartService,
    ClearCache,
    AuthRefresh,
    AbortOperation,
}

#[derive(Debug, Clone)]
pub struct AgentError {
    pub kind: ErrorKind,
    pub message: String,
    pub context: Option<String>,
    pub retry_count: u32,
}

impl AgentError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            context: None,
            retry_count: 0,
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn is_recoverable(&self) -> bool {
        match self.kind {
            ErrorKind::NixosBuild => true,
            ErrorKind::NixosRollback => true,
            ErrorKind::NixosSwitch => true,
            ErrorKind::GitConflict => true,
            ErrorKind::GitPush => true,
            ErrorKind::GitAuth => true,
            ErrorKind::ShellTimeout => true,
            ErrorKind::ShellPermission => false, // Needs manual fix
            ErrorKind::GuiElement => true,
            ErrorKind::GuiAction => true,
            ErrorKind::ApiTimeout => true,
            ErrorKind::ApiAuth => true,
            ErrorKind::Unknown => false,
        }
    }

    pub fn suggested_actions(&self) -> Vec<RecoveryAction> {
        match self.kind {
            ErrorKind::NixosBuild => vec![
                RecoveryAction::Retry,
                RecoveryAction::Rollback,
                RecoveryAction::ManualIntervention,
            ],
            ErrorKind::NixosRollback => vec![
                RecoveryAction::Retry,
                RecoveryAction::ManualIntervention,
            ],
            ErrorKind::NixosSwitch => vec![
                RecoveryAction::Rollback,
                RecoveryAction::Retry,
            ],
            ErrorKind::GitConflict => vec![
                RecoveryAction::ManualIntervention,
                RecoveryAction::AbortOperation,
            ],
            ErrorKind::GitPush => vec![
                RecoveryAction::Retry,
                RecoveryAction::ManualIntervention,
            ],
            ErrorKind::GitAuth => vec![
                RecoveryAction::AuthRefresh,
                RecoveryAction::ManualIntervention,
            ],
            ErrorKind::ShellTimeout => vec![
                RecoveryAction::Retry,
                RecoveryAction::AbortOperation,
            ],
            ErrorKind::ShellPermission => vec![
                RecoveryAction::ManualIntervention,
            ],
            ErrorKind::GuiElement => vec![
                RecoveryAction::Retry,
                RecoveryAction::ClearCache,
            ],
            ErrorKind::GuiAction => vec![
                RecoveryAction::Retry,
            ],
            ErrorKind::ApiTimeout => vec![
                RecoveryAction::Retry,
            ],
            ErrorKind::ApiAuth => vec![
                RecoveryAction::AuthRefresh,
            ],
            ErrorKind::Unknown => vec![
                RecoveryAction::ManualIntervention,
            ],
        }
    }

    pub fn user_message(&self) -> String {
        match self.kind {
            ErrorKind::NixosBuild => format!(
                "NixOS build failed: {}. You can try rebuilding, or rollback to the previous generation.",
                self.message
            ),
            ErrorKind::GitConflict => format!(
                "Git conflict detected: {}. Manual resolution is required.",
                self.message
            ),
            ErrorKind::ApiTimeout => "The API request timed out. Retrying automatically...".to_string(),
            _ => format!("Error: {}", self.message),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub exponential: bool,
}

impl RetryPolicy {
    pub fn exponential(max_attempts: u32, base_delay: Duration) -> Self {
        Self {
            max_attempts,
            base_delay,
            exponential: true,
        }
    }

    pub fn fixed(max_attempts: u32, delay: Duration) -> Self {
        Self {
            max_attempts,
            base_delay: delay,
            exponential: false,
        }
    }

    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if self.exponential {
            self.base_delay * attempt
        } else {
            self.base_delay
        }
    }

    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }
}

pub struct RecoveryManager {
    default_policy: RetryPolicy,
}

impl RecoveryManager {
    pub fn new() -> Self {
        Self {
            default_policy: RetryPolicy::exponential(3, Duration::from_secs(1)),
        }
    }

    pub fn get_policy(&self, kind: ErrorKind) -> RetryPolicy {
        match kind {
            ErrorKind::ApiTimeout => RetryPolicy::exponential(5, Duration::from_millis(500)),
            ErrorKind::NixosBuild => RetryPolicy::fixed(2, Duration::from_secs(5)),
            ErrorKind::GitPush => RetryPolicy::exponential(3, Duration::from_secs(2)),
            _ => self.default_policy.clone(),
        }
    }

    pub async fn attempt_recovery<F, T, E>(
        &self,
        error: &AgentError,
        action: RecoveryAction,
        operation: F,
    ) -> Result<T, AgentError>
    where
        F: Fn() -> Result<T, E>,
        E: std::fmt::Display,
    {
        match action {
            RecoveryAction::Retry => {
                let policy = self.get_policy(error.kind);
                for attempt in 1..=policy.max_attempts {
                    match operation() {
                        Ok(result) => return Ok(result),
                        Err(e) if attempt < policy.max_attempts => {
                            tokio::time::sleep(policy.delay_for_attempt(attempt)).await;
                        }
                        Err(e) => {
                            return Err(AgentError::new(error.kind, e.to_string()));
                        }
                    }
                }
                Err(error.clone())
            }
            _ => Err(error.clone()),
        }
    }
}

impl Default for RecoveryManager {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 4: Create ErrorRecovery UI component**

Create `cvm-agent/web-ui/src/components/ErrorRecovery.tsx`:

```typescript
import React from 'react';

interface AgentError {
  kind: string;
  message: string;
  context?: string;
  suggestedActions: string[];
  userMessage: string;
}

interface Props {
  error: AgentError;
  onAction: (action: string) => void;
  onDismiss: () => void;
}

const ACTION_LABELS: Record<string, string> = {
  Retry: 'Try Again',
  Rollback: 'Rollback',
  ManualIntervention: 'I\'ll Fix It',
  RestartService: 'Restart Service',
  ClearCache: 'Clear Cache',
  AuthRefresh: 'Refresh Auth',
  AbortOperation: 'Cancel',
};

const ACTION_STYLES: Record<string, string> = {
  Retry: 'bg-indigo-600 hover:bg-indigo-500',
  Rollback: 'bg-yellow-600 hover:bg-yellow-500',
  ManualIntervention: 'bg-gray-600 hover:bg-gray-500',
  AbortOperation: 'bg-red-600 hover:bg-red-500',
};

export function ErrorRecovery({ error, onAction, onDismiss }: Props) {
  return (
    <div className="bg-red-900/30 border border-red-500/50 rounded-lg p-4 mb-4">
      <div className="flex items-start">
        <div className="flex-shrink-0">
          <svg
            className="w-5 h-5 text-red-400"
            fill="currentColor"
            viewBox="0 0 20 20"
          >
            <path
              fillRule="evenodd"
              d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z"
              clipRule="evenodd"
            />
          </svg>
        </div>
        <div className="ml-3 flex-1">
          <h3 className="text-sm font-medium text-red-300">
            {error.kind.replace(/([A-Z])/g, ' $1').trim()}
          </h3>
          <p className="mt-1 text-sm text-red-200">{error.userMessage}</p>

          {error.context && (
            <details className="mt-2">
              <summary className="text-xs text-red-400 cursor-pointer">
                Show details
              </summary>
              <pre className="mt-1 text-xs text-red-300 bg-red-900/30 p-2 rounded overflow-x-auto">
                {error.context}
              </pre>
            </details>
          )}

          <div className="mt-3 flex flex-wrap gap-2">
            {error.suggestedActions.map((action) => (
              <button
                key={action}
                onClick={() => onAction(action)}
                className={`px-3 py-1 text-sm text-white rounded ${
                  ACTION_STYLES[action] || 'bg-gray-600 hover:bg-gray-500'
                }`}
              >
                {ACTION_LABELS[action] || action}
              </button>
            ))}
          </div>
        </div>
        <button
          onClick={onDismiss}
          className="ml-2 text-red-400 hover:text-red-300"
        >
          <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
            <path
              fillRule="evenodd"
              d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
              clipRule="evenodd"
            />
          </svg>
        </button>
      </div>
    </div>
  );
}
```

**Step 5: Run tests**

Run: `cd cvm-agent && cargo test recovery`
Expected: All tests pass

**Step 6: Commit**

```bash
git add cvm-agent/agent-api/src/services/recovery.rs cvm-agent/agent-api/src/services/mod.rs cvm-agent/web-ui/src/components/ErrorRecovery.tsx
git commit -m "feat: add error recovery system with UI component"
```

---

## Task 10: Integration Test

**Files:**
- Modify: `cvm-agent/agent-api/src/main.rs` (ensure all services integrated)

**Step 1: Build release**

Run: `cd cvm-agent && cargo build --release`
Expected: Successful build

**Step 2: Run all Rust tests**

Run: `cd cvm-agent && cargo test`
Expected: All tests pass

**Step 3: Build web UI**

Run: `cd cvm-agent/web-ui && npm run build`
Expected: Successful build

**Step 4: TypeScript type check**

Run: `cd cvm-agent/web-ui && npm run typecheck`
Expected: No type errors

**Step 5: Run web UI tests**

Run: `cd cvm-agent/web-ui && npm test`
Expected: All tests pass

**Step 6: Verify database initialization**

Create test script to verify SQLite setup:

```bash
cd cvm-agent
./target/release/agent-api &
PID=$!
sleep 2
# Verify database file created
ls -la data/agent.db
kill $PID
```

Expected: Database file exists

**Step 7: Final commit**

```bash
git add .
git commit -m "feat: complete Phase 5 - Memory & Polish

Implements:
- MemoryService with SQLite backend
- Four memory layers: working, archive, facts, preferences
- Active forgetting with triggers and human confirmation
- Adaptive UI state machine (hidden/quick-input/expanded)
- Memory panel with search and pin functionality
- Error recovery system with suggested actions
- ForgetConfirmDialog for memory cleanup"
```

---

## Summary

Phase 5 delivers:

1. **Memory Storage**: SQLite-backed persistent memory with four distinct layers
2. **Active Forgetting**: Automatic cleanup triggers for stale data with human confirmation for important memories
3. **Adaptive UI**: State machine managing hidden/quick-input/expanded overlay states
4. **Memory UI**: Search, filter, pin, and forget confirmation interfaces
5. **Error Recovery**: Classified errors with suggested recovery actions and UI component

The memory system provides context awareness while preventing unbounded growth through the active forgetting mechanism. Users maintain control over what gets forgotten via the confirmation dialog for non-trivial memories.
