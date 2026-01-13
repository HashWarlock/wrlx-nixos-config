mod migrations;
mod repository;
mod schema;
#[cfg(test)]
mod tests;

pub use migrations::run_migrations;
pub use repository::{LessonsRepository, MemoryRepository};
pub use schema::*;

use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub type DbPool = Arc<Mutex<Connection>>;

/// Initialize the database, creating the file and running migrations
pub fn init_db(path: &str) -> Result<DbPool, rusqlite::Error> {
    // Ensure parent directory exists
    if let Some(parent) = Path::new(path).parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let conn = Connection::open(path)?;
    run_migrations(&conn)?;
    Ok(Arc::new(Mutex::new(conn)))
}

/// Initialize an in-memory database (useful for testing)
#[allow(dead_code)]
pub fn init_memory_db() -> Result<DbPool, rusqlite::Error> {
    let conn = Connection::open_in_memory()?;
    run_migrations(&conn)?;
    Ok(Arc::new(Mutex::new(conn)))
}
