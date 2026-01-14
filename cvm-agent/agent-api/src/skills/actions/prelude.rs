//! Prelude module for skill action development.
//!
//! This module re-exports all the types commonly needed when implementing
//! new skill actions. Import it with:
//!
//! ```ignore
//! use crate::skills::actions::prelude::*;
//! ```
//!
//! This gives you access to:
//! - `SkillAction` trait
//! - `ActionResult` for returning results
//! - `ServiceContext` for accessing services
//! - `async_trait` macro for async trait implementations
//! - Common std types (HashMap, etc.)
//! - `serde_json` for argument parsing
//! - `anyhow::Result` for error handling
//! - `Config` for accessing configuration

// Allow unused imports - these are re-exports for external action implementations
#![allow(unused_imports)]

// Core trait and result type
pub use crate::skills::registry::{ActionResult, SkillAction};

// Service context for accessing db, llm, etc.
pub use crate::context::ServiceContext;

// Configuration access
pub use crate::config::Config;

// Async trait macro (required for SkillAction impl)
pub use async_trait::async_trait;

// Error handling
pub use anyhow::Result;

// JSON handling for action arguments
pub use serde_json::{self, Value as JsonValue};

// Common standard library types
pub use std::collections::HashMap;
