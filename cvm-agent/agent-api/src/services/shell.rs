use futures::Stream;
use std::pin::Pin;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use super::health::proto::shell_service_server::ShellService;
use super::health::proto::{shell_output, ShellOutput, ShellRequest};

pub struct ShellServiceImpl;

#[tonic::async_trait]
impl ShellService for ShellServiceImpl {
    type ExecuteStream = Pin<Box<dyn Stream<Item = Result<ShellOutput, Status>> + Send>>;

    async fn execute(
        &self,
        request: Request<ShellRequest>,
    ) -> Result<Response<Self::ExecuteStream>, Status> {
        let req = request.into_inner();
        let command = req.command;
        let working_dir = if req.working_dir.is_empty() {
            std::env::current_dir()
                .map_err(|e| Status::internal(e.to_string()))?
        } else {
            std::path::PathBuf::from(&req.working_dir)
        };

        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            let result = execute_command(&command, &working_dir, tx.clone()).await;
            if let Err(e) = result {
                let _ = tx
                    .send(Ok(ShellOutput {
                        output: Some(shell_output::Output::Stderr(e.to_string())),
                        exit_code: None,
                    }))
                    .await;
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }
}

async fn execute_command(
    command: &str,
    working_dir: &std::path::Path,
    tx: mpsc::Sender<Result<ShellOutput, Status>>,
) -> anyhow::Result<()> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(working_dir)
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
                .send(Ok(ShellOutput {
                    output: Some(shell_output::Output::Stdout(line)),
                    exit_code: None,
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
                .send(Ok(ShellOutput {
                    output: Some(shell_output::Output::Stderr(line)),
                    exit_code: None,
                }))
                .await;
        }
    });

    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    let status = child.wait().await?;
    let _ = tx
        .send(Ok(ShellOutput {
            output: None,
            exit_code: Some(status.code().unwrap_or(-1)),
        }))
        .await;

    Ok(())
}
