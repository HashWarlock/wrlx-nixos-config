use anyhow::Result;
use std::collections::HashMap;
use tokio::sync::mpsc;

use super::types::{WorkflowSkill, WorkflowStep};

pub struct WorkflowExecutor;

#[derive(Debug, Clone)]
pub struct StepResult {
    pub step_id: String,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub data: HashMap<String, serde_json::Value>,
}

#[derive(Debug)]
pub struct ExecutionContext {
    pub parameters: HashMap<String, String>,
    pub step_results: HashMap<String, StepResult>,
}

impl WorkflowExecutor {
    pub async fn execute(
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
                    let _ = progress_tx.send((
                        step_num,
                        total_steps,
                        format!("Skipped: {}", description),
                        Some("Condition not met".to_string()),
                        None,
                        false,
                    )).await;
                    continue;
                }
            }

            // Send progress update
            let _ = progress_tx.send((
                step_num,
                total_steps,
                description.clone(),
                None,
                None,
                false,
            )).await;

            // Execute step
            let result = Self::execute_step(step, &context).await;

            match &result {
                Ok(step_result) => {
                    context.step_results.insert(step.id.clone(), step_result.clone());

                    if step_result.success {
                        let _ = progress_tx.send((
                            step_num,
                            total_steps,
                            description,
                            step_result.output.clone(),
                            None,
                            false,
                        )).await;
                    } else {
                        let _ = progress_tx.send((
                            step_num,
                            total_steps,
                            description,
                            None,
                            step_result.error.clone(),
                            false,
                        )).await;
                        break;
                    }
                }
                Err(e) => {
                    let _ = progress_tx.send((
                        step_num,
                        total_steps,
                        description,
                        None,
                        Some(e.to_string()),
                        false,
                    )).await;
                    break;
                }
            }
        }

        // Send completion
        let _ = progress_tx.send((
            total_steps,
            total_steps,
            "Workflow complete".to_string(),
            None,
            None,
            true,
        )).await;

        Ok(())
    }

    async fn execute_step(step: &WorkflowStep, context: &ExecutionContext) -> Result<StepResult> {
        let action_parts: Vec<&str> = step.action.split('.').collect();
        let service = action_parts.get(0).unwrap_or(&"");
        let method = action_parts.get(1).unwrap_or(&"");

        let args = step.args.as_ref().map(|a| Self::resolve_templates(a, context));

        match (*service, *method) {
            ("git", "status") => Self::action_git_status().await,
            ("git", "add") => Self::action_git_add(&args).await,
            ("git", "commit") => Self::action_git_commit(&args).await,
            ("git", "push") => Self::action_git_push().await,
            ("nixops", "rebuild") => Self::action_nixops_rebuild(&args).await,
            ("nixops", "rollback") => Self::action_nixops_rollback(&args).await,
            ("nixops", "list_generations") => Self::action_nixops_list_generations().await,
            ("prompt", "confirm") => Self::action_prompt_confirm(&step.message).await,
            ("chat", "respond") => Self::action_chat_respond(&step.message, context).await,
            _ => Ok(StepResult {
                step_id: step.id.clone(),
                success: false,
                output: None,
                error: Some(format!("Unknown action: {}", step.action)),
                data: HashMap::new(),
            }),
        }
    }

    fn evaluate_condition(condition: &str, context: &ExecutionContext) -> bool {
        if condition.starts_with("steps.") {
            let parts: Vec<&str> = condition[6..].split('.').collect();
            if parts.len() >= 2 {
                let step_id = parts[0];
                let field = parts[1];

                if let Some(result) = context.step_results.get(step_id) {
                    return match field {
                        "success" => result.success,
                        "has_changes" => result.data.get("has_changes")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        "confirmed" => result.data.get("confirmed")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        _ => false,
                    };
                }
            }
        }
        true
    }

    fn resolve_templates(value: &serde_json::Value, context: &ExecutionContext) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) => {
                let mut result = s.clone();

                for (key, val) in &context.parameters {
                    let pattern = format!("{{{{ parameters.{} }}}}", key);
                    result = result.replace(&pattern, val);
                }

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

    // Action implementations
    async fn action_git_status() -> Result<StepResult> {
        let output = tokio::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir("/app")
            .output()
            .await?;

        let has_changes = !output.stdout.is_empty();
        let mut data = HashMap::new();
        data.insert("has_changes".to_string(), serde_json::json!(has_changes));

        Ok(StepResult {
            step_id: "git_status".to_string(),
            success: true,
            output: Some(String::from_utf8_lossy(&output.stdout).to_string()),
            error: None,
            data,
        })
    }

    async fn action_git_add(args: &Option<serde_json::Value>) -> Result<StepResult> {
        let paths = args
            .as_ref()
            .and_then(|a| a.get("paths"))
            .and_then(|p| p.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_else(|| vec!["."]);

        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("add").current_dir("/app");
        for path in paths {
            cmd.arg(path);
        }

        let output = cmd.output().await?;

        Ok(StepResult {
            step_id: "git_add".to_string(),
            success: output.status.success(),
            output: Some("Files staged".to_string()),
            error: if output.status.success() { None } else {
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            },
            data: HashMap::new(),
        })
    }

    async fn action_git_commit(args: &Option<serde_json::Value>) -> Result<StepResult> {
        let message = args
            .as_ref()
            .and_then(|a| a.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("Update configuration");

        let output = tokio::process::Command::new("git")
            .args(["commit", "-m", message])
            .current_dir("/app")
            .output()
            .await?;

        let mut data = HashMap::new();
        if output.status.success() {
            let hash_output = tokio::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir("/app")
                .output()
                .await?;
            let hash = String::from_utf8_lossy(&hash_output.stdout).trim().to_string();
            data.insert("hash".to_string(), serde_json::json!(hash));
        }

        Ok(StepResult {
            step_id: "git_commit".to_string(),
            success: output.status.success(),
            output: if output.status.success() { Some("Committed".to_string()) } else { None },
            error: if output.status.success() { None } else {
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            },
            data,
        })
    }

    async fn action_git_push() -> Result<StepResult> {
        let output = tokio::process::Command::new("git")
            .args(["push"])
            .current_dir("/app")
            .output()
            .await?;

        Ok(StepResult {
            step_id: "git_push".to_string(),
            success: output.status.success(),
            output: if output.status.success() { Some("Pushed".to_string()) } else { None },
            error: if output.status.success() { None } else {
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            },
            data: HashMap::new(),
        })
    }

    async fn action_nixops_rebuild(args: &Option<serde_json::Value>) -> Result<StepResult> {
        let action = args
            .as_ref()
            .and_then(|a| a.get("action"))
            .and_then(|a| a.as_str())
            .unwrap_or("switch");

        let output = tokio::process::Command::new("nixos-rebuild")
            .args([action, "--flake", "/app#phala-cvm"])
            .output()
            .await?;

        let mut data = HashMap::new();
        data.insert("status".to_string(), serde_json::json!(
            if output.status.success() { "success" } else { "failed" }
        ));

        Ok(StepResult {
            step_id: "nixops_rebuild".to_string(),
            success: output.status.success(),
            output: Some(String::from_utf8_lossy(&output.stdout).to_string()),
            error: if output.status.success() { None } else {
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            },
            data,
        })
    }

    async fn action_nixops_rollback(args: &Option<serde_json::Value>) -> Result<StepResult> {
        let generation = args
            .as_ref()
            .and_then(|a| a.get("generation"))
            .and_then(|g| g.as_str());

        let mut cmd = tokio::process::Command::new("nixos-rebuild");
        cmd.arg("switch").arg("--rollback");
        if let Some(gen) = generation {
            cmd.arg("--generation").arg(gen);
        }

        let output = cmd.output().await?;

        Ok(StepResult {
            step_id: "nixops_rollback".to_string(),
            success: output.status.success(),
            output: Some("Rollback complete".to_string()),
            error: if output.status.success() { None } else {
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            },
            data: HashMap::new(),
        })
    }

    async fn action_nixops_list_generations() -> Result<StepResult> {
        let output = tokio::process::Command::new("nix-env")
            .args(["--list-generations", "-p", "/nix/var/nix/profiles/system"])
            .output()
            .await?;

        let mut data = HashMap::new();
        data.insert("list".to_string(), serde_json::json!(
            String::from_utf8_lossy(&output.stdout).to_string()
        ));

        Ok(StepResult {
            step_id: "nixops_list_generations".to_string(),
            success: output.status.success(),
            output: Some(String::from_utf8_lossy(&output.stdout).to_string()),
            error: None,
            data,
        })
    }

    async fn action_prompt_confirm(_message: &Option<String>) -> Result<StepResult> {
        let mut data = HashMap::new();
        data.insert("confirmed".to_string(), serde_json::json!(true));

        Ok(StepResult {
            step_id: "prompt_confirm".to_string(),
            success: true,
            output: Some("Confirmed".to_string()),
            error: None,
            data,
        })
    }

    async fn action_chat_respond(message: &Option<String>, context: &ExecutionContext) -> Result<StepResult> {
        let resolved_message = message.clone().map(|m| {
            let mut result = m;
            for (key, val) in &context.parameters {
                result = result.replace(&format!("{{{{ parameters.{} }}}}", key), val);
            }
            for (step_id, step_result) in &context.step_results {
                if let Some(output) = &step_result.output {
                    result = result.replace(&format!("{{{{ steps.{}.output }}}}", step_id), output);
                }
                for (k, v) in &step_result.data {
                    result = result.replace(&format!("{{{{ steps.{}.{} }}}}", step_id, k), &v.to_string());
                }
            }
            result
        });

        Ok(StepResult {
            step_id: "chat_respond".to_string(),
            success: true,
            output: resolved_message,
            error: None,
            data: HashMap::new(),
        })
    }
}
