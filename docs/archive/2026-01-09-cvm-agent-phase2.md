# CVM Agent Phase 2 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add NixOS operations (rebuild, rollback, generations) and Git operations (status, commit, push) with risk classification and diff preview UI.

**Architecture:** Extend the existing gRPC server with NixOpsService and GitOpsService. Add risk classification logic that analyzes file paths and content changes. Web UI gets a new DiffPreview component for reviewing changes before applying them.

**Tech Stack:** Rust (Tonic), Protocol Buffers, TypeScript, React, gRPC-web

---

## Prerequisites

Before starting, ensure you're in the worktree:
```bash
cd /Users/hashwarlock/Projects/wrlx-nixos-config/.worktrees/cvm-agent
```

---

## Task 1: Add NixOps Proto Definitions

**Files:**
- Modify: `cvm-agent/proto/agent.proto`

**Step 1: Add NixOpsService definitions to agent.proto**

Add after the existing ShellService definition:

```protobuf
// NixOS operations service
service NixOpsService {
  // Rebuild NixOS configuration
  rpc Rebuild(RebuildRequest) returns (stream RebuildOutput);

  // Rollback to previous generation
  rpc Rollback(RollbackRequest) returns (stream RebuildOutput);

  // List available generations
  rpc ListGenerations(ListGenerationsRequest) returns (ListGenerationsResponse);

  // Get current generation info
  rpc CurrentGeneration(CurrentGenerationRequest) returns (GenerationInfo);
}

message RebuildRequest {
  string flake_path = 1;        // Path to flake (default: current dir)
  string hostname = 2;          // Target hostname
  string action = 3;            // "switch", "boot", "test", "build"
}

message RebuildOutput {
  oneof output {
    string stdout = 1;
    string stderr = 2;
  }
  optional int32 exit_code = 3;
  optional string phase = 4;    // "evaluating", "building", "activating"
}

message RollbackRequest {
  optional int32 generation = 1;  // Specific generation, or previous if not set
}

message ListGenerationsRequest {
  int32 limit = 1;              // Max generations to return (default: 10)
}

message ListGenerationsResponse {
  repeated GenerationInfo generations = 1;
}

message CurrentGenerationRequest {}

message GenerationInfo {
  int32 number = 1;
  string date = 2;
  string nixos_version = 3;
  string kernel_version = 4;
  string configuration_revision = 5;
  bool current = 6;
}
```

**Step 2: Verify proto compiles**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds

**Step 3: Commit**

```bash
git add cvm-agent/proto/agent.proto
git commit -m "feat: add NixOpsService proto definitions

- Rebuild with flake support
- Rollback to specific or previous generation
- List and query generations"
```

---

## Task 2: Add GitOps Proto Definitions

**Files:**
- Modify: `cvm-agent/proto/agent.proto`

**Step 1: Add GitOpsService definitions to agent.proto**

Add after NixOpsService:

```protobuf
// Git operations service
service GitOpsService {
  // Get repository status
  rpc Status(GitStatusRequest) returns (GitStatusResponse);

  // Get diff of changes
  rpc Diff(GitDiffRequest) returns (GitDiffResponse);

  // Stage files
  rpc Add(GitAddRequest) returns (GitAddResponse);

  // Create commit
  rpc Commit(GitCommitRequest) returns (GitCommitResponse);

  // Push to remote
  rpc Push(GitPushRequest) returns (stream GitPushOutput);
}

message GitStatusRequest {
  string repo_path = 1;         // Repository path (default: /app)
}

message GitStatusResponse {
  string branch = 1;
  repeated FileStatus modified = 2;
  repeated FileStatus staged = 3;
  repeated FileStatus untracked = 4;
  bool has_upstream = 5;
  int32 ahead = 6;
  int32 behind = 7;
}

message FileStatus {
  string path = 1;
  string status = 2;            // "modified", "added", "deleted", "renamed"
}

message GitDiffRequest {
  string repo_path = 1;
  repeated string paths = 2;    // Specific paths, or all if empty
  bool staged = 3;              // Show staged changes
}

message GitDiffResponse {
  repeated FileDiff diffs = 1;
}

message FileDiff {
  string path = 1;
  string diff = 2;              // Unified diff format
  RiskLevel risk_level = 3;
}

enum RiskLevel {
  RISK_LOW = 0;
  RISK_MEDIUM = 1;
  RISK_HIGH = 2;
  RISK_CRITICAL = 3;
}

message GitAddRequest {
  string repo_path = 1;
  repeated string paths = 2;    // Files to stage
}

message GitAddResponse {
  bool success = 1;
  repeated string staged_paths = 2;
}

message GitCommitRequest {
  string repo_path = 1;
  string message = 2;
}

message GitCommitResponse {
  bool success = 1;
  string commit_hash = 2;
  string error = 3;
}

message GitPushRequest {
  string repo_path = 1;
  string remote = 2;            // Default: "origin"
  string branch = 3;            // Default: current branch
}

message GitPushOutput {
  oneof output {
    string stdout = 1;
    string stderr = 2;
  }
  optional int32 exit_code = 3;
}
```

**Step 2: Verify proto compiles**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds

**Step 3: Commit**

```bash
git add cvm-agent/proto/agent.proto
git commit -m "feat: add GitOpsService proto definitions

- Status with branch and file tracking
- Diff with risk classification
- Add, commit, push operations"
```

---

## Task 3: Implement Risk Classification Module

**Files:**
- Create: `cvm-agent/agent-api/src/risk/mod.rs`
- Create: `cvm-agent/agent-api/src/risk/classifier.rs`
- Modify: `cvm-agent/agent-api/src/main.rs`

**Step 1: Create risk module**

```bash
mkdir -p cvm-agent/agent-api/src/risk
```

Create `cvm-agent/agent-api/src/risk/mod.rs`:
```rust
pub mod classifier;

pub use classifier::{RiskClassifier, RiskLevel};
```

**Step 2: Implement risk classifier**

