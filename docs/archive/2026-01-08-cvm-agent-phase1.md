# CVM Agent Phase 1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the core infrastructure for the CVM Agent - Rust API server with basic chat and shell services, plus a minimal web UI overlay.

**Architecture:** Rust gRPC server (Tonic) exposes ChatService and ShellService. Web UI (React/TypeScript) connects via gRPC-web. Services communicate with Redpill API for LLM reasoning. Everything integrates with existing phala-cvm NixOS configuration.

**Tech Stack:** Rust (Tonic, Axum), Protocol Buffers, TypeScript, React, Vite, gRPC-web, Tailwind CSS

---

## Prerequisites

Before starting, ensure you're in the worktree:
```bash
cd /Users/hashwarlock/Projects/wrlx-nixos-config/.worktrees/cvm-agent
```

---

## Task 1: Initialize Rust Workspace

**Files:**
- Create: `cvm-agent/Cargo.toml`
- Create: `cvm-agent/agent-api/Cargo.toml`
- Create: `cvm-agent/agent-api/src/main.rs`
- Create: `cvm-agent/agent-api/build.rs`

**Step 1: Create workspace directory structure**

```bash
mkdir -p cvm-agent/agent-api/src
mkdir -p cvm-agent/proto
```

**Step 2: Create workspace Cargo.toml**

Create `cvm-agent/Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = [
    "agent-api",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"

[workspace.dependencies]
tokio = { version = "1.35", features = ["full"] }
tonic = "0.12"
tonic-web = "0.12"
prost = "0.13"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
reqwest = { version = "0.12", features = ["json"] }
```

**Step 3: Create agent-api Cargo.toml**

Create `cvm-agent/agent-api/Cargo.toml`:
```toml
[package]
name = "agent-api"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
tokio.workspace = true
tonic.workspace = true
tonic-web.workspace = true
prost.workspace = true
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
thiserror.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
reqwest.workspace = true
tower-http = { version = "0.5", features = ["cors"] }
futures = "0.3"

[build-dependencies]
tonic-build = "0.12"
```

**Step 4: Create build.rs for proto compilation**

Create `cvm-agent/agent-api/build.rs`:
```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(
            &["../proto/agent.proto"],
            &["../proto"],
        )?;
    Ok(())
}
```

**Step 5: Create minimal main.rs**

Create `cvm-agent/agent-api/src/main.rs`:
```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("CVM Agent API starting...");

    Ok(())
}
```

**Step 6: Verify workspace builds**

Run:
```bash
cd cvm-agent && cargo check
```
Expected: Compilation succeeds (warnings OK)

**Step 7: Commit**

```bash
git add cvm-agent/
git commit -m "feat: initialize Rust workspace for CVM Agent

- Workspace with agent-api crate
- Tonic + gRPC dependencies
- Build script for proto compilation"
```

---

## Task 2: Define Protocol Buffers

**Files:**
- Create: `cvm-agent/proto/agent.proto`
- Create: `cvm-agent/proto/buf.yaml`

**Step 1: Create proto directory structure**

Already created in Task 1.

**Step 2: Create agent.proto with core services**

Create `cvm-agent/proto/agent.proto`:
```protobuf
syntax = "proto3";
package cvm.agent;

// Health check
service HealthService {
  rpc Check(HealthRequest) returns (HealthResponse);
}

message HealthRequest {}

message HealthResponse {
  bool healthy = 1;
  string version = 2;
}

// Chat service - main interaction point
service ChatService {
  rpc SendMessage(ChatRequest) returns (stream ChatResponse);
}

message ChatRequest {
  string message = 1;
  string conversation_id = 2;
}

message ChatResponse {
  oneof response {
    string text = 1;
    string error = 2;
    bool done = 3;
  }
}

// Shell execution service
service ShellService {
  rpc Execute(ShellRequest) returns (stream ShellOutput);
}

message ShellRequest {
  string command = 1;
  string working_dir = 2;
  int32 timeout_seconds = 3;
}

message ShellOutput {
  oneof output {
    string stdout = 1;
    string stderr = 2;
    int32 exit_code = 3;
  }
}
```

**Step 3: Create buf.yaml for linting**

