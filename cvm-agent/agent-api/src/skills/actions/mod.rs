//! Built-in skill actions for the CVM Agent.
//!
//! This module contains all the default actions that can be used in workflows.
//!
//! # Adding a New Action (30 seconds)
//!
//! 1. Copy `template.rs.example` to `myaction.rs`
//! 2. Change the struct name and `name()` return value
//! 3. Implement `execute()` with your logic
//! 4. Add `mod myaction;` below
//! 5. Add `pub use myaction::MyAction;`
//! 6. Register in `register_all()`: `registry.register(MyAction)?;`
//!
//! See `SKILLS.md` in the agent-api directory for detailed documentation.

mod git;
mod nixops;
pub mod prelude;
mod prompt;

pub use git::{GitStatusAction, GitAddAction, GitCommitAction, GitPushAction};
pub use nixops::{NixopsRebuildAction, NixopsRollbackAction, NixopsListGenerationsAction};
pub use prompt::{PromptConfirmAction, ChatRespondAction};

use anyhow::Result;
use super::registry::SkillRegistry;

/// Registers all built-in actions with the registry.
///
/// Call this at startup to populate the registry with default actions.
///
/// # Errors
/// Returns an error if any action fails to register (e.g., duplicate name).
pub fn register_all(registry: &mut SkillRegistry) -> Result<()> {
    // Git actions
    registry.register(GitStatusAction)?;
    registry.register(GitAddAction)?;
    registry.register(GitCommitAction)?;
    registry.register(GitPushAction)?;

    // NixOS operations
    registry.register(NixopsRebuildAction)?;
    registry.register(NixopsRollbackAction)?;
    registry.register(NixopsListGenerationsAction)?;

    // Prompt/Chat actions
    registry.register(PromptConfirmAction)?;
    registry.register(ChatRespondAction)?;

    Ok(())
}
