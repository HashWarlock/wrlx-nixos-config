//! Service context for skill actions.
//!
//! Provides access to shared services (database, LLM client) that skills
//! may need during execution. This enables dependency injection without
//! requiring skills to manage their own connections.

use std::sync::Arc;

use crate::db::DbPool;
use crate::llm::RedpillClient;

/// Context providing access to shared services for skill execution.
///
/// The context is passed to skill actions during execution, allowing them
/// to access database, LLM, and other services without managing connections.
///
/// # Example
///
/// ```ignore
/// #[async_trait]
/// impl SkillAction for MyAction {
///     async fn execute(&self, ctx: &ServiceContext, args: Value) -> Result<ActionResult> {
///         // Access LLM if needed
///         if let Some(llm) = ctx.llm() {
///             // Use LLM for reasoning
///         }
///
///         // Access database if needed
///         let db = ctx.db();
///         // Query database
///
///         Ok(ActionResult::success("Done"))
///     }
/// }
/// ```
#[derive(Clone)]
pub struct ServiceContext {
    /// Database connection pool (available for future actions)
    #[allow(dead_code)]
    db: DbPool,
    /// LLM client (may be dummy if not configured; available for future actions)
    #[allow(dead_code)]
    llm: Arc<RedpillClient>,
}

impl ServiceContext {
    /// Creates a new service context.
    ///
    /// # Arguments
    /// * `db` - Database connection pool
    /// * `llm` - LLM client instance
    pub fn new(db: DbPool, llm: Arc<RedpillClient>) -> Self {
        Self { db, llm }
    }

    /// Returns a reference to the database connection pool.
    #[allow(dead_code)]
    pub fn db(&self) -> &DbPool {
        &self.db
    }

    /// Returns a reference to the LLM client.
    ///
    /// Note: The client may be a dummy if REDPILL_API_KEY was not configured.
    /// Skills should handle this gracefully.
    #[allow(dead_code)]
    pub fn llm(&self) -> &Arc<RedpillClient> {
        &self.llm
    }
}