Create `cvm-agent/agent-api/src/risk/classifier.rs`:
```rust
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

pub struct RiskClassifier;

impl RiskClassifier {
    /// Classify risk based on file path
    pub fn classify_path(path: &str) -> RiskLevel {
        let path = Path::new(path);
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let path_str = path.to_str().unwrap_or("");

        // Critical: Boot, kernel, and core system files
        if path_str.contains("boot")
            || path_str.contains("hardware-configuration")
            || filename == "flake.lock"
            || path_str.contains("/etc/nixos")
        {
            return RiskLevel::Critical;
        }

        // High: System modules and services
        if path_str.contains("modules/") && !path_str.contains("home/")
            || path_str.contains("system.nix")
            || path_str.contains("networking")
            || path_str.contains("firewall")
        {
            return RiskLevel::High;
        }

        // Medium: User configs and home-manager
        if path_str.contains("home/")
            || path_str.contains("users/")
            || path_str.ends_with(".nix")
        {
            return RiskLevel::Medium;
        }

        // Low: Everything else
        RiskLevel::Low
    }

    /// Classify risk based on diff content
    pub fn classify_diff(diff: &str) -> RiskLevel {
        let mut risk = RiskLevel::Low;

        // Check for dangerous patterns
        let critical_patterns = [
            "boot.loader",
            "fileSystems",
            "swapDevices",
            "networking.firewall.enable = false",
            "security.sudo.wheelNeedsPassword = false",
            "PermitRootLogin yes",
        ];

        let high_patterns = [
            "services.openssh",
            "networking.firewall",
            "security.",
            "systemd.services",
            "users.users.root",
        ];

        let medium_patterns = [
            "environment.systemPackages",
            "programs.",
            "services.",
        ];

        for pattern in critical_patterns {
            if diff.contains(pattern) {
                return RiskLevel::Critical;
            }
        }

        for pattern in high_patterns {
            if diff.contains(pattern) {
                risk = RiskLevel::High;
            }
        }

        if risk == RiskLevel::Low {
            for pattern in medium_patterns {
                if diff.contains(pattern) {
                    risk = RiskLevel::Medium;
                }
            }
        }

        risk
    }

    /// Combine path and content risk (take higher)
    pub fn classify(path: &str, diff: &str) -> RiskLevel {
        let path_risk = Self::classify_path(path);
        let diff_risk = Self::classify_diff(diff);

        if path_risk as u8 > diff_risk as u8 {
            path_risk
        } else {
            diff_risk
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_critical_paths() {
        assert_eq!(RiskClassifier::classify_path("hosts/foo/hardware-configuration.nix"), RiskLevel::Critical);
        assert_eq!(RiskClassifier::classify_path("flake.lock"), RiskLevel::Critical);
    }

    #[test]
    fn test_high_paths() {
        assert_eq!(RiskClassifier::classify_path("modules/system.nix"), RiskLevel::High);
        assert_eq!(RiskClassifier::classify_path("modules/networking.nix"), RiskLevel::High);
    }

    #[test]
    fn test_medium_paths() {
        assert_eq!(RiskClassifier::classify_path("home/modules/git.nix"), RiskLevel::Medium);
        assert_eq!(RiskClassifier::classify_path("users/hashwarlock/home.nix"), RiskLevel::Medium);
    }

    #[test]
    fn test_low_paths() {
        assert_eq!(RiskClassifier::classify_path("README.md"), RiskLevel::Low);
        assert_eq!(RiskClassifier::classify_path("docs/notes.txt"), RiskLevel::Low);
    }

    #[test]
    fn test_critical_diff() {
        let diff = "+  boot.loader.grub.device = \"/dev/sda\";";
        assert_eq!(RiskClassifier::classify_diff(diff), RiskLevel::Critical);
    }
}
```

**Step 3: Add risk module to main.rs**

Add to top of `cvm-agent/agent-api/src/main.rs`:
```rust
mod risk;
```

**Step 4: Run tests**

Run:
```bash
cd cvm-agent && cargo test
```
Expected: All tests pass

**Step 5: Commit**

```bash
git add cvm-agent/agent-api/src/risk/
git add cvm-agent/agent-api/src/main.rs
git commit -m "feat: add risk classification module

- Classify by file path (boot, modules, home)
- Classify by diff content (dangerous patterns)
- Unit tests for classification"
```

---

## Task 4: Implement NixOpsService

**Files:**
- Create: `cvm-agent/agent-api/src/services/nixops.rs`
- Modify: `cvm-agent/agent-api/src/services/mod.rs`
- Modify: `cvm-agent/agent-api/src/main.rs`

**Step 1: Implement NixOpsService**