Create `cvm-agent/proto/buf.yaml`:
```yaml
version: v1
name: buf.build/cvm/agent
lint:
  use:
    - DEFAULT
breaking:
  use:
    - FILE
```

**Step 4: Verify proto compiles**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds, generates proto code in target/

**Step 5: Commit**

```bash
git add cvm-agent/proto/
git commit -m "feat: add Protocol Buffer definitions

- HealthService for liveness checks
- ChatService with streaming responses
- ShellService with streaming output"
```

---

## Task 3: Implement Health Service

**Files:**
- Create: `cvm-agent/agent-api/src/services/mod.rs`
- Create: `cvm-agent/agent-api/src/services/health.rs`
- Modify: `cvm-agent/agent-api/src/main.rs`

**Step 1: Create services module**

```bash
mkdir -p cvm-agent/agent-api/src/services
```

Create `cvm-agent/agent-api/src/services/mod.rs`:
```rust
pub mod health;

pub use health::HealthServiceImpl;
```

**Step 2: Implement HealthService**

Create `cvm-agent/agent-api/src/services/health.rs`:
```rust
use tonic::{Request, Response, Status};

pub mod proto {
    tonic::include_proto!("cvm.agent");
}

use proto::health_service_server::HealthService;
use proto::{HealthRequest, HealthResponse};

pub struct HealthServiceImpl;

#[tonic::async_trait]
impl HealthService for HealthServiceImpl {
    async fn check(
        &self,
        _request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        let response = HealthResponse {
            healthy: true,
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        Ok(Response::new(response))
    }
}
```

**Step 3: Update main.rs to serve HealthService**

Replace `cvm-agent/agent-api/src/main.rs`:
```rust
mod services;

use std::net::SocketAddr;
use tonic::transport::Server;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use services::health::proto::health_service_server::HealthServiceServer;
use services::HealthServiceImpl;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let addr: SocketAddr = "0.0.0.0:8080".parse()?;
    tracing::info!("CVM Agent API listening on {}", addr);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods(Any);

    let health_service = HealthServiceImpl;

    Server::builder()
        .accept_http1(true)
        .layer(cors)
        .layer(tonic_web::GrpcWebLayer::new())
        .add_service(HealthServiceServer::new(health_service))
        .serve(addr)
        .await?;

    Ok(())
}
```

**Step 4: Verify it compiles and runs**

Run:
```bash
cd cvm-agent && cargo run &
sleep 2
curl -v http://localhost:8080/cvm.agent.HealthService/Check \
  -H "Content-Type: application/grpc-web+proto" \
  -d '' || echo "Server running (gRPC endpoint)"
pkill agent-api
```
Expected: Server starts, responds to health check

**Step 5: Commit**

```bash
git add cvm-agent/agent-api/src/
git commit -m "feat: implement HealthService

- gRPC health endpoint
- CORS enabled for browser access
- gRPC-web layer for browser clients"
```

---

## Task 4: Implement Shell Service

**Files:**
- Create: `cvm-agent/agent-api/src/services/shell.rs`
- Modify: `cvm-agent/agent-api/src/services/mod.rs`
- Modify: `cvm-agent/agent-api/src/main.rs`

**Step 1: Add shell service module**

Update `cvm-agent/agent-api/src/services/mod.rs`:
```rust
pub mod health;
pub mod shell;

pub use health::HealthServiceImpl;
pub use shell::ShellServiceImpl;
```

**Step 2: Implement ShellService with streaming**

Create `cvm-agent/agent-api/src/services/shell.rs`:
```rust
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
                }))
                .await;
        }
    });

    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    let status = child.wait().await?;
    let _ = tx
        .send(Ok(ShellOutput {
            output: Some(shell_output::Output::ExitCode(status.code().unwrap_or(-1))),
        }))
        .await;

    Ok(())
}
```

**Step 3: Add tokio-stream dependency**

Update `cvm-agent/agent-api/Cargo.toml` dependencies section:
```toml
[dependencies]
tokio.workspace = true
tonic.workspace = true
tonic-web.workspace = true
prost.workspace = true
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
thiserror.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
reqwest.workspace = true
tower-http = { version = "0.5", features = ["cors"] }
futures = "0.3"
tokio-stream = "0.1"
```

**Step 4: Register ShellService in main.rs**

