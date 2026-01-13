# CVM Agent API Guide

The CVM Agent provides a gRPC API at `localhost:8080` for controlling the NixOS VM.

## Services Overview

| Service | Purpose |
|---------|---------|
| `HealthService` | Health checks |
| `ChatService` | LLM-powered natural language interaction |
| `ShellService` | Execute shell commands |
| `NixOpsService` | NixOS rebuild/rollback operations |
| `GitOpsService` | Git version control |
| `GUIService` | GUI automation (screenshots, clicks, typing) |
| `VoiceService` | Voice transcription (Whisper) |
| `SkillsService` | Skill management |
| `MemoryService` | Persistent memory |

## Quick Examples

### Using grpcurl (from host)

```bash
# Install grpcurl first
brew install grpcurl  # macOS
# or: go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest

# Health check
grpcurl -plaintext localhost:8080 cvm.agent.HealthService/Check

# Execute shell command
grpcurl -plaintext -d '{"command": "ls -la /app"}' \
  localhost:8080 cvm.agent.ShellService/Execute

# Send chat message (streams response)
grpcurl -plaintext -d '{"message": "What files are in the home directory?"}' \
  localhost:8080 cvm.agent.ChatService/SendMessage

# Take screenshot
grpcurl -plaintext localhost:8080 cvm.agent.GUIService/Screenshot

# Press keyboard keys
grpcurl -plaintext -d '{"keys": ["ctrl", "shift", "space"]}' \
  localhost:8080 cvm.agent.GUIService/KeyPress

# Click at coordinates
grpcurl -plaintext -d '{"position": {"x": 500, "y": 300}, "button": "MOUSE_LEFT"}' \
  localhost:8080 cvm.agent.GUIService/Click
```

### Using Rust Client (from code)

```rust
use tonic::transport::Channel;
use cvm_agent::proto::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let channel = Channel::from_static("http://localhost:8080")
        .connect()
        .await?;

    // Shell command
    let mut shell = shell_service_client::ShellServiceClient::new(channel.clone());
    let request = ShellRequest {
        command: "ls -la".to_string(),
        working_dir: "/app".to_string(),
        timeout_seconds: 30,
    };
    let mut stream = shell.execute(request).await?.into_inner();
    while let Some(output) = stream.message().await? {
        if let Some(out) = output.output {
            println!("{}", out);
        }
    }

    // Chat
    let mut chat = chat_service_client::ChatServiceClient::new(channel.clone());
    let request = ChatRequest {
        message: "What is the current time?".to_string(),
        conversation_id: "".to_string(),
    };
    let mut stream = chat.send_message(request).await?.into_inner();
    while let Some(response) = stream.message().await? {
        if let Some(text) = response.response {
            print!("{}", text);
        }
    }

    Ok(())
}
```

### Using TypeScript/JavaScript (from Tauri app)

```typescript
// Using grpc-web client
import { ChatServiceClient } from './proto/agent_grpc_web_pb';
import { ChatRequest } from './proto/agent_pb';

const client = new ChatServiceClient('http://localhost:8080');

const request = new ChatRequest();
request.setMessage('Open Firefox browser');

const stream = client.sendMessage(request);
stream.on('data', (response) => {
  console.log(response.getText());
});
stream.on('end', () => {
  console.log('Stream ended');
});
```

## Service Details

### ChatService - Natural Language Control

The primary interface. Send natural language commands and the LLM agent executes them.

```protobuf
rpc SendMessage(ChatRequest) returns (stream ChatResponse);
```

**Examples:**
- "Open Firefox and navigate to github.com"
- "Install htop using nix"
- "Show me the current git status"
- "Take a screenshot and describe what's on screen"

### ShellService - Direct Command Execution

Execute shell commands with streaming output.

```protobuf
rpc Execute(ShellRequest) returns (stream ShellOutput);
```

**Fields:**
- `command`: The shell command to run
- `working_dir`: Working directory (default: current)
- `timeout_seconds`: Command timeout

### GUIService - GUI Automation

Control the desktop environment programmatically.

| Method | Purpose |
|--------|---------|
| `Screenshot` | Capture screen image |
| `Click` | Mouse click at element or coordinates |
| `Type` | Type text into focused element |
| `KeyPress` | Press keyboard keys (shortcuts) |
| `MoveMouse` | Move cursor |
| `GetElements` | Query UI accessibility tree |
| `Describe` | Use vision model to describe screen |

**Common key combinations:**
```bash
# Open terminal
grpcurl -plaintext -d '{"keys": ["ctrl", "alt", "t"]}' localhost:8080 cvm.agent.GUIService/KeyPress

# Toggle overlay (Tauri app)
grpcurl -plaintext -d '{"keys": ["ctrl", "shift", "space"]}' localhost:8080 cvm.agent.GUIService/KeyPress

# Copy/paste
grpcurl -plaintext -d '{"keys": ["ctrl", "c"]}' localhost:8080 cvm.agent.GUIService/KeyPress
grpcurl -plaintext -d '{"keys": ["ctrl", "v"]}' localhost:8080 cvm.agent.GUIService/KeyPress
```

### NixOpsService - NixOS Operations

```protobuf
rpc Rebuild(RebuildRequest) returns (stream RebuildOutput);
rpc Rollback(RollbackRequest) returns (stream RebuildOutput);
rpc ListGenerations(ListGenerationsRequest) returns (ListGenerationsResponse);
```

**Actions for Rebuild:**
- `switch`: Build and activate (default boot)
- `boot`: Build, set as boot option, don't activate
- `test`: Build and activate, don't set as boot
- `build`: Build only

### MemoryService - Persistent Context

The agent maintains memory across sessions:

| Layer | Purpose |
|-------|---------|
| `WORKING` | Current session context |
| `ARCHIVE` | Summarized past conversations |
| `FACTS` | System state facts |
| `PREFERENCES` | Learned user preferences |

## Ports

| Port | Service |
|------|---------|
| 8080 | Agent API (gRPC) |
| 3000 | Web UI |
| 8082 | Whisper (voice) |
| 5900 | VNC |
| 6080 | noVNC (web) |

## From Inside Container

The Agent API is also accessible from inside the container:

```bash
# Using grpcurl from container shell
grpcurl -plaintext localhost:8080 cvm.agent.HealthService/Check
```