Create `cvm-agent/agent-api/src/services/nixops.rs`:
```rust
use futures::Stream;
use std::pin::Pin;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use super::health::proto::nix_ops_service_server::NixOpsService;
use super::health::proto::{
    rebuild_output, CurrentGenerationRequest, GenerationInfo, ListGenerationsRequest,
    ListGenerationsResponse, RebuildOutput, RebuildRequest, RollbackRequest,
};

pub struct NixOpsServiceImpl;

#[tonic::async_trait]
impl NixOpsService for NixOpsServiceImpl {
    type RebuildStream = Pin<Box<dyn Stream<Item = Result<RebuildOutput, Status>> + Send>>;
    type RollbackStream = Pin<Box<dyn Stream<Item = Result<RebuildOutput, Status>> + Send>>;

    async fn rebuild(
        &self,
        request: Request<RebuildRequest>,
    ) -> Result<Response<Self::RebuildStream>, Status> {
        let req = request.into_inner();
        let flake_path = if req.flake_path.is_empty() {
            "/app".to_string()
        } else {
            req.flake_path
        };
        let hostname = if req.hostname.is_empty() {
            "phala-cvm".to_string()
        } else {
            req.hostname
        };
        let action = if req.action.is_empty() {
            "switch".to_string()
        } else {
            req.action
        };

        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            let cmd = format!(
                "nixos-rebuild {} --flake {}#{}",
                action, flake_path, hostname
            );

            // Send phase update
            let _ = tx.send(Ok(RebuildOutput {
                output: None,
                exit_code: None,
                phase: Some("evaluating".to_string()),
            })).await;

            if let Err(e) = execute_streaming(&cmd, tx.clone()).await {
                let _ = tx.send(Ok(RebuildOutput {
                    output: Some(rebuild_output::Output::Stderr(e.to_string())),
                    exit_code: Some(1),
                    phase: None,
                })).await;
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }

    async fn rollback(
        &self,
        request: Request<RollbackRequest>,
    ) -> Result<Response<Self::RollbackStream>, Status> {
        let req = request.into_inner();
        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            let cmd = if let Some(gen) = req.generation {
                format!("nixos-rebuild switch --rollback --generation {}", gen)
            } else {
                "nixos-rebuild --rollback switch".to_string()
            };

            if let Err(e) = execute_streaming(&cmd, tx.clone()).await {
                let _ = tx.send(Ok(RebuildOutput {
                    output: Some(rebuild_output::Output::Stderr(e.to_string())),
                    exit_code: Some(1),
                    phase: None,
                })).await;
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }

    async fn list_generations(
        &self,
        request: Request<ListGenerationsRequest>,
    ) -> Result<Response<ListGenerationsResponse>, Status> {
        let req = request.into_inner();
        let limit = if req.limit == 0 { 10 } else { req.limit };

        let output = Command::new("nixos-rebuild")
            .args(["list-generations", "--json"])
            .output()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if !output.status.success() {
            // Fallback: parse nix-env output
            let output = Command::new("nix-env")
                .args(["--list-generations", "-p", "/nix/var/nix/profiles/system"])
                .output()
                .await
                .map_err(|e| Status::internal(e.to_string()))?;

            let generations = parse_generations_text(&String::from_utf8_lossy(&output.stdout), limit);
            return Ok(Response::new(ListGenerationsResponse { generations }));
        }

        let generations = parse_generations_json(&String::from_utf8_lossy(&output.stdout), limit)
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ListGenerationsResponse { generations }))
    }

    async fn current_generation(
        &self,
        _request: Request<CurrentGenerationRequest>,
    ) -> Result<Response<GenerationInfo>, Status> {
        let output = Command::new("nixos-version")
            .args(["--json"])
            .output()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let version_info: serde_json::Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|_| serde_json::json!({}));

        // Get current generation number
        let gen_output = Command::new("readlink")
            .args(["-f", "/nix/var/nix/profiles/system"])
            .output()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let gen_path = String::from_utf8_lossy(&gen_output.stdout);
        let gen_num = gen_path
            .trim()
            .rsplit('-')
            .next()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(0);

        Ok(Response::new(GenerationInfo {
            number: gen_num,
            date: chrono::Utc::now().to_rfc3339(),
            nixos_version: version_info["nixosVersion"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            kernel_version: version_info["kernelVersion"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            configuration_revision: version_info["configurationRevision"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            current: true,
        }))
    }
}

async fn execute_streaming(
    cmd: &str,
    tx: mpsc::Sender<Result<RebuildOutput, Status>>,
) -> anyhow::Result<()> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let tx_stdout = tx.clone();
    let stdout_handle = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = tx_stdout.send(Ok(RebuildOutput {
                output: Some(rebuild_output::Output::Stdout(line)),
                exit_code: None,
                phase: None,
            })).await;
        }
    });

    let tx_stderr = tx.clone();
    let stderr_handle = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = tx_stderr.send(Ok(RebuildOutput {
                output: Some(rebuild_output::Output::Stderr(line)),
                exit_code: None,
                phase: None,
            })).await;
        }
    });

    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    let status = child.wait().await?;
    let _ = tx.send(Ok(RebuildOutput {
        output: None,
        exit_code: Some(status.code().unwrap_or(-1)),
        phase: Some("complete".to_string()),
    })).await;

    Ok(())
}

fn parse_generations_text(output: &str, limit: i32) -> Vec<GenerationInfo> {
    output
        .lines()
        .filter_map(|line| {
            // Format: "  123   2024-01-01 12:00:00   (current)"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let number = parts[0].parse::<i32>().ok()?;
                let date = format!("{} {}", parts[1], parts.get(2).unwrap_or(&""));
                let current = line.contains("(current)");
                Some(GenerationInfo {
                    number,
                    date,
                    nixos_version: String::new(),
                    kernel_version: String::new(),
                    configuration_revision: String::new(),
                    current,
                })
            } else {
                None
            }
        })
        .take(limit as usize)
        .collect()
}

fn parse_generations_json(output: &str, limit: i32) -> Result<Vec<GenerationInfo>, serde_json::Error> {
    let data: Vec<serde_json::Value> = serde_json::from_str(output)?;
    Ok(data
        .into_iter()
        .take(limit as usize)
        .map(|v| GenerationInfo {
            number: v["generation"].as_i64().unwrap_or(0) as i32,
            date: v["date"].as_str().unwrap_or("").to_string(),
            nixos_version: v["nixosVersion"].as_str().unwrap_or("").to_string(),
            kernel_version: v["kernelVersion"].as_str().unwrap_or("").to_string(),
            configuration_revision: v["configurationRevision"].as_str().unwrap_or("").to_string(),
            current: v["current"].as_bool().unwrap_or(false),
        })
        .collect())
}
```

**Step 2: Add chrono dependency**

Add to `cvm-agent/Cargo.toml` workspace dependencies:
```toml
chrono = { version = "0.4", features = ["serde"] }
```

Add to `cvm-agent/agent-api/Cargo.toml`:
```toml
chrono.workspace = true
```

**Step 3: Update services/mod.rs**

Update `cvm-agent/agent-api/src/services/mod.rs`:
```rust
pub mod chat;
pub mod health;
pub mod nixops;
pub mod shell;

pub use chat::ChatServiceImpl;
pub use health::HealthServiceImpl;
pub use nixops::NixOpsServiceImpl;
pub use shell::ShellServiceImpl;
```

**Step 4: Register NixOpsService in main.rs**

Update imports in `cvm-agent/agent-api/src/main.rs`:
```rust
use services::health::proto::nix_ops_service_server::NixOpsServiceServer;
use services::{ChatServiceImpl, HealthServiceImpl, NixOpsServiceImpl, ShellServiceImpl};
```

Add service registration:
```rust
let nixops_service = NixOpsServiceImpl;

Server::builder()
    // ... existing config
    .add_service(NixOpsServiceServer::new(nixops_service))
    // ... rest
```

**Step 5: Verify build**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds

**Step 6: Commit**

```bash
git add cvm-agent/
git commit -m "feat: implement NixOpsService

- Rebuild with flake support and streaming output
- Rollback to specific or previous generation
- List generations with JSON and text parsing
- Current generation info"
```

---

## Task 5: Implement GitOpsService

**Files:**
- Create: `cvm-agent/agent-api/src/services/gitops.rs`
- Modify: `cvm-agent/agent-api/src/services/mod.rs`
- Modify: `cvm-agent/agent-api/src/main.rs`

**Step 1: Implement GitOpsService**