Update `cvm-agent/agent-api/src/main.rs`:
```rust
mod services;

use std::net::SocketAddr;
use tonic::transport::Server;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use services::health::proto::health_service_server::HealthServiceServer;
use services::health::proto::shell_service_server::ShellServiceServer;
use services::{HealthServiceImpl, ShellServiceImpl};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let addr: SocketAddr = "0.0.0.0:8080".parse()?;
    tracing::info!("CVM Agent API listening on {}", addr);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods(Any);

    let health_service = HealthServiceImpl;
    let shell_service = ShellServiceImpl;

    Server::builder()
        .accept_http1(true)
        .layer(cors)
        .layer(tonic_web::GrpcWebLayer::new())
        .add_service(HealthServiceServer::new(health_service))
        .add_service(ShellServiceServer::new(shell_service))
        .serve(addr)
        .await?;

    Ok(())
}
```

**Step 5: Verify compilation**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds

**Step 6: Commit**

```bash
git add cvm-agent/
git commit -m "feat: implement ShellService with streaming

- Execute shell commands via gRPC
- Stream stdout/stderr in real-time
- Return exit code on completion"
```

---

## Task 5: Implement Redpill LLM Client

**Files:**
- Create: `cvm-agent/agent-api/src/llm/mod.rs`
- Create: `cvm-agent/agent-api/src/llm/redpill.rs`
- Modify: `cvm-agent/agent-api/src/main.rs`

**Step 1: Create LLM module structure**

```bash
mkdir -p cvm-agent/agent-api/src/llm
```

Create `cvm-agent/agent-api/src/llm/mod.rs`:
```rust
pub mod redpill;

pub use redpill::RedpillClient;
```

**Step 2: Implement Redpill client (Anthropic-compatible)**

Create `cvm-agent/agent-api/src/llm/redpill.rs`:
```rust
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct RedpillClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
    stream: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<Delta>,
}

#[derive(Deserialize)]
struct Delta {
    #[serde(rename = "type")]
    delta_type: Option<String>,
    text: Option<String>,
}

impl RedpillClient {
    pub fn new() -> Result<Self> {
        let api_key = std::env::var("REDPILL_API_KEY")
            .or_else(|_| std::env::var("ANTHROPIC_AUTH_TOKEN"))
            .map_err(|_| anyhow::anyhow!("REDPILL_API_KEY or ANTHROPIC_AUTH_TOKEN required"))?;

        let base_url = std::env::var("REDPILL_BASE_URL")
            .or_else(|_| std::env::var("ANTHROPIC_BASE_URL"))
            .unwrap_or_else(|_| "https://api.redpill.ai".to_string());

        let model = std::env::var("REDPILL_MODEL")
            .or_else(|_| std::env::var("ANTHROPIC_MODEL"))
            .unwrap_or_else(|_| "claude-3-5-sonnet-20241022".to_string());

        Ok(Self {
            client: Client::new(),
            api_key,
            base_url,
            model,
        })
    }

    pub async fn chat_stream(
        &self,
        messages: Vec<Message>,
        tx: mpsc::Sender<Result<String>>,
    ) -> Result<()> {
        let request = ChatRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            messages,
            stream: true,
        };

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            tx.send(Err(anyhow::anyhow!("API error: {}", error_text))).await.ok();
            return Ok(());
        }

        let mut stream = response.bytes_stream();
        use futures::StreamExt;

        let mut buffer = String::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete SSE events
            while let Some(pos) = buffer.find("\n\n") {
                let event_str = buffer[..pos].to_string();
                buffer = buffer[pos + 2..].to_string();

                if let Some(data) = event_str.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        continue;
                    }
                    if let Ok(event) = serde_json::from_str::<StreamEvent>(data) {
                        if event.event_type == "content_block_delta" {
                            if let Some(delta) = event.delta {
                                if let Some(text) = delta.text {
                                    tx.send(Ok(text)).await.ok();
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
```

**Step 3: Add futures-util for stream processing**

Update `cvm-agent/agent-api/Cargo.toml`:
```toml
[dependencies]
tokio.workspace = true
tonic.workspace = true
tonic-web.workspace = true
prost.workspace = true
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
thiserror.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
reqwest = { workspace = true, features = ["stream"] }
tower-http = { version = "0.5", features = ["cors"] }
futures = "0.3"
tokio-stream = "0.1"
```

