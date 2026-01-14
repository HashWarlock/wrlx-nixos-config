//! Git-related skill actions.

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

use crate::config::Config;
use crate::context::ServiceContext;
use crate::skills::registry::{ActionResult, SkillAction};

/// Checks git status and reports if there are changes.
pub struct GitStatusAction;

#[async_trait]
impl SkillAction for GitStatusAction {
    fn name(&self) -> &'static str {
        "git.status"
    }

    async fn execute(&self, _ctx: &ServiceContext, _args: serde_json::Value) -> Result<ActionResult> {
        let working_dir = &Config::get().paths.working_dir;

        let output = tokio::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(working_dir)
            .output()
            .await?;

        let has_changes = !output.stdout.is_empty();
        let mut data = HashMap::new();
        data.insert("has_changes".to_string(), serde_json::json!(has_changes));

        Ok(ActionResult::success_with_data(
            String::from_utf8_lossy(&output.stdout).to_string(),
            data,
        ))
    }
}

/// Stages files for commit.
pub struct GitAddAction;

#[async_trait]
impl SkillAction for GitAddAction {
    fn name(&self) -> &'static str {
        "git.add"
    }

    async fn execute(&self, _ctx: &ServiceContext, args: serde_json::Value) -> Result<ActionResult> {
        let working_dir = &Config::get().paths.working_dir;

        let paths: Vec<&str> = args
            .get("paths")
            .and_then(|p| p.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_else(|| vec!["."]);

        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("add").current_dir(working_dir);
        for path in paths {
            cmd.arg(path);
        }

        let output = cmd.output().await?;

        if output.status.success() {
            Ok(ActionResult::success("Files staged"))
        } else {
            Ok(ActionResult::failure(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }
}

/// Creates a git commit.
pub struct GitCommitAction;

#[async_trait]
impl SkillAction for GitCommitAction {
    fn name(&self) -> &'static str {
        "git.commit"
    }

    async fn execute(&self, _ctx: &ServiceContext, args: serde_json::Value) -> Result<ActionResult> {
        let working_dir = &Config::get().paths.working_dir;

        let message = args
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Update configuration");

        let output = tokio::process::Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(working_dir)
            .output()
            .await?;

        if output.status.success() {
            // Get the commit hash
            let hash_output = tokio::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(working_dir)
                .output()
                .await?;

            let hash = String::from_utf8_lossy(&hash_output.stdout).trim().to_string();
            let mut data = HashMap::new();
            data.insert("hash".to_string(), serde_json::json!(hash));

            Ok(ActionResult::success_with_data("Committed", data))
        } else {
            Ok(ActionResult::failure(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }
}

/// Pushes commits to remote.
pub struct GitPushAction;

#[async_trait]
impl SkillAction for GitPushAction {
    fn name(&self) -> &'static str {
        "git.push"
    }

    async fn execute(&self, _ctx: &ServiceContext, _args: serde_json::Value) -> Result<ActionResult> {
        let working_dir = &Config::get().paths.working_dir;

        let output = tokio::process::Command::new("git")
            .args(["push"])
            .current_dir(working_dir)
            .output()
            .await?;

        if output.status.success() {
            Ok(ActionResult::success("Pushed"))
        } else {
            Ok(ActionResult::failure(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }
}