Create `cvm-agent/agent-api/src/services/gitops.rs`:
```rust
use futures::Stream;
use std::pin::Pin;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::risk::{RiskClassifier, RiskLevel as InternalRiskLevel};

use super::health::proto::git_ops_service_server::GitOpsService;
use super::health::proto::{
    git_push_output, FileDiff, FileStatus, GitAddRequest, GitAddResponse, GitCommitRequest,
    GitCommitResponse, GitDiffRequest, GitDiffResponse, GitPushOutput, GitPushRequest,
    GitStatusRequest, GitStatusResponse, RiskLevel,
};

pub struct GitOpsServiceImpl;

#[tonic::async_trait]
impl GitOpsService for GitOpsServiceImpl {
    type PushStream = Pin<Box<dyn Stream<Item = Result<GitPushOutput, Status>> + Send>>;

    async fn status(
        &self,
        request: Request<GitStatusRequest>,
    ) -> Result<Response<GitStatusResponse>, Status> {
        let req = request.into_inner();
        let repo_path = if req.repo_path.is_empty() {
            "/app".to_string()
        } else {
            req.repo_path
        };

        // Get branch name
        let branch_output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&repo_path)
            .output()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();

        // Get status --porcelain
        let status_output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&repo_path)
            .output()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let (modified, staged, untracked) = parse_porcelain_status(
            &String::from_utf8_lossy(&status_output.stdout)
        );

        // Check upstream
        let upstream_output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "@{upstream}"])
            .current_dir(&repo_path)
            .output()
            .await;
        let has_upstream = upstream_output.map(|o| o.status.success()).unwrap_or(false);

        // Get ahead/behind
        let (ahead, behind) = if has_upstream {
            let count_output = Command::new("git")
                .args(["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])
                .current_dir(&repo_path)
                .output()
                .await
                .ok();

            count_output
                .map(|o| {
                    let s = String::from_utf8_lossy(&o.stdout);
                    let parts: Vec<&str> = s.trim().split('\t').collect();
                    (
                        parts.first().and_then(|s| s.parse().ok()).unwrap_or(0),
                        parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0),
                    )
                })
                .unwrap_or((0, 0))
        } else {
            (0, 0)
        };

        Ok(Response::new(GitStatusResponse {
            branch,
            modified,
            staged,
            untracked,
            has_upstream,
            ahead,
            behind,
        }))
    }

    async fn diff(
        &self,
        request: Request<GitDiffRequest>,
    ) -> Result<Response<GitDiffResponse>, Status> {
        let req = request.into_inner();
        let repo_path = if req.repo_path.is_empty() {
            "/app".to_string()
        } else {
            req.repo_path
        };

        let mut args = vec!["diff"];
        if req.staged {
            args.push("--staged");
        }
        args.extend(req.paths.iter().map(|s| s.as_str()));

        let output = Command::new("git")
            .args(&args)
            .current_dir(&repo_path)
            .output()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let diff_text = String::from_utf8_lossy(&output.stdout);
        let diffs = parse_unified_diff(&diff_text);

        Ok(Response::new(GitDiffResponse { diffs }))
    }

    async fn add(
        &self,
        request: Request<GitAddRequest>,
    ) -> Result<Response<GitAddResponse>, Status> {
        let req = request.into_inner();
        let repo_path = if req.repo_path.is_empty() {
            "/app".to_string()
        } else {
            req.repo_path
        };

        let mut args = vec!["add"];
        args.extend(req.paths.iter().map(|s| s.as_str()));

        let output = Command::new("git")
            .args(&args)
            .current_dir(&repo_path)
            .output()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GitAddResponse {
            success: output.status.success(),
            staged_paths: req.paths,
        }))
    }

    async fn commit(
        &self,
        request: Request<GitCommitRequest>,
    ) -> Result<Response<GitCommitResponse>, Status> {
        let req = request.into_inner();
        let repo_path = if req.repo_path.is_empty() {
            "/app".to_string()
        } else {
            req.repo_path
        };

        let output = Command::new("git")
            .args(["commit", "-m", &req.message])
            .current_dir(&repo_path)
            .output()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if !output.status.success() {
            return Ok(Response::new(GitCommitResponse {
                success: false,
                commit_hash: String::new(),
                error: String::from_utf8_lossy(&output.stderr).to_string(),
            }));
        }

        // Get commit hash
        let hash_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GitCommitResponse {
            success: true,
            commit_hash: String::from_utf8_lossy(&hash_output.stdout).trim().to_string(),
            error: String::new(),
        }))
    }

    async fn push(
        &self,
        request: Request<GitPushRequest>,
    ) -> Result<Response<Self::PushStream>, Status> {
        let req = request.into_inner();
        let repo_path = if req.repo_path.is_empty() {
            "/app".to_string()
        } else {
            req.repo_path
        };
        let remote = if req.remote.is_empty() {
            "origin".to_string()
        } else {
            req.remote
        };

        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            let mut args = vec!["push", &remote];
            if !req.branch.is_empty() {
                args.push(&req.branch);
            }

            let mut child = match Command::new("git")
                .args(&args)
                .current_dir(&repo_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Ok(GitPushOutput {
                        output: Some(git_push_output::Output::Stderr(e.to_string())),
                        exit_code: Some(1),
                    })).await;
                    return;
                }
            };

            let stderr = child.stderr.take().unwrap();
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx.send(Ok(GitPushOutput {
                    output: Some(git_push_output::Output::Stderr(line)),
                    exit_code: None,
                })).await;
            }

            let status = child.wait().await.ok();
            let _ = tx.send(Ok(GitPushOutput {
                output: None,
                exit_code: Some(status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1)),
            })).await;
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }
}

fn parse_porcelain_status(output: &str) -> (Vec<FileStatus>, Vec<FileStatus>, Vec<FileStatus>) {
    let mut modified = Vec::new();
    let mut staged = Vec::new();
    let mut untracked = Vec::new();

    for line in output.lines() {
        if line.len() < 3 {
            continue;
        }
        let index_status = line.chars().next().unwrap_or(' ');
        let worktree_status = line.chars().nth(1).unwrap_or(' ');
        let path = line[3..].to_string();

        // Staged changes
        if index_status != ' ' && index_status != '?' {
            staged.push(FileStatus {
                path: path.clone(),
                status: match index_status {
                    'M' => "modified",
                    'A' => "added",
                    'D' => "deleted",
                    'R' => "renamed",
                    _ => "unknown",
                }.to_string(),
            });
        }

        // Worktree changes
        if worktree_status == 'M' {
            modified.push(FileStatus {
                path: path.clone(),
                status: "modified".to_string(),
            });
        } else if worktree_status == 'D' {
            modified.push(FileStatus {
                path: path.clone(),
                status: "deleted".to_string(),
            });
        }

        // Untracked
        if index_status == '?' {
            untracked.push(FileStatus {
                path,
                status: "untracked".to_string(),
            });
        }
    }

    (modified, staged, untracked)
}

fn parse_unified_diff(diff_text: &str) -> Vec<FileDiff> {
    let mut diffs = Vec::new();
    let mut current_path = String::new();
    let mut current_diff = String::new();

    for line in diff_text.lines() {
        if line.starts_with("diff --git") {
            // Save previous diff
            if !current_path.is_empty() {
                let risk = RiskClassifier::classify(&current_path, &current_diff);
                diffs.push(FileDiff {
                    path: current_path.clone(),
                    diff: current_diff.clone(),
                    risk_level: match risk {
                        InternalRiskLevel::Low => RiskLevel::RiskLow as i32,
                        InternalRiskLevel::Medium => RiskLevel::RiskMedium as i32,
                        InternalRiskLevel::High => RiskLevel::RiskHigh as i32,
                        InternalRiskLevel::Critical => RiskLevel::RiskCritical as i32,
                    },
                });
            }
            // Extract path from "diff --git a/path b/path"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                current_path = parts[2].trim_start_matches("a/").to_string();
            }
            current_diff = String::new();
        }
        current_diff.push_str(line);
        current_diff.push('\n');
    }

    // Don't forget the last diff
    if !current_path.is_empty() {
        let risk = RiskClassifier::classify(&current_path, &current_diff);
        diffs.push(FileDiff {
            path: current_path,
            diff: current_diff,
            risk_level: match risk {
                InternalRiskLevel::Low => RiskLevel::RiskLow as i32,
                InternalRiskLevel::Medium => RiskLevel::RiskMedium as i32,
                InternalRiskLevel::High => RiskLevel::RiskHigh as i32,
                InternalRiskLevel::Critical => RiskLevel::RiskCritical as i32,
            },
        });
    }

    diffs
}
```