**Step 4: Verify compilation**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds

**Step 5: Commit**

```bash
git add cvm-agent/
git commit -m "feat: implement Redpill LLM client

- Anthropic-compatible API client
- Streaming response support
- Configurable via environment variables"
```

---

## Task 6: Implement Chat Service

**Files:**
- Create: `cvm-agent/agent-api/src/services/chat.rs`
- Modify: `cvm-agent/agent-api/src/services/mod.rs`
- Modify: `cvm-agent/agent-api/src/main.rs`

**Step 1: Add chat service module**

Update `cvm-agent/agent-api/src/services/mod.rs`:
```rust
pub mod chat;
pub mod health;
pub mod shell;

pub use chat::ChatServiceImpl;
pub use health::HealthServiceImpl;
pub use shell::ShellServiceImpl;
```

**Step 2: Implement ChatService**

Create `cvm-agent/agent-api/src/services/chat.rs`:
```rust
use std::pin::Pin;
use std::sync::Arc;
use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use super::health::proto::chat_service_server::ChatService;
use super::health::proto::{chat_response, ChatRequest, ChatResponse};
use crate::llm::{RedpillClient, redpill::Message};

pub struct ChatServiceImpl {
    llm: Arc<RedpillClient>,
}

impl ChatServiceImpl {
    pub fn new(llm: Arc<RedpillClient>) -> Self {
        Self { llm }
    }
}

#[tonic::async_trait]
impl ChatService for ChatServiceImpl {
    type SendMessageStream = Pin<Box<dyn Stream<Item = Result<ChatResponse, Status>> + Send>>;

    async fn send_message(
        &self,
        request: Request<ChatRequest>,
    ) -> Result<Response<Self::SendMessageStream>, Status> {
        let req = request.into_inner();
        let message = req.message;

        let (tx, rx) = mpsc::channel(128);
        let llm = self.llm.clone();

        tokio::spawn(async move {
            let (llm_tx, mut llm_rx) = mpsc::channel(128);

            let messages = vec![
                Message {
                    role: "user".to_string(),
                    content: message,
                },
            ];

            let llm_handle = {
                let llm = llm.clone();
                tokio::spawn(async move {
                    if let Err(e) = llm.chat_stream(messages, llm_tx).await {
                        tracing::error!("LLM error: {}", e);
                    }
                })
            };

            while let Some(result) = llm_rx.recv().await {
                match result {
                    Ok(text) => {
                        let _ = tx
                            .send(Ok(ChatResponse {
                                response: Some(chat_response::Response::Text(text)),
                            }))
                            .await;
                    }
                    Err(e) => {
                        let _ = tx
                            .send(Ok(ChatResponse {
                                response: Some(chat_response::Response::Error(e.to_string())),
                            }))
                            .await;
                    }
                }
            }

            let _ = llm_handle.await;

            let _ = tx
                .send(Ok(ChatResponse {
                    response: Some(chat_response::Response::Done(true)),
                }))
                .await;
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }
}
```

**Step 3: Update main.rs with ChatService**

Update `cvm-agent/agent-api/src/main.rs`:
```rust
mod llm;
mod services;

use std::net::SocketAddr;
use std::sync::Arc;
use tonic::transport::Server;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use llm::RedpillClient;
use services::health::proto::chat_service_server::ChatServiceServer;
use services::health::proto::health_service_server::HealthServiceServer;
use services::health::proto::shell_service_server::ShellServiceServer;
use services::{ChatServiceImpl, HealthServiceImpl, ShellServiceImpl};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let addr: SocketAddr = "0.0.0.0:8080".parse()?;
    tracing::info!("CVM Agent API listening on {}", addr);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods(Any);

    // Initialize LLM client (will fail gracefully if not configured)
    let llm_client = match RedpillClient::new() {
        Ok(client) => {
            tracing::info!("Redpill LLM client initialized");
            Arc::new(client)
        }
        Err(e) => {
            tracing::warn!("LLM client not configured: {}. Chat service will return errors.", e);
            // Create a dummy client that will error on use
            Arc::new(RedpillClient::new_dummy())
        }
    };

    let health_service = HealthServiceImpl;
    let shell_service = ShellServiceImpl;
    let chat_service = ChatServiceImpl::new(llm_client);

    Server::builder()
        .accept_http1(true)
        .layer(cors)
        .layer(tonic_web::GrpcWebLayer::new())
        .add_service(HealthServiceServer::new(health_service))
        .add_service(ShellServiceServer::new(shell_service))
        .add_service(ChatServiceServer::new(chat_service))
        .serve(addr)
        .await?;

    Ok(())
}
```

