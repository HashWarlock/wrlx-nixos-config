//! Prompt and chat skill actions.

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

use crate::context::ServiceContext;
use crate::skills::registry::{ActionResult, SkillAction};

/// Prompts for confirmation (auto-confirms in automated context).
pub struct PromptConfirmAction;

#[async_trait]
impl SkillAction for PromptConfirmAction {
    fn name(&self) -> &'static str {
        "prompt.confirm"
    }

    async fn execute(&self, _ctx: &ServiceContext, _args: serde_json::Value) -> Result<ActionResult> {
        // In automated workflow context, we auto-confirm
        // A real implementation might integrate with a UI or wait for user input
        let mut data = HashMap::new();
        data.insert("confirmed".to_string(), serde_json::json!(true));

        Ok(ActionResult::success_with_data("Confirmed", data))
    }
}

/// Responds with a message (used for workflow output).
pub struct ChatRespondAction;

#[async_trait]
impl SkillAction for ChatRespondAction {
    fn name(&self) -> &'static str {
        "chat.respond"
    }

    async fn execute(&self, _ctx: &ServiceContext, args: serde_json::Value) -> Result<ActionResult> {
        // The message should be pre-resolved by the executor
        let message = args
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();

        Ok(ActionResult::success(message))
    }
}
