//! NixOS operations skill actions.

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

use crate::config::Config;
use crate::context::ServiceContext;
use crate::skills::registry::{ActionResult, SkillAction};

/// Rebuilds the NixOS system.
pub struct NixopsRebuildAction;

#[async_trait]
impl SkillAction for NixopsRebuildAction {
    fn name(&self) -> &'static str {
        "nixops.rebuild"
    }

    async fn execute(&self, _ctx: &ServiceContext, args: serde_json::Value) -> Result<ActionResult> {
        let flake_ref = &Config::get().paths.flake_ref;

        let action = args
            .get("action")
            .and_then(|a| a.as_str())
            .unwrap_or("switch");

        let output = tokio::process::Command::new("nixos-rebuild")
            .args([action, "--flake", flake_ref])
            .output()
            .await?;

        let mut data = HashMap::new();
        data.insert(
            "status".to_string(),
            serde_json::json!(if output.status.success() { "success" } else { "failed" }),
        );

        if output.status.success() {
            Ok(ActionResult::success_with_data(
                String::from_utf8_lossy(&output.stdout).to_string(),
                data,
            ))
        } else {
            Ok(ActionResult::failure(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }
}

/// Rolls back to a previous NixOS generation.
pub struct NixopsRollbackAction;

#[async_trait]
impl SkillAction for NixopsRollbackAction {
    fn name(&self) -> &'static str {
        "nixops.rollback"
    }

    async fn execute(&self, _ctx: &ServiceContext, args: serde_json::Value) -> Result<ActionResult> {
        let generation = args.get("generation").and_then(|g| g.as_str());

        let mut cmd = tokio::process::Command::new("nixos-rebuild");
        cmd.arg("switch").arg("--rollback");
        if let Some(gen) = generation {
            cmd.arg("--generation").arg(gen);
        }

        let output = cmd.output().await?;

        if output.status.success() {
            Ok(ActionResult::success("Rollback complete"))
        } else {
            Ok(ActionResult::failure(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }
}

/// Lists available NixOS generations.
pub struct NixopsListGenerationsAction;

#[async_trait]
impl SkillAction for NixopsListGenerationsAction {
    fn name(&self) -> &'static str {
        "nixops.list_generations"
    }

    async fn execute(&self, _ctx: &ServiceContext, _args: serde_json::Value) -> Result<ActionResult> {
        let output = tokio::process::Command::new("nix-env")
            .args(["--list-generations", "-p", "/nix/var/nix/profiles/system"])
            .output()
            .await?;

        let list = String::from_utf8_lossy(&output.stdout).to_string();
        let mut data = HashMap::new();
        data.insert("list".to_string(), serde_json::json!(list));

        Ok(ActionResult::success_with_data(list, data))
    }
}