**Step 4: Add dummy client method for unconfigured state**

Update `cvm-agent/agent-api/src/llm/redpill.rs`, add after `impl RedpillClient`:
```rust
impl RedpillClient {
    pub fn new() -> Result<Self> {
        // ... existing implementation
    }

    pub fn new_dummy() -> Self {
        Self {
            client: Client::new(),
            api_key: String::new(),
            base_url: String::new(),
            model: String::new(),
        }
    }

    pub async fn chat_stream(
        &self,
        messages: Vec<Message>,
        tx: mpsc::Sender<Result<String>>,
    ) -> Result<()> {
        if self.api_key.is_empty() {
            tx.send(Err(anyhow::anyhow!("LLM not configured. Set REDPILL_API_KEY."))).await.ok();
            return Ok(());
        }
        // ... rest of existing implementation
    }
}
```

**Step 5: Verify compilation**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds

**Step 6: Commit**

```bash
git add cvm-agent/
git commit -m "feat: implement ChatService with LLM integration

- Streaming chat responses via gRPC
- Redpill/Anthropic API integration
- Graceful handling when LLM not configured"
```

---

## Task 7: Initialize Web UI Project

**Files:**
- Create: `cvm-agent/web-ui/package.json`
- Create: `cvm-agent/web-ui/tsconfig.json`
- Create: `cvm-agent/web-ui/vite.config.ts`
- Create: `cvm-agent/web-ui/index.html`
- Create: `cvm-agent/web-ui/src/main.tsx`
- Create: `cvm-agent/web-ui/src/App.tsx`
- Create: `cvm-agent/web-ui/tailwind.config.js`
- Create: `cvm-agent/web-ui/postcss.config.js`

**Step 1: Create web-ui directory**

```bash
mkdir -p cvm-agent/web-ui/src
```

**Step 2: Create package.json**

Create `cvm-agent/web-ui/package.json`:
```json
{
  "name": "cvm-agent-web",
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "proto": "buf generate ../proto"
  },
  "dependencies": {
    "@bufbuild/protobuf": "^2.0.0",
    "@connectrpc/connect": "^2.0.0",
    "@connectrpc/connect-web": "^2.0.0",
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@bufbuild/buf": "^1.28.0",
    "@bufbuild/protoc-gen-es": "^2.0.0",
    "@connectrpc/protoc-gen-connect-es": "^2.0.0",
    "@types/react": "^18.3.0",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.2.0",
    "autoprefixer": "^10.4.16",
    "postcss": "^8.4.32",
    "tailwindcss": "^3.4.0",
    "typescript": "^5.3.0",
    "vite": "^5.0.0"
  }
}
```

**Step 3: Create tsconfig.json**

Create `cvm-agent/web-ui/tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"]
}
```

**Step 4: Create vite.config.ts**

Create `cvm-agent/web-ui/vite.config.ts`:
```typescript
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  server: {
    port: 3000,
    proxy: {
      '/api': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, ''),
      },
    },
  },
  build: {
    outDir: 'dist',
  },
})
```

**Step 5: Create index.html**

Create `cvm-agent/web-ui/index.html`:
```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>CVM Agent</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

**Step 6: Create Tailwind config files**

Create `cvm-agent/web-ui/tailwind.config.js`:
```javascript
/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {},
  },
  plugins: [],
}
```

Create `cvm-agent/web-ui/postcss.config.js`:
```javascript
export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
}
```

**Step 7: Create main.tsx**

Create `cvm-agent/web-ui/src/main.tsx`:
```tsx
import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import './index.css'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
```

**Step 8: Create index.css**

Create `cvm-agent/web-ui/src/index.css`:
```css
@tailwind base;
@tailwind components;
@tailwind utilities;