**Step 2: Update services/mod.rs**

Update `cvm-agent/agent-api/src/services/mod.rs`:
```rust
pub mod chat;
pub mod gitops;
pub mod health;
pub mod nixops;
pub mod shell;

pub use chat::ChatServiceImpl;
pub use gitops::GitOpsServiceImpl;
pub use health::HealthServiceImpl;
pub use nixops::NixOpsServiceImpl;
pub use shell::ShellServiceImpl;
```

**Step 3: Register GitOpsService in main.rs**

Update imports and add service:
```rust
use services::health::proto::git_ops_service_server::GitOpsServiceServer;
use services::{ChatServiceImpl, GitOpsServiceImpl, HealthServiceImpl, NixOpsServiceImpl, ShellServiceImpl};

// In main():
let gitops_service = GitOpsServiceImpl;

Server::builder()
    // ... existing
    .add_service(GitOpsServiceServer::new(gitops_service))
```

**Step 4: Verify build**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds

**Step 5: Commit**

```bash
git add cvm-agent/
git commit -m "feat: implement GitOpsService

- Status with branch, modified, staged, untracked files
- Diff with risk classification per file
- Add, commit, push operations
- Streaming push output"
```

---

## Task 6: Regenerate TypeScript Client

**Files:**
- Modify: `cvm-agent/web-ui/src/gen/` (regenerated)

**Step 1: Regenerate proto client**

Run:
```bash
cd cvm-agent/web-ui && docker run --rm -v "$(pwd):/app" -v "$(pwd)/../proto:/proto" -w /app node:20-alpine sh -c "npm install && npx buf generate /proto"
```

**Step 2: Verify generation**

Run:
```bash
ls cvm-agent/web-ui/src/gen/
```
Expected: Updated `agent_pb.ts` with NixOps and GitOps types

**Step 3: Commit**

```bash
git add cvm-agent/web-ui/src/gen/
git commit -m "feat: regenerate TypeScript client for Phase 2 services

- NixOpsService client methods
- GitOpsService client methods
- Risk level enum types"
```

---

## Task 7: Implement DiffPreview Component

**Files:**
- Create: `cvm-agent/web-ui/src/components/DiffPreview.tsx`
- Create: `cvm-agent/web-ui/src/hooks/useGitOps.ts`

**Step 1: Create useGitOps hook**

Create `cvm-agent/web-ui/src/hooks/useGitOps.ts`:
```typescript
import { createConnectTransport } from "@connectrpc/connect-web";
import { createClient } from "@connectrpc/connect";
import { GitOpsService } from "../gen/agent_pb";
import { useState, useCallback } from "react";

const transport = createConnectTransport({
  baseUrl: "/api",
});

const gitClient = createClient(GitOpsService, transport);

export interface FileChange {
  path: string;
  status: string;
}

export interface FileDiff {
  path: string;
  diff: string;
  riskLevel: "low" | "medium" | "high" | "critical";
}

export interface GitStatus {
  branch: string;
  modified: FileChange[];
  staged: FileChange[];
  untracked: FileChange[];
  hasUpstream: boolean;
  ahead: number;
  behind: number;
}

export function useGitOps() {
  const [status, setStatus] = useState<GitStatus | null>(null);
  const [diffs, setDiffs] = useState<FileDiff[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchStatus = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await gitClient.status({});
      setStatus({
        branch: response.branch,
        modified: response.modified.map(f => ({ path: f.path, status: f.status })),
        staged: response.staged.map(f => ({ path: f.path, status: f.status })),
        untracked: response.untracked.map(f => ({ path: f.path, status: f.status })),
        hasUpstream: response.hasUpstream,
        ahead: response.ahead,
        behind: response.behind,
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  }, []);

  const fetchDiff = useCallback(async (staged: boolean = false) => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await gitClient.diff({ staged });
      setDiffs(response.diffs.map(d => ({
        path: d.path,
        diff: d.diff,
        riskLevel: ["low", "medium", "high", "critical"][d.riskLevel] as FileDiff["riskLevel"],
      })));
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  }, []);

  const stageFiles = useCallback(async (paths: string[]) => {
    setIsLoading(true);
    try {
      await gitClient.add({ paths });
      await fetchStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  }, [fetchStatus]);

  const commit = useCallback(async (message: string) => {
    setIsLoading(true);
    try {
      const response = await gitClient.commit({ message });
      if (!response.success) {
        setError(response.error);
        return null;
      }
      await fetchStatus();
      return response.commitHash;
    } catch (e) {
      setError(String(e));
      return null;
    } finally {
      setIsLoading(false);
    }
  }, [fetchStatus]);

  return {
    status,
    diffs,
    isLoading,
    error,
    fetchStatus,
    fetchDiff,
    stageFiles,
    commit,
  };
}
```

**Step 2: Create DiffPreview component**

