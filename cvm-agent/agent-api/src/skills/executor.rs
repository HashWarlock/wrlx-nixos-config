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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_memory_db;
    use crate::llm::RedpillClient;
    use crate::skills::registry::{ActionResult, SkillAction};
    use crate::skills::types::{WorkflowSkill, WorkflowStep};
    use async_trait::async_trait;

    /// Mock action for testing workflow execution.
    struct MockAction {
        should_succeed: bool,
        output: String,
        data: HashMap<String, serde_json::Value>,
    }

    impl MockAction {
        fn new(should_succeed: bool, output: &str) -> Self {
            Self {
                should_succeed,
                output: output.to_string(),
                data: HashMap::new(),
            }
        }

        #[allow(dead_code)]
        fn with_data(mut self, key: &str, value: serde_json::Value) -> Self {
            self.data.insert(key.to_string(), value);
            self
        }
    }

    #[async_trait]
    impl SkillAction for MockAction {
        fn name(&self) -> &'static str {
            "mock.action"
        }

        async fn execute(
            &self,
            _ctx: &ServiceContext,
            _args: serde_json::Value,
        ) -> Result<ActionResult> {
            if self.should_succeed {
                Ok(ActionResult::success_with_data(&self.output, self.data.clone()))
            } else {
                Ok(ActionResult::failure("Mock failure"))
            }
        }
    }

    /// Creates a test ServiceContext with in-memory database.
    fn test_context() -> ServiceContext {
        let db = init_memory_db().expect("Failed to create test db");
        let llm = Arc::new(RedpillClient::new_dummy());
        ServiceContext::new(db, llm)
    }

    /// Creates a minimal workflow skill for testing.
    fn create_workflow(steps: Vec<WorkflowStep>) -> WorkflowSkill {
        WorkflowSkill {
            name: "test_workflow".to_string(),
            description: "A test workflow".to_string(),
            triggers: vec!["test".to_string()],
            parameters: vec![],
            steps,
            source: "test".to_string(),
        }
    }

    /// Creates a workflow step with minimal configuration.
    fn create_step(id: &str, action: &str) -> WorkflowStep {
        WorkflowStep {
            id: id.to_string(),
            action: action.to_string(),
            description: Some(format!("Step {}", id)),
            condition: None,
            args: None,
            stream: None,
            message: None,
        }
    }

    // =========================================================================
    // Test 1: test_execute_empty_workflow
    // =========================================================================
    #[tokio::test]
    async fn test_execute_empty_workflow() {
        // Create registry with mock action (not used but needed)
        let mut registry = SkillRegistry::new();
        registry
            .register(MockAction::new(true, "success"))
            .expect("Failed to register mock action");

        let ctx = test_context();
        let executor = WorkflowExecutor::new(Arc::new(registry), ctx);

        // Create workflow with zero steps
        let workflow = create_workflow(vec![]);

        // Create channel for progress updates
        let (tx, mut rx) = mpsc::channel(10);

        // Execute workflow
        let result = executor
            .execute(&workflow, HashMap::new(), tx)
            .await;

        assert!(result.is_ok(), "Empty workflow should complete without error");

        // Collect all messages
        let mut messages = vec![];
        while let Ok(msg) = rx.try_recv() {
            messages.push(msg);
        }

        // Should receive exactly one completion message
        assert_eq!(messages.len(), 1, "Should receive exactly one message for empty workflow");

        let (step_num, total_steps, description, output, error, is_complete) = &messages[0];
        assert_eq!(*step_num, 0, "Step number should be 0 for empty workflow");
        assert_eq!(*total_steps, 0, "Total steps should be 0");
        assert_eq!(description, "Workflow complete");
        assert!(output.is_none());
        assert!(error.is_none());
        assert!(*is_complete, "Completion flag should be true");
    }

    // =========================================================================
    // Test 2: test_execute_single_step
    // =========================================================================
    #[tokio::test]
    async fn test_execute_single_step() {
        // Create registry with mock action
        let mut registry = SkillRegistry::new();
        registry
            .register(MockAction::new(true, "Step executed successfully"))
            .expect("Failed to register mock action");

        let ctx = test_context();
        let executor = WorkflowExecutor::new(Arc::new(registry), ctx);

        // Create workflow with single step
        let workflow = create_workflow(vec![create_step("step1", "mock.action")]);

        let (tx, mut rx) = mpsc::channel(10);

        let result = executor
            .execute(&workflow, HashMap::new(), tx)
            .await;

        assert!(result.is_ok(), "Single step workflow should complete");

        // Collect messages
        let mut messages = vec![];
        while let Ok(msg) = rx.try_recv() {
            messages.push(msg);
        }

        // Should have: progress update, result update, completion
        assert!(messages.len() >= 2, "Should have at least 2 messages (step + completion)");

        // Verify step execution message
        let step_msg = messages.iter().find(|(_, _, _, output, _, _)| output.is_some());
        assert!(step_msg.is_some(), "Should have a message with output");
        let (step_num, total, _, output, error, _) = step_msg.unwrap();
        assert_eq!(*step_num, 1);
        assert_eq!(*total, 1);
        assert_eq!(output.as_ref().unwrap(), "Step executed successfully");
        assert!(error.is_none());

        // Verify completion message
        let completion = messages.last().unwrap();
        assert!(completion.5, "Last message should be completion");
        assert_eq!(completion.2, "Workflow complete");
    }

    // =========================================================================
    // Test 3: test_condition_evaluation
    // =========================================================================
    #[test]
    fn test_condition_evaluation() {
        // Test steps.X.success when success=true
        {
            let mut context = ExecutionContext {
                parameters: HashMap::new(),
                step_results: HashMap::new(),
            };
            context.step_results.insert(
                "check".to_string(),
                StepResult {
                    step_id: "check".to_string(),
                    success: true,
                    output: Some("done".to_string()),
                    error: None,
                    data: HashMap::new(),
                },
            );

            assert!(
                WorkflowExecutor::evaluate_condition("steps.check.success", &context),
                "steps.check.success should be true when success=true"
            );
        }

        // Test steps.X.success when success=false
        {
            let mut context = ExecutionContext {
                parameters: HashMap::new(),
                step_results: HashMap::new(),
            };
            context.step_results.insert(
                "check".to_string(),
                StepResult {
                    step_id: "check".to_string(),
                    success: false,
                    output: None,
                    error: Some("error".to_string()),
                    data: HashMap::new(),
                },
            );

            assert!(
                !WorkflowExecutor::evaluate_condition("steps.check.success", &context),
                "steps.check.success should be false when success=false"
            );
        }

        // Test steps.X.has_changes with data["has_changes"]=true
        {
            let mut context = ExecutionContext {
                parameters: HashMap::new(),
                step_results: HashMap::new(),
            };
            let mut data = HashMap::new();
            data.insert("has_changes".to_string(), serde_json::json!(true));
            context.step_results.insert(
                "check".to_string(),
                StepResult {
                    step_id: "check".to_string(),
                    success: true,
                    output: None,
                    error: None,
                    data,
                },
            );

            assert!(
                WorkflowExecutor::evaluate_condition("steps.check.has_changes", &context),
                "steps.check.has_changes should be true when data has has_changes=true"
            );
        }

        // Test steps.X.has_changes with data["has_changes"]=false
        {
            let mut context = ExecutionContext {
                parameters: HashMap::new(),
                step_results: HashMap::new(),
            };
            let mut data = HashMap::new();
            data.insert("has_changes".to_string(), serde_json::json!(false));
            context.step_results.insert(
                "check".to_string(),
                StepResult {
                    step_id: "check".to_string(),
                    success: true,
                    output: None,
                    error: None,
                    data,
                },
            );

            assert!(
                !WorkflowExecutor::evaluate_condition("steps.check.has_changes", &context),
                "steps.check.has_changes should be false when data has has_changes=false"
            );
        }

        // Test steps.X.confirmed with data["confirmed"]=true
        {
            let mut context = ExecutionContext {
                parameters: HashMap::new(),
                step_results: HashMap::new(),
            };
            let mut data = HashMap::new();
            data.insert("confirmed".to_string(), serde_json::json!(true));
            context.step_results.insert(
                "confirm".to_string(),
                StepResult {
                    step_id: "confirm".to_string(),
                    success: true,
                    output: None,
                    error: None,
                    data,
                },
            );

            assert!(
                WorkflowExecutor::evaluate_condition("steps.confirm.confirmed", &context),
                "steps.confirm.confirmed should be true when data has confirmed=true"
            );
        }

        // Test unknown step returns true (default behavior)
        {
            let context = ExecutionContext {
                parameters: HashMap::new(),
                step_results: HashMap::new(),
            };

            assert!(
                WorkflowExecutor::evaluate_condition("steps.nonexistent.success", &context),
                "Unknown step should default to true"
            );
        }
    }

    // =========================================================================
    // Test 4: test_template_resolution
    // =========================================================================
    #[test]
    fn test_template_resolution() {
        let mut context = ExecutionContext {
            parameters: HashMap::new(),
            step_results: HashMap::new(),
        };

        // Add parameters
        context.parameters.insert("name".to_string(), "Alice".to_string());
        context.parameters.insert("project".to_string(), "TestProject".to_string());

        // Add step result with output and data
        let mut step_data = HashMap::new();
        step_data.insert("my_key".to_string(), serde_json::json!("my_value"));
        step_data.insert("count".to_string(), serde_json::json!(42));
        context.step_results.insert(
            "step1".to_string(),
            StepResult {
                step_id: "step1".to_string(),
                success: true,
                output: Some("Step 1 output".to_string()),
                error: None,
                data: step_data,
            },
        );

        // Test {{ parameters.name }} resolution
        let input = serde_json::json!("Hello {{ parameters.name }}!");
        let result = WorkflowExecutor::resolve_templates(&input, &context);
        assert_eq!(
            result,
            serde_json::json!("Hello Alice!"),
            "Should resolve parameter reference"
        );

        // Test {{ steps.X.output }} resolution
        let input = serde_json::json!("Output was: {{ steps.step1.output }}");
        let result = WorkflowExecutor::resolve_templates(&input, &context);
        assert_eq!(
            result,
            serde_json::json!("Output was: Step 1 output"),
            "Should resolve step output reference"
        );

        // Test {{ steps.X.data_key }} resolution
        let input = serde_json::json!("Key value: {{ steps.step1.my_key }}");
        let result = WorkflowExecutor::resolve_templates(&input, &context);
        assert_eq!(
            result,
            serde_json::json!("Key value: \"my_value\""),
            "Should resolve step data reference"
        );

        // Test multiple templates in one string
        let input = serde_json::json!("{{ parameters.name }} working on {{ parameters.project }}");
        let result = WorkflowExecutor::resolve_templates(&input, &context);
        assert_eq!(
            result,
            serde_json::json!("Alice working on TestProject"),
            "Should resolve multiple templates"
        );

        // Test template resolution in nested object
        let input = serde_json::json!({
            "greeting": "Hello {{ parameters.name }}",
            "nested": {
                "value": "{{ steps.step1.output }}"
            }
        });
        let result = WorkflowExecutor::resolve_templates(&input, &context);
        assert_eq!(
            result["greeting"],
            serde_json::json!("Hello Alice"),
            "Should resolve templates in objects"
        );
        assert_eq!(
            result["nested"]["value"],
            serde_json::json!("Step 1 output"),
            "Should resolve templates in nested objects"
        );

        // Test template resolution in array
        let input = serde_json::json!(["{{ parameters.name }}", "{{ parameters.project }}"]);
        let result = WorkflowExecutor::resolve_templates(&input, &context);
        assert_eq!(
            result[0],
            serde_json::json!("Alice"),
            "Should resolve templates in arrays"
        );
        assert_eq!(
            result[1],
            serde_json::json!("TestProject"),
            "Should resolve templates in arrays"
        );
    }

    // =========================================================================
    // Test 5: test_invalid_condition_defaults_true
    // =========================================================================
    #[test]
    fn test_invalid_condition_defaults_true() {
        let context = ExecutionContext {
            parameters: HashMap::new(),
            step_results: HashMap::new(),
        };

        // Test completely invalid condition
        assert!(
            WorkflowExecutor::evaluate_condition("invalid", &context),
            "Invalid condition should default to true"
        );

        // Test partial "steps." without proper format
        assert!(
            WorkflowExecutor::evaluate_condition("steps.", &context),
            "Malformed 'steps.' should default to true"
        );

        // Test unrelated format
        assert!(
            WorkflowExecutor::evaluate_condition("foo.bar.baz", &context),
            "Unrelated condition format should default to true"
        );

        // Test empty string
        assert!(
            WorkflowExecutor::evaluate_condition("", &context),
            "Empty condition should default to true"
        );

        // Test "steps" without dot
        assert!(
            WorkflowExecutor::evaluate_condition("steps", &context),
            "'steps' without dot should default to true"
        );

        // Test steps.X with only one part after "steps."
        assert!(
            WorkflowExecutor::evaluate_condition("steps.onlyid", &context),
            "steps.X with only step_id should default to true"
        );
    }

    // =========================================================================
    // Test 6: test_progress_channel_communication
    // =========================================================================
    #[tokio::test]
    async fn test_progress_channel_communication() {
        // Create registry with mock action
        let mut registry = SkillRegistry::new();
        registry
            .register(MockAction::new(true, "Action completed"))
            .expect("Failed to register mock action");

        let ctx = test_context();
        let executor = WorkflowExecutor::new(Arc::new(registry), ctx);

        // Create workflow with two steps
        let workflow = create_workflow(vec![
            WorkflowStep {
                id: "step1".to_string(),
                action: "mock.action".to_string(),
                description: Some("First step".to_string()),
                condition: None,
                args: None,
                stream: None,
                message: None,
            },
            WorkflowStep {
                id: "step2".to_string(),
                action: "mock.action".to_string(),
                description: Some("Second step".to_string()),
                condition: None,
                args: None,
                stream: None,
                message: None,
            },
        ]);

        let (tx, mut rx) = mpsc::channel(20);

        let result = executor
            .execute(&workflow, HashMap::new(), tx)
            .await;

        assert!(result.is_ok(), "Workflow should complete successfully");

        // Collect all messages
        let mut messages = vec![];
        while let Ok(msg) = rx.try_recv() {
            messages.push(msg);
        }

        // Verify we got the expected number of messages
        // For 2 steps: 2 progress updates + 2 result updates + 1 completion = 5
        // Actually: progress update before execution, then result after each step, then completion
        assert!(
            messages.len() >= 3,
            "Should have at least 3 messages for 2-step workflow (got {})",
            messages.len()
        );

        // Verify step 1 progress (first message for step 1)
        let step1_progress = messages.iter().find(|(num, _, desc, _, _, _)| {
            *num == 1 && desc == "First step"
        });
        assert!(step1_progress.is_some(), "Should have progress for step 1");

        // Verify step 2 progress
        let step2_progress = messages.iter().find(|(num, _, desc, _, _, _)| {
            *num == 2 && desc == "Second step"
        });
        assert!(step2_progress.is_some(), "Should have progress for step 2");

        // Verify completion message is last
        let completion = messages.last().unwrap();
        assert_eq!(completion.0, 2, "Completion step_num should equal total_steps");
        assert_eq!(completion.1, 2, "Completion total_steps should be 2");
        assert_eq!(completion.2, "Workflow complete");
        assert!(completion.5, "Completion is_complete should be true");

        // Verify all non-completion messages have correct total_steps
        for msg in messages.iter().take(messages.len() - 1) {
            assert_eq!(msg.1, 2, "All messages should have total_steps=2");
        }

        // Verify step numbers are sequential for step messages
        let step_nums: Vec<i32> = messages
            .iter()
            .filter(|(_, _, _, _, _, complete)| !complete)
            .map(|(num, _, _, _, _, _)| *num)
            .collect();

        // Should have step 1 and step 2 messages (may have duplicates for progress + result)
        assert!(step_nums.contains(&1), "Should have step 1 messages");
        assert!(step_nums.contains(&2), "Should have step 2 messages");
    }
}