body {
  margin: 0;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
}
```

**Step 9: Create minimal App.tsx**

Create `cvm-agent/web-ui/src/App.tsx`:
```tsx
function App() {
  return (
    <div className="min-h-screen bg-gray-900 text-white flex items-center justify-center">
      <div className="text-center">
        <h1 className="text-2xl font-bold mb-4">CVM Agent</h1>
        <p className="text-gray-400">Web UI initializing...</p>
      </div>
    </div>
  )
}

export default App
```

**Step 10: Install dependencies and verify**

Run:
```bash
cd cvm-agent/web-ui && npm install && npm run build
```
Expected: Build succeeds, outputs to dist/

**Step 11: Commit**

```bash
git add cvm-agent/web-ui/
git commit -m "feat: initialize web UI project

- React + TypeScript + Vite setup
- Tailwind CSS for styling
- Connect-web for gRPC client"
```

---

## Task 8: Generate gRPC Client Code

**Files:**
- Create: `cvm-agent/web-ui/buf.gen.yaml`
- Create: `cvm-agent/web-ui/src/gen/` (generated)

**Step 1: Create buf.gen.yaml**

Create `cvm-agent/web-ui/buf.gen.yaml`:
```yaml
version: v1
plugins:
  - plugin: es
    out: src/gen
    opt: target=ts
  - plugin: connect-es
    out: src/gen
    opt: target=ts
```

**Step 2: Generate client code**

Run:
```bash
cd cvm-agent/web-ui && npx buf generate ../proto
```
Expected: Creates src/gen/ with TypeScript client code

**Step 3: Verify generated files**

Run:
```bash
ls cvm-agent/web-ui/src/gen/
```
Expected: `agent_pb.ts`, `agent_connect.ts`

**Step 4: Add .gitignore for generated files (optional, or commit them)**

For reproducibility, we'll commit generated files.

**Step 5: Commit**

```bash
git add cvm-agent/web-ui/
git commit -m "feat: generate gRPC-web client code

- Buf codegen configuration
- TypeScript client stubs for all services"
```

---

## Task 9: Implement Chat UI Component

**Files:**
- Create: `cvm-agent/web-ui/src/components/ChatOverlay.tsx`
- Create: `cvm-agent/web-ui/src/hooks/useAgent.ts`
- Modify: `cvm-agent/web-ui/src/App.tsx`

**Step 1: Create components directory**

```bash
mkdir -p cvm-agent/web-ui/src/components
mkdir -p cvm-agent/web-ui/src/hooks
```

**Step 2: Create useAgent hook**

Create `cvm-agent/web-ui/src/hooks/useAgent.ts`:
```typescript
import { createConnectTransport } from "@connectrpc/connect-web";
import { createClient } from "@connectrpc/connect";
import { ChatService } from "../gen/agent_connect";
import { useState, useCallback } from "react";

const transport = createConnectTransport({
  baseUrl: "/api",
});

const chatClient = createClient(ChatService, transport);

export function useAgent() {
  const [messages, setMessages] = useState<Array<{ role: string; content: string }>>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [currentResponse, setCurrentResponse] = useState("");

  const sendMessage = useCallback(async (message: string) => {
    setIsLoading(true);
    setMessages((prev) => [...prev, { role: "user", content: message }]);
    setCurrentResponse("");

    try {
      let fullResponse = "";
      for await (const response of chatClient.sendMessage({ message })) {
        if (response.response.case === "text") {
          fullResponse += response.response.value;
          setCurrentResponse(fullResponse);
        } else if (response.response.case === "error") {
          fullResponse = `Error: ${response.response.value}`;
          setCurrentResponse(fullResponse);
          break;
        } else if (response.response.case === "done") {
          break;
        }
      }
      setMessages((prev) => [...prev, { role: "assistant", content: fullResponse }]);
      setCurrentResponse("");
    } catch (error) {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: `Error: ${error}` },
      ]);
    } finally {
      setIsLoading(false);
    }
  }, []);

  return { messages, sendMessage, isLoading, currentResponse };
}
```

**Step 3: Create ChatOverlay component**

Create `cvm-agent/web-ui/src/components/ChatOverlay.tsx`:
```tsx
import { useState, useRef, useEffect } from "react";
import { useAgent } from "../hooks/useAgent";

