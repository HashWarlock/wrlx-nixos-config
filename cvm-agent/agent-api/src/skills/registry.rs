//! Skill action registry for dynamic action dispatch.
//!
//! This module provides a trait-based registry that allows actions to be
//! registered at runtime and dispatched by name. This enables adding new
//! automation skills without modifying the core executor code.

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::context::ServiceContext;

/// Result of executing a skill action.
#[derive(Debug, Clone)]
pub struct ActionResult {
    /// Whether the action succeeded.
    pub success: bool,
    /// Output message from the action.
    pub output: Option<String>,
    /// Error message if the action failed.
    pub error: Option<String>,
    /// Additional data produced by the action.
    pub data: HashMap<String, serde_json::Value>,
}

impl ActionResult {
    /// Creates a successful result with an output message.
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            data: HashMap::new(),
        }
    }

    /// Creates a successful result with output and data.
    pub fn success_with_data(output: impl Into<String>, data: HashMap<String, serde_json::Value>) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            data,
        }
    }

    /// Creates a failed result with an error message.
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(error.into()),
            data: HashMap::new(),
        }
    }
}

/// Trait for implementing skill actions.
///
/// Actions receive a `ServiceContext` providing access to shared services
/// like the database and LLM client, enabling rich integrations.
///
/// # Example
///
/// ```ignore
/// use async_trait::async_trait;
/// use crate::context::ServiceContext;
///
/// pub struct MyAction;
///
/// #[async_trait]
/// impl SkillAction for MyAction {
///     fn name(&self) -> &'static str { "my.action" }
///
///     async fn execute(&self, ctx: &ServiceContext, args: serde_json::Value) -> Result<ActionResult> {
///         // Access services via ctx.db(), ctx.llm()
///         Ok(ActionResult::success("Done!"))
///     }
/// }
/// ```
#[async_trait]
pub trait SkillAction: Send + Sync + 'static {
    /// Returns the action name in "service.method" format (e.g., "git.status").
    fn name(&self) -> &'static str;

    /// Executes the action with the given arguments and service context.
    ///
    /// # Arguments
    /// * `ctx` - Service context providing access to database, LLM, etc.
    /// * `args` - JSON value containing action-specific arguments
    ///
    /// # Returns
    /// An `ActionResult` indicating success/failure and any output data.
    async fn execute(&self, ctx: &ServiceContext, args: serde_json::Value) -> Result<ActionResult>;
}

/// Registry for skill actions.
///
/// Allows dynamic registration and lookup of actions by name.
/// Actions are stored as trait objects for runtime dispatch.
pub struct SkillRegistry {
    actions: HashMap<String, Arc<dyn SkillAction>>,
}

impl SkillRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            actions: HashMap::new(),
        }
    }

    /// Registers an action with the registry.
    ///
    /// # Arguments
    /// * `action` - The action to register
    ///
    /// # Panics
    /// Panics if an action with the same name is already registered.
    pub fn register<T: SkillAction + 'static>(&mut self, action: T) {
        let name = action.name().to_string();
        if self.actions.contains_key(&name) {
            panic!("Action '{}' is already registered", name);
        }
        self.actions.insert(name, Arc::new(action));
    }

    /// Gets an action by name.
    #[allow(dead_code)]
    pub fn get(&self, name: &str) -> Option<Arc<dyn SkillAction>> {
        self.actions.get(name).cloned()
    }

    /// Executes an action by name.
    ///
    /// # Arguments
    /// * `name` - The action name (e.g., "git.status")
    /// * `ctx` - Service context to pass to the action
    /// * `args` - Arguments to pass to the action
    ///
    /// # Returns
    /// The action result, or an error if the action is not found.
    pub async fn execute(&self, name: &str, ctx: &ServiceContext, args: serde_json::Value) -> Result<ActionResult> {
        match self.actions.get(name) {
            Some(action) => action.execute(ctx, args).await,
            None => Ok(ActionResult::failure(format!("Unknown action: {}", name))),
        }
    }

    /// Returns the number of registered actions.
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Returns true if no actions are registered.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Returns a list of all registered action names.
    #[allow(dead_code)]
    pub fn list_actions(&self) -> Vec<&str> {
        self.actions.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_memory_db;
    use crate::llm::RedpillClient;

    struct TestAction;

    #[async_trait]
    impl SkillAction for TestAction {
        fn name(&self) -> &'static str {
            "test.action"
        }

        async fn execute(&self, _ctx: &ServiceContext, _args: serde_json::Value) -> Result<ActionResult> {
            Ok(ActionResult::success("Test completed"))
        }
    }

    fn test_context() -> ServiceContext {
        let db = init_memory_db().expect("Failed to create test db");
        let llm = Arc::new(RedpillClient::new_dummy());
        ServiceContext::new(db, llm)
    }

    #[tokio::test]
    async fn test_register_and_execute() {
        let mut registry = SkillRegistry::new();
        registry.register(TestAction);

        assert_eq!(registry.len(), 1);
        assert!(registry.get("test.action").is_some());

        let ctx = test_context();
        let result = registry.execute("test.action", &ctx, serde_json::json!({})).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output, Some("Test completed".to_string()));
    }

    #[tokio::test]
    async fn test_unknown_action() {
        let registry = SkillRegistry::new();
        let ctx = test_context();
        let result = registry.execute("unknown.action", &ctx, serde_json::json!({})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Unknown action"));
    }

    #[test]
    fn test_list_actions() {
        let mut registry = SkillRegistry::new();
        registry.register(TestAction);

        let actions = registry.list_actions();
        assert_eq!(actions.len(), 1);
        assert!(actions.contains(&"test.action"));
    }
}
