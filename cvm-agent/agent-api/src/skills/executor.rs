//! Workflow executor using the skill registry.
//!
//! Executes workflow steps by dispatching actions through the registry.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::context::ServiceContext;
use super::registry::SkillRegistry;
use super::types::{WorkflowSkill, WorkflowStep};

/// Executor for workflow skills.
pub struct WorkflowExecutor {
    registry: Arc<SkillRegistry>,
    ctx: ServiceContext,
}

/// Result of executing a workflow step.
#[derive(Debug, Clone)]
pub struct StepResult {
    #[allow(dead_code)]
    pub step_id: String,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub data: HashMap<String, serde_json::Value>,
}

/// Context for workflow execution, tracking parameters and step results.
#[derive(Debug)]
pub struct ExecutionContext {
    pub parameters: HashMap<String, String>,
    pub step_results: HashMap<String, StepResult>,
}

impl WorkflowExecutor {
    /// Creates a new executor with the given registry and service context.
    pub fn new(registry: Arc<SkillRegistry>, ctx: ServiceContext) -> Self {
        Self { registry, ctx }
    }

    /// Executes a workflow skill.
    ///
    /// # Arguments
    /// * `skill` - The workflow to execute
    /// * `parameters` - Parameters for the workflow
    /// * `progress_tx` - Channel for progress updates
    pub async fn execute(
        &self,
        skill: &WorkflowSkill,
        parameters: HashMap<String, String>,
        progress_tx: mpsc::Sender<(i32, i32, String, Option<String>, Option<String>, bool)>,
    ) -> Result<()> {
        let mut context = ExecutionContext {
            parameters,
            step_results: HashMap::new(),
        };

        let total_steps = skill.steps.len() as i32;

        for (idx, step) in skill.steps.iter().enumerate() {
            let step_num = (idx + 1) as i32;
            let description = step.description.clone().unwrap_or_else(|| step.action.clone());

            // Check condition
            if let Some(condition) = &step.condition {
                if !Self::evaluate_condition(condition, &context) {
                    let _ = progress_tx
                        .send((
                            step_num,
                            total_steps,
                            format!("Skipped: {}", description),
                            Some("Condition not met".to_string()),
                            None,
                            false,
                        ))
                        .await;
                    continue;
                }
            }

            // Send progress update
            let _ = progress_tx
                .send((step_num, total_steps, description.clone(), None, None, false))
                .await;

            // Execute step using registry
            let result = self.execute_step(step, &context).await;

            match &result {
                Ok(step_result) => {
                    context
                        .step_results
                        .insert(step.id.clone(), step_result.clone());

                    if step_result.success {
                        let _ = progress_tx
                            .send((
                                step_num,
                                total_steps,
                                description,
                                step_result.output.clone(),
                                None,
                                false,
                            ))
                            .await;
                    } else {
                        let _ = progress_tx
                            .send((
                                step_num,
                                total_steps,
                                description,
                                None,
                                step_result.error.clone(),
                                false,
                            ))
                            .await;
                        break;
                    }
                }
                Err(e) => {
                    let _ = progress_tx
                        .send((
                            step_num,
                            total_steps,
                            description,
                            None,
                            Some(e.to_string()),
                            false,
                        ))
                        .await;
                    break;
                }
            }
        }

        // Send completion
        let _ = progress_tx
            .send((
                total_steps,
                total_steps,
                "Workflow complete".to_string(),
                None,
                None,
                true,
            ))
            .await;

        Ok(())
    }

    /// Executes a single workflow step using the registry.
    async fn execute_step(
        &self,
        step: &WorkflowStep,
        context: &ExecutionContext,
    ) -> Result<StepResult> {
        // Resolve templates in args
        let args = step
            .args
            .as_ref()
            .map(|a| Self::resolve_templates(a, context))
            .unwrap_or_else(|| serde_json::json!({}));

        // For chat.respond, we need to include the resolved message in args
        let args = if step.action == "chat.respond" {
            let message = step.message.as_ref().map(|m| {
                Self::resolve_templates(&serde_json::Value::String(m.clone()), context)
            });
            let mut args_map = args.as_object().cloned().unwrap_or_default();
            if let Some(serde_json::Value::String(msg)) = message {
                args_map.insert("message".to_string(), serde_json::Value::String(msg));
            }
            serde_json::Value::Object(args_map)
        } else {
            args
        };

        // Execute via registry with service context
        let action_result = self.registry.execute(&step.action, &self.ctx, args).await?;

        Ok(StepResult {
            step_id: step.id.clone(),
            success: action_result.success,
            output: action_result.output,
            error: action_result.error,
            data: action_result.data,
        })
    }

    /// Evaluates a condition expression.
    fn evaluate_condition(condition: &str, context: &ExecutionContext) -> bool {
        if condition.starts_with("steps.") {
            let parts: Vec<&str> = condition[6..].split('.').collect();
            if parts.len() >= 2 {
                let step_id = parts[0];
                let field = parts[1];

                if let Some(result) = context.step_results.get(step_id) {
                    return match field {
                        "success" => result.success,
                        "has_changes" => result
                            .data
                            .get("has_changes")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        "confirmed" => result
                            .data
                            .get("confirmed")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        _ => false,
                    };
                }
            }
        }
        true
    }

    /// Resolves template expressions in a JSON value.
    fn resolve_templates(
        value: &serde_json::Value,
        context: &ExecutionContext,
    ) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) => {
                let mut result = s.clone();

                // Resolve parameter references
                for (key, val) in &context.parameters {
                    let pattern = format!("{{{{ parameters.{} }}}}", key);
                    result = result.replace(&pattern, val);
                }

                // Resolve step result references
                for (step_id, step_result) in &context.step_results {
                    if let Some(output) = &step_result.output {
                        let pattern = format!("{{{{ steps.{}.output }}}}", step_id);
                        result = result.replace(&pattern, output);
                    }
                    for (key, val) in &step_result.data {
                        let pattern = format!("{{{{ steps.{}.{} }}}}", step_id, key);
                        result = result.replace(&pattern, &val.to_string());
                    }
                }

                serde_json::Value::String(result)
            }
            serde_json::Value::Object(map) => {
                let resolved: serde_json::Map<String, serde_json::Value> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::resolve_templates(v, context)))
                    .collect();
                serde_json::Value::Object(resolved)
            }
            serde_json::Value::Array(arr) => {
                let resolved: Vec<serde_json::Value> = arr
                    .iter()
                    .map(|v| Self::resolve_templates(v, context))
                    .collect();
                serde_json::Value::Array(resolved)
            }
            _ => value.clone(),
        }
    }
}