interface ChatOverlayProps {
  isOpen: boolean;
  onClose: () => void;
  isExpanded: boolean;
  onToggleExpand: () => void;
}

export function ChatOverlay({ isOpen, onClose, isExpanded, onToggleExpand }: ChatOverlayProps) {
  const [input, setInput] = useState("");
  const { messages, sendMessage, isLoading, currentResponse } = useAgent();
  const inputRef = useRef<HTMLInputElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (isOpen && inputRef.current) {
      inputRef.current.focus();
    }
  }, [isOpen]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, currentResponse]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (input.trim() && !isLoading) {
      sendMessage(input.trim());
      setInput("");
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
    }
  };

  if (!isOpen) return null;

  return (
    <div
      className={`fixed z-50 transition-all duration-200 ${
        isExpanded
          ? "top-0 right-0 w-96 h-full"
          : "bottom-4 left-1/2 -translate-x-1/2 w-[600px]"
      }`}
      onKeyDown={handleKeyDown}
    >
      <div
        className={`bg-gray-900/95 backdrop-blur-sm border border-gray-700 shadow-2xl ${
          isExpanded ? "h-full rounded-l-lg" : "rounded-xl"
        }`}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-2 border-b border-gray-700">
          <span className="text-sm text-gray-400">CVM Agent</span>
          <div className="flex gap-2">
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

        {/* Messages (only in expanded mode) */}
        {isExpanded && (
          <div className="h-[calc(100%-120px)] overflow-y-auto p-4 space-y-4">
            {messages.map((msg, i) => (
              <div
                key={i}
                className={`${
                  msg.role === "user" ? "text-blue-400" : "text-gray-200"
                }`}
              >
                <span className="text-xs text-gray-500 uppercase">
                  {msg.role}
                </span>
                <p className="mt-1 whitespace-pre-wrap">{msg.content}</p>
              </div>
            ))}
            {currentResponse && (
              <div className="text-gray-200">
                <span className="text-xs text-gray-500 uppercase">assistant</span>
                <p className="mt-1 whitespace-pre-wrap">{currentResponse}</p>
              </div>
            )}
            <div ref={messagesEndRef} />
          </div>
        )}

        {/* Input */}
        <form onSubmit={handleSubmit} className="p-3 border-t border-gray-700">
          <div className="flex gap-2">
            <input
              ref={inputRef}
              type="text"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder="Ask the agent..."
              disabled={isLoading}
              className="flex-1 bg-gray-800 border border-gray-600 rounded-lg px-4 py-2 text-white placeholder-gray-500 focus:outline-none focus:border-blue-500"
            />
            <button
              type="submit"
              disabled={isLoading || !input.trim()}
              className="px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 rounded-lg text-white font-medium"
            >
              {isLoading ? "..." : "➤"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
```

**Step 4: Update App.tsx with overlay and hotkey**

Replace `cvm-agent/web-ui/src/App.tsx`:
```tsx
import { useState, useEffect } from "react";
import { ChatOverlay } from "./components/ChatOverlay";

function App() {
  const [isOpen, setIsOpen] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);

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
      {/* Placeholder for noVNC - in production this would be the noVNC canvas */}
      <div className="flex items-center justify-center h-screen text-gray-600">
        <div className="text-center">
          <p className="text-sm">Press <kbd className="px-2 py-1 bg-gray-800 rounded">⌘</kbd> + <kbd className="px-2 py-1 bg-gray-800 rounded">⇧</kbd> + <kbd className="px-2 py-1 bg-gray-800 rounded">Space</kbd> to open agent</p>
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
        >
          💬
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
      />
    </div>
  );
}

export default App;
```

**Step 5: Verify build**

Run:
```bash
cd cvm-agent/web-ui && npm run build
```
Expected: Build succeeds

**Step 6: Commit**

```bash
git add cvm-agent/web-ui/
git commit -m "feat: implement chat overlay UI