Create `cvm-agent/web-ui/src/components/DiffPreview.tsx`:
```tsx
import { useEffect, useState } from "react";
import { useGitOps, FileDiff } from "../hooks/useGitOps";

interface DiffPreviewProps {
  onClose: () => void;
  onApprove: () => void;
}

const riskColors = {
  low: "bg-green-500/20 text-green-400 border-green-500/50",
  medium: "bg-yellow-500/20 text-yellow-400 border-yellow-500/50",
  high: "bg-orange-500/20 text-orange-400 border-orange-500/50",
  critical: "bg-red-500/20 text-red-400 border-red-500/50",
};

const riskLabels = {
  low: "Low Risk",
  medium: "Medium Risk",
  high: "High Risk",
  critical: "Critical - Review Carefully",
};

export function DiffPreview({ onClose, onApprove }: DiffPreviewProps) {
  const { status, diffs, isLoading, error, fetchStatus, fetchDiff, stageFiles, commit } = useGitOps();
  const [commitMessage, setCommitMessage] = useState("");
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  const [expandedDiff, setExpandedDiff] = useState<string | null>(null);

  useEffect(() => {
    fetchStatus();
    fetchDiff();
  }, [fetchStatus, fetchDiff]);

  const handleStage = async () => {
    if (selectedFiles.size > 0) {
      await stageFiles(Array.from(selectedFiles));
      setSelectedFiles(new Set());
      fetchDiff(true);
    }
  };

  const handleCommit = async () => {
    if (commitMessage.trim()) {
      const hash = await commit(commitMessage);
      if (hash) {
        setCommitMessage("");
        onApprove();
      }
    }
  };

  const toggleFile = (path: string) => {
    const newSelected = new Set(selectedFiles);
    if (newSelected.has(path)) {
      newSelected.delete(path);
    } else {
      newSelected.add(path);
    }
    setSelectedFiles(newSelected);
  };

  const maxRisk = diffs.reduce((max, d) => {
    const order = ["low", "medium", "high", "critical"];
    return order.indexOf(d.riskLevel) > order.indexOf(max) ? d.riskLevel : max;
  }, "low" as FileDiff["riskLevel"]);

  return (
    <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50">
      <div className="bg-gray-900 border border-gray-700 rounded-xl w-[800px] max-h-[80vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-gray-700">
          <div className="flex items-center gap-3">
            <h2 className="text-lg font-semibold text-white">Review Changes</h2>
            {status && (
              <span className="text-sm text-gray-400">
                {status.branch} • {diffs.length} file(s) changed
              </span>
            )}
          </div>
          <button onClick={onClose} className="text-gray-400 hover:text-white">
            ✕
          </button>
        </div>

        {/* Risk Banner */}
        {maxRisk !== "low" && (
          <div className={`px-4 py-2 border-b ${riskColors[maxRisk]}`}>
            <span className="font-medium">{riskLabels[maxRisk]}</span>
            {maxRisk === "critical" && (
              <span className="ml-2 text-sm">
                These changes affect core system configuration
              </span>
            )}
          </div>
        )}

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4">
          {isLoading && <p className="text-gray-400">Loading...</p>}
          {error && <p className="text-red-400">{error}</p>}

          {/* File List */}
          {diffs.map((diff) => (
            <div key={diff.path} className="mb-4">
              <div
                className="flex items-center justify-between p-2 bg-gray-800 rounded-t cursor-pointer"
                onClick={() => setExpandedDiff(expandedDiff === diff.path ? null : diff.path)}
              >
                <div className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={selectedFiles.has(diff.path)}
                    onChange={() => toggleFile(diff.path)}
                    onClick={(e) => e.stopPropagation()}
                    className="rounded"
                  />
                  <span className="text-gray-200 font-mono text-sm">{diff.path}</span>
                </div>
                <span className={`px-2 py-0.5 rounded text-xs border ${riskColors[diff.riskLevel]}`}>
                  {diff.riskLevel}
                </span>
              </div>

              {expandedDiff === diff.path && (
                <pre className="p-3 bg-gray-950 rounded-b text-xs font-mono overflow-x-auto max-h-64 overflow-y-auto">
                  {diff.diff.split('\n').map((line, i) => (
                    <div
                      key={i}
                      className={
                        line.startsWith('+') ? 'text-green-400' :
                        line.startsWith('-') ? 'text-red-400' :
                        line.startsWith('@') ? 'text-blue-400' :
                        'text-gray-400'
                      }
                    >
                      {line}
                    </div>
                  ))}
                </pre>
              )}
            </div>
          ))}
        </div>

        {/* Footer */}
        <div className="border-t border-gray-700 p-4">
          <div className="flex gap-2 mb-3">
            <input
              type="text"
              value={commitMessage}
              onChange={(e) => setCommitMessage(e.target.value)}
              placeholder="Commit message..."
              className="flex-1 bg-gray-800 border border-gray-600 rounded px-3 py-2 text-white text-sm"
            />
          </div>
          <div className="flex justify-between">
            <button
              onClick={handleStage}
              disabled={selectedFiles.size === 0}
              className="px-4 py-2 bg-gray-700 hover:bg-gray-600 disabled:bg-gray-800 disabled:text-gray-500 rounded text-sm"
            >
              Stage Selected ({selectedFiles.size})
            </button>
            <div className="flex gap-2">
              <button
                onClick={onClose}
                className="px-4 py-2 bg-gray-700 hover:bg-gray-600 rounded text-sm"
              >
                Cancel
              </button>
              <button
                onClick={handleCommit}
                disabled={!commitMessage.trim() || (status?.staged.length === 0)}
                className={`px-4 py-2 rounded text-sm font-medium ${
                  maxRisk === "critical"
                    ? "bg-red-600 hover:bg-red-700"
                    : "bg-blue-600 hover:bg-blue-700"
                } disabled:bg-gray-800 disabled:text-gray-500`}
              >
                {maxRisk === "critical" ? "Commit (Critical)" : "Commit"}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
```

**Step 3: Verify build**

Run:
```bash
cd cvm-agent/web-ui && docker run --rm -v "$(pwd):/app" -w /app node:20-alpine npm run build
```
Expected: Build succeeds

**Step 4: Commit**

```bash
git add cvm-agent/web-ui/src/
git commit -m "feat: add DiffPreview component with risk visualization

- useGitOps hook for git operations
- DiffPreview modal with file selection
- Risk level badges and color coding
- Expandable unified diff view
- Stage and commit workflow"
```

---

## Task 8: Implement NixOps UI Components

**Files:**
- Create: `cvm-agent/web-ui/src/hooks/useNixOps.ts`
- Create: `cvm-agent/web-ui/src/components/GenerationsList.tsx`

**Step 1: Create useNixOps hook**

