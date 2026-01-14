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
            let _ = tx
                .send(Ok(RebuildOutput {
                    output: None,
                    exit_code: None,
                    phase: Some("evaluating".to_string()),
                }))
                .await;

            if let Err(e) = execute_streaming(&cmd, tx.clone()).await {
                let _ = tx
                    .send(Ok(RebuildOutput {
                        output: Some(rebuild_output::Output::Stderr(e.to_string())),
                        exit_code: Some(1),
                        phase: None,
                    }))
                    .await;
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
                let _ = tx
                    .send(Ok(RebuildOutput {
                        output: Some(rebuild_output::Output::Stderr(e.to_string())),
                        exit_code: Some(1),
                        phase: None,
                    }))
                    .await;
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

            let generations =
                parse_generations_text(&String::from_utf8_lossy(&output.stdout), limit);
            return Ok(Response::new(ListGenerationsResponse { generations }));
        }

        let generations =
            parse_generations_json(&String::from_utf8_lossy(&output.stdout), limit)
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

        let version_info: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| serde_json::json!({}));

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
            let _ = tx_stdout
                .send(Ok(RebuildOutput {
                    output: Some(rebuild_output::Output::Stdout(line)),
                    exit_code: None,
                    phase: None,
                }))
                .await;
        }
    });

    let tx_stderr = tx.clone();
    let stderr_handle = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = tx_stderr
                .send(Ok(RebuildOutput {
                    output: Some(rebuild_output::Output::Stderr(line)),
                    exit_code: None,
                    phase: None,
                }))
                .await;
        }
    });

    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    let status = child.wait().await?;
    let _ = tx
        .send(Ok(RebuildOutput {
            output: None,
            exit_code: Some(status.code().unwrap_or(-1)),
            phase: Some("complete".to_string()),
        }))
        .await;

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

fn parse_generations_json(
    output: &str,
    limit: i32,
) -> Result<Vec<GenerationInfo>, serde_json::Error> {
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