- ChatOverlay component with expand/collapse
- useAgent hook for gRPC communication
- Hotkey activation (cmd+shift+space)
- Streaming response display"
```

---

## Task 10: Create NixOS Module

**Files:**
- Create: `modules/cvm-agent.nix`
- Modify: `hosts/phala-cvm/default.nix`

**Step 1: Create cvm-agent NixOS module**

Create `modules/cvm-agent.nix`:
```nix
{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.services.cvm-agent;
in
{
  options.services.cvm-agent = {
    enable = mkEnableOption "CVM Agent service";

    port = mkOption {
      type = types.port;
      default = 8080;
      description = "Port for the agent API";
    };

    webPort = mkOption {
      type = types.port;
      default = 8081;
      description = "Port for the web UI";
    };

    environment = mkOption {
      type = types.attrsOf types.str;
      default = {};
      description = "Environment variables for the agent";
      example = {
        REDPILL_API_KEY = "your-key";
        REDPILL_MODEL = "claude-3-5-sonnet-20241022";
      };
    };
  };

  config = mkIf cfg.enable {
    # Agent API service
    systemd.services.cvm-agent = {
      description = "CVM Agent API";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];

      environment = {
        RUST_LOG = "info";
      } // cfg.environment;

      serviceConfig = {
        Type = "simple";
        # TODO: Replace with actual package path after building
        ExecStart = "/opt/cvm-agent/agent-api";
        Restart = "always";
        RestartSec = "5s";
      };
    };

    # Open firewall ports
    networking.firewall.allowedTCPPorts = [ cfg.port cfg.webPort ];
  };
}
```

**Step 2: Update phala-cvm host config**

Read current file first, then add the import.

This step requires reading the current `hosts/phala-cvm/default.nix` and adding the cvm-agent module. The exact edit depends on current file structure.

**Step 3: Commit**

```bash
git add modules/cvm-agent.nix
git commit -m "feat: add cvm-agent NixOS module

- Systemd service definition
- Configurable ports and environment
- Firewall rules"
```

---

## Task 11: Update Docker Compose

**Files:**
- Modify: `files/phala-cvm/docker-compose.yml`
- Modify: `files/phala-cvm/entrypoint.sh`

**Step 1: Update docker-compose.yml ports**

Add new port mappings for agent services. Read current file, add ports 8080 and 8081.

**Step 2: Update entrypoint.sh to start agent**

Add agent startup after XFCE initialization. This will need to build and run the agent binary.

**Step 3: Commit**

```bash
git add files/phala-cvm/
git commit -m "feat: integrate agent into CVM deployment

- Add agent API and web UI ports
- Start agent service in entrypoint"
```

---

## Task 12: Integration Test

**Step 1: Build everything**

```bash
cd cvm-agent && cargo build --release
cd web-ui && npm run build
```

**Step 2: Start agent locally (for testing)**

```bash
cd cvm-agent
RUST_LOG=debug cargo run &

# In another terminal
cd cvm-agent/web-ui && npm run dev
```

**Step 3: Test in browser**

Open http://localhost:3000
- Press cmd+shift+space
- Type "hello"
- Verify response streams (will error if no REDPILL_API_KEY)

**Step 4: Test shell service**

Use grpcurl or write a quick test:
```bash
# If you have grpcurl
grpcurl -plaintext -d '{"command": "echo hello"}' localhost:8080 cvm.agent.ShellService/Execute
```

**Step 5: Final commit**

```bash
git add -A
git commit -m "feat: complete Phase 1 - CVM Agent core infrastructure

Phase 1 complete:
- Rust gRPC server with Health, Shell, Chat services
- Redpill LLM integration with streaming
- React web UI with overlay and hotkey
- NixOS module for deployment"
```

---

## Summary

After completing all tasks, you will have:

1. **Rust API Server** (`cvm-agent/agent-api/`)
   - HealthService - liveness checks
   - ShellService - streaming command execution
   - ChatService - LLM-powered chat with streaming

2. **Web UI** (`cvm-agent/web-ui/`)
   - React + TypeScript + Tailwind
   - gRPC-web client
   - Overlay with hotkey activation
   - Streaming response display

3. **NixOS Integration** (`modules/cvm-agent.nix`)
   - Systemd service
   - Configurable environment

4. **Docker Integration** (`files/phala-cvm/`)
   - Updated ports
   - Agent startup in entrypoint

## Next Phase

Phase 2 will add:
- NixOpsService (rebuild, rollback, generations)
- GitOpsService (commit, push, status)
- Risk classification for config edits
- Diff preview UI