Create `cvm-agent/web-ui/src/hooks/useNixOps.ts`:
```typescript
import { createConnectTransport } from "@connectrpc/connect-web";
import { createClient } from "@connectrpc/connect";
import { NixOpsService } from "../gen/agent_pb";
import { useState, useCallback } from "react";

const transport = createConnectTransport({
  baseUrl: "/api",
});

const nixClient = createClient(NixOpsService, transport);

export interface Generation {
  number: number;
  date: string;
  nixosVersion: string;
  kernelVersion: string;
  configurationRevision: string;
  current: boolean;
}

export function useNixOps() {
  const [generations, setGenerations] = useState<Generation[]>([]);
  const [currentGen, setCurrentGen] = useState<Generation | null>(null);
  const [output, setOutput] = useState<string[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchGenerations = useCallback(async (limit: number = 10) => {
    try {
      const response = await nixClient.listGenerations({ limit });
      setGenerations(response.generations.map(g => ({
        number: g.number,
        date: g.date,
        nixosVersion: g.nixosVersion,
        kernelVersion: g.kernelVersion,
        configurationRevision: g.configurationRevision,
        current: g.current,
      })));
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const fetchCurrentGeneration = useCallback(async () => {
    try {
      const response = await nixClient.currentGeneration({});
      setCurrentGen({
        number: response.number,
        date: response.date,
        nixosVersion: response.nixosVersion,
        kernelVersion: response.kernelVersion,
        configurationRevision: response.configurationRevision,
        current: true,
      });
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const rebuild = useCallback(async (action: "switch" | "boot" | "test" | "build" = "switch") => {
    setIsRunning(true);
    setOutput([]);
    setError(null);

    try {
      for await (const chunk of nixClient.rebuild({ action })) {
        if (chunk.output.case === "stdout") {
          setOutput(prev => [...prev, chunk.output.value]);
        } else if (chunk.output.case === "stderr") {
          setOutput(prev => [...prev, `[stderr] ${chunk.output.value}`]);
        }
        if (chunk.phase) {
          setOutput(prev => [...prev, `--- Phase: ${chunk.phase} ---`]);
        }
        if (chunk.exitCode !== undefined && chunk.exitCode !== null) {
          if (chunk.exitCode !== 0) {
            setError(`Rebuild failed with exit code ${chunk.exitCode}`);
          }
        }
      }
      await fetchCurrentGeneration();
      await fetchGenerations();
    } catch (e) {
      setError(String(e));
    } finally {
      setIsRunning(false);
    }
  }, [fetchCurrentGeneration, fetchGenerations]);

  const rollback = useCallback(async (generation?: number) => {
    setIsRunning(true);
    setOutput([]);
    setError(null);

    try {
      for await (const chunk of nixClient.rollback({ generation })) {
        if (chunk.output.case === "stdout") {
          setOutput(prev => [...prev, chunk.output.value]);
        } else if (chunk.output.case === "stderr") {
          setOutput(prev => [...prev, `[stderr] ${chunk.output.value}`]);
        }
        if (chunk.exitCode !== undefined && chunk.exitCode !== null) {
          if (chunk.exitCode !== 0) {
            setError(`Rollback failed with exit code ${chunk.exitCode}`);
          }
        }
      }
      await fetchCurrentGeneration();
      await fetchGenerations();
    } catch (e) {
      setError(String(e));
    } finally {
      setIsRunning(false);
    }
  }, [fetchCurrentGeneration, fetchGenerations]);

  return {
    generations,
    currentGen,
    output,
    isRunning,
    error,
    fetchGenerations,
    fetchCurrentGeneration,
    rebuild,
    rollback,
  };
}
```

**Step 2: Create GenerationsList component**

Create `cvm-agent/web-ui/src/components/GenerationsList.tsx`:
```tsx
import { useEffect } from "react";
import { useNixOps } from "../hooks/useNixOps";

interface GenerationsListProps {
  onClose: () => void;
}

export function GenerationsList({ onClose }: GenerationsListProps) {
  const {
    generations,
    currentGen,
    output,
    isRunning,
    error,
    fetchGenerations,
    fetchCurrentGeneration,
    rebuild,
    rollback,
  } = useNixOps();

  useEffect(() => {
    fetchGenerations();
    fetchCurrentGeneration();
  }, [fetchGenerations, fetchCurrentGeneration]);

  return (
    <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50">
      <div className="bg-gray-900 border border-gray-700 rounded-xl w-[700px] max-h-[80vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-gray-700">
          <h2 className="text-lg font-semibold text-white">NixOS Generations</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white">
            ✕
          </button>
        </div>

        {/* Current Generation */}
        {currentGen && (
          <div className="px-4 py-3 bg-blue-500/10 border-b border-gray-700">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-blue-400 font-medium">Current: Generation {currentGen.number}</span>
                <span className="text-gray-400 text-sm ml-3">{currentGen.nixosVersion}</span>
              </div>
              <div className="flex gap-2">
                <button
                  onClick={() => rebuild("test")}
                  disabled={isRunning}
                  className="px-3 py-1 bg-gray-700 hover:bg-gray-600 disabled:bg-gray-800 rounded text-sm"
                >
                  Test
                </button>
                <button
                  onClick={() => rebuild("switch")}
                  disabled={isRunning}
                  className="px-3 py-1 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-800 rounded text-sm"
                >
                  Rebuild
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Generations List */}
        <div className="flex-1 overflow-y-auto p-4">
          {error && (
            <div className="mb-4 p-3 bg-red-500/20 border border-red-500/50 rounded text-red-400 text-sm">
              {error}
            </div>
          )}

          <div className="space-y-2">
            {generations.map((gen) => (
              <div
                key={gen.number}
                className={`flex items-center justify-between p-3 rounded ${
                  gen.current ? "bg-blue-500/20 border border-blue-500/30" : "bg-gray-800"
                }`}
              >
                <div>
                  <div className="flex items-center gap-2">
                    <span className="text-white font-medium">Generation {gen.number}</span>
                    {gen.current && (
                      <span className="px-2 py-0.5 bg-blue-500/30 rounded text-xs text-blue-400">
                        current
                      </span>
                    )}
                  </div>
                  <div className="text-gray-400 text-sm mt-1">
                    {gen.date} • {gen.nixosVersion || "unknown version"}
                  </div>
                </div>
                {!gen.current && (
                  <button
                    onClick={() => rollback(gen.number)}
                    disabled={isRunning}
                    className="px-3 py-1 bg-orange-600 hover:bg-orange-700 disabled:bg-gray-800 rounded text-sm"
                  >
                    Rollback
                  </button>
                )}
              </div>
            ))}
          </div>
        </div>

        {/* Output Console */}
        {output.length > 0 && (
          <div className="border-t border-gray-700">
            <div className="px-4 py-2 bg-gray-800 text-sm text-gray-400 flex items-center justify-between">
              <span>Output</span>
              {isRunning && <span className="animate-pulse text-blue-400">Running...</span>}
            </div>
            <pre className="p-4 bg-gray-950 text-xs font-mono max-h-48 overflow-y-auto text-gray-300">
              {output.join('\n')}
            </pre>
          </div>
        )}
      </div>
    </div>
  );
}
```

**Step 3: Verify build**

Run:
```bash
cd cvm-agent/web-ui && docker run --rm -v "$(pwd):/app" -w /app node:20-alpine npm run build
```
Expected: Build succeeds

**Step 4: Commit**

