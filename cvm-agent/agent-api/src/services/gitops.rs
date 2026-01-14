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
        let branch = String::from_utf8_lossy(&branch_output.stdout)
            .trim()
            .to_string();

        // Get status --porcelain
        let status_output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&repo_path)
            .output()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let (modified, staged, untracked) =
            parse_porcelain_status(&String::from_utf8_lossy(&status_output.stdout));

        // Check upstream
        let upstream_output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "@{upstream}"])
            .current_dir(&repo_path)
            .output()
            .await;
        let has_upstream = upstream_output
            .map(|o| o.status.success())
            .unwrap_or(false);

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
            commit_hash: String::from_utf8_lossy(&hash_output.stdout)
                .trim()
                .to_string(),
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
                    let _ = tx
                        .send(Ok(GitPushOutput {
                            output: Some(git_push_output::Output::Stderr(e.to_string())),
                            exit_code: Some(1),
                        }))
                        .await;
                    return;
                }
            };

            let stderr = child.stderr.take().unwrap();
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx
                    .send(Ok(GitPushOutput {
                        output: Some(git_push_output::Output::Stderr(line)),
                        exit_code: None,
                    }))
                    .await;
            }

            let status = child.wait().await.ok();
            let _ = tx
                .send(Ok(GitPushOutput {
                    output: None,
                    exit_code: Some(status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1)),
                }))
                .await;
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
                }
                .to_string(),
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