```bash
git add cvm-agent/web-ui/src/
git commit -m "feat: add NixOps UI components

- useNixOps hook for NixOS operations
- GenerationsList modal with current/history view
- Rebuild and rollback actions
- Streaming output console"
```

---

## Task 9: Integrate New Components into App

**Files:**
- Modify: `cvm-agent/web-ui/src/App.tsx`
- Modify: `cvm-agent/web-ui/src/components/ChatOverlay.tsx`

**Step 1: Update App.tsx with new modals**

Update `cvm-agent/web-ui/src/App.tsx`:
```tsx
import { useState, useEffect } from "react";
import { ChatOverlay } from "./components/ChatOverlay";
import { DiffPreview } from "./components/DiffPreview";
import { GenerationsList } from "./components/GenerationsList";

function App() {
  const [isOpen, setIsOpen] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);
  const [showDiff, setShowDiff] = useState(false);
  const [showGenerations, setShowGenerations] = useState(false);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // cmd+shift+space (Mac) or ctrl+shift+space (Windows/Linux)
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.code === "Space") {
        e.preventDefault();
        setIsOpen((prev) => !prev);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  return (
    <div className="min-h-screen bg-gray-950">
      {/* Placeholder for noVNC */}
      <div className="flex items-center justify-center h-screen text-gray-600">
        <div className="text-center">
          <p className="text-sm">
            Press <kbd className="px-2 py-1 bg-gray-800 rounded">Cmd</kbd> +{" "}
            <kbd className="px-2 py-1 bg-gray-800 rounded">Shift</kbd> +{" "}
            <kbd className="px-2 py-1 bg-gray-800 rounded">Space</kbd> to open agent
          </p>
          <p className="text-xs mt-2 text-gray-700">Or click the button below</p>
          <button
            onClick={() => setIsOpen(true)}
            className="mt-4 px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-white"
          >
            Open Agent
          </button>
        </div>
      </div>

      {/* Floating action button */}
      {!isOpen && (
        <button
          onClick={() => setIsOpen(true)}
          className="fixed bottom-4 right-4 w-12 h-12 bg-blue-600 hover:bg-blue-700 rounded-full shadow-lg flex items-center justify-center text-white text-xl"
          title="Open CVM Agent"
        >
          ?
        </button>
      )}

      <ChatOverlay
        isOpen={isOpen}
        onClose={() => {
          setIsOpen(false);
          setIsExpanded(false);
        }}
        isExpanded={isExpanded}
        onToggleExpand={() => setIsExpanded(!isExpanded)}
        onShowDiff={() => setShowDiff(true)}
        onShowGenerations={() => setShowGenerations(true)}
      />

      {showDiff && (
        <DiffPreview
          onClose={() => setShowDiff(false)}
          onApprove={() => setShowDiff(false)}
        />
      )}

      {showGenerations && (
        <GenerationsList onClose={() => setShowGenerations(false)} />
      )}
    </div>
  );
}

export default App;
```

**Step 2: Update ChatOverlay with action buttons**

Update `cvm-agent/web-ui/src/components/ChatOverlay.tsx` to accept and use the new props:

Add to interface:
```typescript
interface ChatOverlayProps {
  isOpen: boolean;
  onClose: () => void;
  isExpanded: boolean;
  onToggleExpand: () => void;
  onShowDiff?: () => void;
  onShowGenerations?: () => void;
}
```

Add action buttons in the header (after the expand/close buttons):
```tsx
{/* Header */}
<div className="flex items-center justify-between px-4 py-2 border-b border-gray-700">
  <span className="text-sm text-gray-400">CVM Agent</span>
  <div className="flex gap-2">
    {onShowDiff && (
      <button
        onClick={onShowDiff}
        className="text-gray-400 hover:text-white text-sm px-2"
        title="Review Changes"
      >
        Diff
      </button>
    )}
    {onShowGenerations && (
      <button
        onClick={onShowGenerations}
        className="text-gray-400 hover:text-white text-sm px-2"
        title="NixOS Generations"
      >
        Gen
      </button>
    )}
    <button
      onClick={onToggleExpand}
      className="text-gray-400 hover:text-white text-sm"
    >
      {isExpanded ? "⊟" : "⊞"}
    </button>
    <button
      onClick={onClose}
      className="text-gray-400 hover:text-white"
    >
      ✕
    </button>
  </div>
</div>
```

**Step 3: Verify build**

Run:
```bash
cd cvm-agent/web-ui && docker run --rm -v "$(pwd):/app" -w /app node:20-alpine npm run build
```
Expected: Build succeeds

**Step 4: Commit**

```bash
git add cvm-agent/web-ui/src/
git commit -m "feat: integrate DiffPreview and GenerationsList into App

- Add Diff and Gen buttons to ChatOverlay header
- Modal state management in App
- Connect all Phase 2 UI components"
```

---

## Task 10: Integration Test

**Step 1: Build everything**

Run:
```bash
cd cvm-agent && cargo build --release
cd cvm-agent/web-ui && docker run --rm -v "$(pwd):/app" -w /app node:20-alpine npm run build
```
Expected: Both builds succeed

**Step 2: Run Rust tests**

Run:
```bash
cd cvm-agent && cargo test
```
Expected: All tests pass (including risk classifier tests)

**Step 3: Run TypeScript type check**

Run:
```bash
cd cvm-agent/web-ui && docker run --rm -v "$(pwd):/app" -w /app node:20-alpine npx tsc --noEmit
```
Expected: No type errors

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: complete Phase 2 - NixOS and Git Operations

Phase 2 complete:
- NixOpsService: rebuild, rollback, list generations
- GitOpsService: status, diff, add, commit, push
- Risk classification for config changes
- DiffPreview UI with risk visualization
- GenerationsList UI with rebuild/rollback

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Summary

After completing all tasks, you will have:

1. **NixOpsService** (`cvm-agent/agent-api/src/services/nixops.rs`)
   - Rebuild with flake support
   - Rollback to any generation
   - List and query generations

2. **GitOpsService** (`cvm-agent/agent-api/src/services/gitops.rs`)
   - Repository status
   - Diff with risk classification
   - Stage, commit, push operations

3. **Risk Classification** (`cvm-agent/agent-api/src/risk/`)
   - Path-based classification (boot, modules, home)
   - Content-based classification (dangerous patterns)
   - Combined risk levels

4. **Web UI Components**
   - `DiffPreview` - Review changes with risk badges
   - `GenerationsList` - Manage NixOS generations
   - Integration into main app

## Next Phase

Phase 3 will add:
- AT-SPI integration for GUI element discovery
- xdotool wrapper for click/type actions
- Screenshot capture
- Vision fallback via Redpill
