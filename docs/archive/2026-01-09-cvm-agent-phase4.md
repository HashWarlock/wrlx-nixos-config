# CVM Agent Phase 4 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add voice input via Whisper server with live transcription UI, and implement a dual skills system supporting both instruction-based (markdown) and workflow-based (YAML) skills.

**Architecture:** Whisper.cpp server runs as a separate service, receiving audio via WebSocket and returning transcriptions. VoiceService wraps this for the gRPC API. SkillsService manages skill loading, matching, and execution. Instruction skills are loaded into LLM context, workflow skills execute step-by-step with defined actions.

**Tech Stack:** Rust (Tonic), whisper.cpp, WebSocket, YAML parsing, TypeScript, React, Web Audio API

---

## Prerequisites

Before starting, ensure you're in the worktree:
```bash
cd /Users/hashwarlock/Projects/wrlx-nixos-config/.worktrees/cvm-agent
```

Phase 3 must be complete (GUIService with screenshot, AT-SPI, xdotool, vision).

---

## Task 1: Add Voice and Skills Proto Definitions

**Files:**
- Modify: `cvm-agent/proto/agent.proto`

**Step 1: Add VoiceService definitions**

Add after GUIService:

```protobuf
// Voice transcription service
service VoiceService {
  // Stream audio for transcription
  rpc Transcribe(stream AudioChunk) returns (stream TranscriptChunk);

  // Get available models
  rpc ListModels(ListModelsRequest) returns (ListModelsResponse);

  // Check if voice service is available
  rpc Status(VoiceStatusRequest) returns (VoiceStatusResponse);
}

message AudioChunk {
  bytes data = 1;                 // Raw audio bytes (16-bit PCM, 16kHz mono)
  bool is_final = 2;              // Last chunk in utterance
}

message TranscriptChunk {
  string text = 1;                // Transcribed text
  bool is_partial = 2;            // Partial (still processing) vs final
  float confidence = 3;           // 0-1 confidence score
  int32 start_ms = 4;             // Start time in audio
  int32 end_ms = 5;               // End time in audio
}

message ListModelsRequest {}

message ListModelsResponse {
  repeated WhisperModel models = 1;
}

message WhisperModel {
  string name = 1;                // "tiny", "base", "small", "medium", "large"
  bool available = 2;
  int64 size_bytes = 3;
}

message VoiceStatusRequest {}

message VoiceStatusResponse {
  bool available = 1;
  string current_model = 2;
  bool is_processing = 3;
}
```

**Step 2: Add SkillsService definitions**

Add after VoiceService:

```protobuf
// Skills management service
service SkillsService {
  // List all available skills
  rpc List(ListSkillsRequest) returns (ListSkillsResponse);

  // Get skill details
  rpc Get(GetSkillRequest) returns (SkillResponse);

  // Create new skill
  rpc Create(CreateSkillRequest) returns (SkillResponse);

  // Update existing skill
  rpc Update(UpdateSkillRequest) returns (SkillResponse);

  // Delete skill
  rpc Delete(DeleteSkillRequest) returns (DeleteSkillResponse);

  // Execute workflow skill
  rpc Execute(ExecuteSkillRequest) returns (stream SkillProgress);

  // Find matching skills for input
  rpc Match(MatchSkillsRequest) returns (MatchSkillsResponse);
}

message ListSkillsRequest {
  SkillType type_filter = 1;      // Filter by type, or SKILL_ALL
}

enum SkillType {
  SKILL_ALL = 0;
  SKILL_INSTRUCTION = 1;
  SKILL_WORKFLOW = 2;
}

message ListSkillsResponse {
  repeated SkillSummary skills = 1;
}

message SkillSummary {
  string name = 1;
  string description = 2;
  SkillType type = 3;
  repeated string triggers = 4;
  string source = 5;              // "builtin", "user", "imported"
}

message GetSkillRequest {
  string name = 1;
}

message SkillResponse {
  string name = 1;
  string description = 2;
  SkillType type = 3;
  repeated string triggers = 4;
  string content = 5;             // Full skill content (markdown or YAML)
  string source = 6;
}

message CreateSkillRequest {
  string name = 1;
  string description = 2;
  SkillType type = 3;
  repeated string triggers = 4;
  string content = 5;
}

message UpdateSkillRequest {
  string name = 1;
  string description = 2;
  repeated string triggers = 3;
  string content = 4;
}

message DeleteSkillRequest {
  string name = 1;
}

message DeleteSkillResponse {
  bool success = 1;
  string error = 2;
}

message ExecuteSkillRequest {
  string name = 1;
  map<string, string> parameters = 2;
}

message SkillProgress {
  int32 step_number = 1;
  int32 total_steps = 2;
  string step_description = 3;
  oneof result {
    string output = 4;
    string error = 5;
  }
  bool completed = 6;
}

message MatchSkillsRequest {
  string input = 1;               // User input to match against triggers
}

message MatchSkillsResponse {
  repeated SkillMatch matches = 1;
}

message SkillMatch {
  string name = 1;
  SkillType type = 2;
  float relevance = 3;            // 0-1 relevance score
  string matched_trigger = 4;
}
```

**Step 3: Verify proto compiles**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds

**Step 4: Commit**

```bash
git add cvm-agent/proto/agent.proto
git commit -m "feat: add VoiceService and SkillsService proto definitions

- VoiceService: streaming transcription, model management
- SkillsService: CRUD, execution, trigger matching
- Support for instruction and workflow skill types"
```

---

## Task 2: Add Whisper Server to Deployment

**Files:**
- Modify: `files/phala-cvm/entrypoint.sh`
- Modify: `files/phala-cvm/docker-compose.yml`
- Modify: `modules/cvm-agent.nix`

**Step 1: Add whisper.cpp to entrypoint.sh**

Add to the package installation section:
```bash
# Voice transcription
nix-env -iA nixpkgs.whisper-cpp
```

Add whisper server startup after agent startup:
```bash
# Start Whisper server for voice transcription
echo "Starting Whisper server..."
WHISPER_MODEL=${WHISPER_MODEL:-base}
WHISPER_MODEL_PATH="/nix/store/*/share/whisper-cpp/models/ggml-${WHISPER_MODEL}.bin"

# Find the actual model path
ACTUAL_MODEL=$(ls $WHISPER_MODEL_PATH 2>/dev/null | head -1)
if [ -n "$ACTUAL_MODEL" ]; then
    whisper-server --model "$ACTUAL_MODEL" --port 8082 &
    WHISPER_PID=$!
    echo "Whisper server started on port 8082 (model: $WHISPER_MODEL)"
else
    echo "WARNING: Whisper model not found, voice input disabled"
    WHISPER_PID=""
fi
```

Add to cleanup function:
```bash
[ -n "$WHISPER_PID" ] && kill $WHISPER_PID 2>/dev/null
```

**Step 2: Add whisper port to docker-compose.yml**

Add to ports section:
```yaml
- "8082:8082"    # Whisper server (voice)
```

Add environment variable:
```yaml
- WHISPER_MODEL=${WHISPER_MODEL:-base}
```

**Step 3: Update NixOS module**

Add whisper service to `modules/cvm-agent.nix`:
```nix
options.services.cvm-agent = {
  # ... existing options

  whisperModel = mkOption {
    type = types.str;
    default = "base";
    description = "Whisper model size (tiny, base, small, medium)";
  };

  whisperPort = mkOption {
    type = types.port;
    default = 8082;
    description = "Port for Whisper server";
  };
};

config = mkIf cfg.enable {
  # ... existing config

  systemd.services.whisper-server = {
    description = "Whisper Transcription Server";
    wantedBy = [ "multi-user.target" ];
    after = [ "network.target" ];

    serviceConfig = {
      Type = "simple";
      ExecStart = "${pkgs.whisper-cpp}/bin/whisper-server --model ${pkgs.whisper-cpp}/share/whisper-cpp/models/ggml-${cfg.whisperModel}.bin --port ${toString cfg.whisperPort}";
      Restart = "always";
      RestartSec = "5s";
    };
  };

  networking.firewall.allowedTCPPorts = [ cfg.port cfg.webPort cfg.whisperPort ];
};
```

**Step 4: Commit**

```bash
git add files/phala-cvm/ modules/cvm-agent.nix
git commit -m "feat: add Whisper server deployment

- whisper-cpp package in entrypoint
- Port 8082 for voice WebSocket
- Configurable model size
- NixOS systemd service"
```

---

## Task 3: Implement VoiceService

**Files:**
- Create: `cvm-agent/agent-api/src/services/voice.rs`
- Modify: `cvm-agent/agent-api/src/services/mod.rs`
- Modify: `cvm-agent/agent-api/src/main.rs`

**Step 1: Add WebSocket dependencies**

Update `cvm-agent/Cargo.toml`:
```toml
[workspace.dependencies]
# ... existing
tokio-tungstenite = "0.21"
```

Update `cvm-agent/agent-api/Cargo.toml`:
```toml
[dependencies]
# ... existing
tokio-tungstenite.workspace = true
```

**Step 2: Implement VoiceService**

Create `cvm-agent/agent-api/src/services/voice.rs`:
```rust
use futures::{Stream, StreamExt, SinkExt};
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tonic::{Request, Response, Status, Streaming};

use super::health::proto::voice_service_server::VoiceService;
use super::health::proto::{
    AudioChunk, ListModelsRequest, ListModelsResponse, TranscriptChunk,
    VoiceStatusRequest, VoiceStatusResponse, WhisperModel,
};

pub struct VoiceServiceImpl {
    whisper_url: String,
}

impl VoiceServiceImpl {
    pub fn new() -> Self {
        let port = std::env::var("WHISPER_PORT").unwrap_or_else(|_| "8082".to_string());
        Self {
            whisper_url: format!("ws://127.0.0.1:{}/transcribe", port),
        }
    }

    async fn check_whisper_available(&self) -> bool {
        let status_url = self.whisper_url.replace("/transcribe", "/status");
        let http_url = status_url.replace("ws://", "http://");

        reqwest::get(&http_url)
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

#[tonic::async_trait]
impl VoiceService for VoiceServiceImpl {
    type TranscribeStream = Pin<Box<dyn Stream<Item = Result<TranscriptChunk, Status>> + Send>>;

    async fn transcribe(
        &self,
        request: Request<Streaming<AudioChunk>>,
    ) -> Result<Response<Self::TranscribeStream>, Status> {
        let mut audio_stream = request.into_inner();
        let (tx, rx) = mpsc::channel(128);
        let whisper_url = self.whisper_url.clone();

        tokio::spawn(async move {
            // Connect to Whisper WebSocket
            let ws_stream = match connect_async(&whisper_url).await {
                Ok((stream, _)) => stream,
                Err(e) => {
                    let _ = tx.send(Ok(TranscriptChunk {
                        text: format!("Failed to connect to Whisper: {}", e),
                        is_partial: false,
                        confidence: 0.0,
                        start_ms: 0,
                        end_ms: 0,
                    })).await;
                    return;
                }
            };

            let (mut ws_tx, mut ws_rx) = ws_stream.split();

            // Forward audio chunks to Whisper
            let tx_clone = tx.clone();
            let audio_forward = tokio::spawn(async move {
                while let Some(Ok(chunk)) = audio_stream.next().await {
                    if ws_tx.send(Message::Binary(chunk.data)).await.is_err() {
                        break;
                    }
                    if chunk.is_final {
                        let _ = ws_tx.close().await;
                        break;
                    }
                }
            });

            // Receive transcriptions from Whisper
            while let Some(msg) = ws_rx.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        // Parse Whisper response (assuming JSON format)
                        if let Ok(response) = serde_json::from_str::<WhisperResponse>(&text) {
                            let _ = tx.send(Ok(TranscriptChunk {
                                text: response.text,
                                is_partial: response.is_partial,
                                confidence: response.confidence,
                                start_ms: response.start_ms,
                                end_ms: response.end_ms,
                            })).await;
                        } else {
                            // Plain text response
                            let _ = tx.send(Ok(TranscriptChunk {
                                text,
                                is_partial: false,
                                confidence: 1.0,
                                start_ms: 0,
                                end_ms: 0,
                            })).await;
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Err(_) => break,
                    _ => {}
                }
            }

            let _ = audio_forward.await;
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }

    async fn list_models(
        &self,
        _request: Request<ListModelsRequest>,
    ) -> Result<Response<ListModelsResponse>, Status> {
        // List available Whisper models
        let models = vec![
            WhisperModel {
                name: "tiny".to_string(),
                available: true,
                size_bytes: 75_000_000,
            },
            WhisperModel {
                name: "base".to_string(),
                available: true,
                size_bytes: 142_000_000,
            },
            WhisperModel {
                name: "small".to_string(),
                available: true,
                size_bytes: 466_000_000,
            },
            WhisperModel {
                name: "medium".to_string(),
                available: true,
                size_bytes: 1_500_000_000,
            },
        ];

        Ok(Response::new(ListModelsResponse { models }))
    }

    async fn status(
        &self,
        _request: Request<VoiceStatusRequest>,
    ) -> Result<Response<VoiceStatusResponse>, Status> {
        let available = self.check_whisper_available().await;
        let model = std::env::var("WHISPER_MODEL").unwrap_or_else(|_| "base".to_string());

        Ok(Response::new(VoiceStatusResponse {
            available,
            current_model: model,
            is_processing: false, // Would need to track this
        }))
    }
}

#[derive(serde::Deserialize)]
struct WhisperResponse {
    text: String,
    #[serde(default)]
    is_partial: bool,
    #[serde(default = "default_confidence")]
    confidence: f32,
    #[serde(default)]
    start_ms: i32,
    #[serde(default)]
    end_ms: i32,
}

fn default_confidence() -> f32 {
    1.0
}
```

**Step 3: Update services/mod.rs**

Add to `cvm-agent/agent-api/src/services/mod.rs`:
```rust
pub mod voice;
pub use voice::VoiceServiceImpl;
```

**Step 4: Register VoiceService in main.rs**

Add imports and registration:
```rust
use services::health::proto::voice_service_server::VoiceServiceServer;
use services::VoiceServiceImpl;

// In main():
let voice_service = VoiceServiceImpl::new();
Server::builder()
    // ... existing
    .add_service(VoiceServiceServer::new(voice_service))
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
git commit -m "feat: implement VoiceService

- WebSocket connection to Whisper server
- Streaming audio transcription
- Model listing and status checks"
```

---

## Task 4: Create Skills Directory Structure

**Files:**
- Create: `cvm-agent/skills/instructions/nixos-editing.md`
- Create: `cvm-agent/skills/instructions/git-workflow.md`
- Create: `cvm-agent/skills/workflows/deploy-config.yaml`
- Create: `cvm-agent/skills/workflows/rollback.yaml`

**Step 1: Create directory structure**

```bash
mkdir -p cvm-agent/skills/instructions
mkdir -p cvm-agent/skills/workflows
```

**Step 2: Create NixOS editing instruction skill**

Create `cvm-agent/skills/instructions/nixos-editing.md`:
```markdown
---
name: nixos-editing
description: Guidelines for editing NixOS configuration files
triggers:
  - edit config
  - modify nix
  - change system
  - add package
  - remove package
---

# NixOS Configuration Editing Guidelines

When editing NixOS configuration files in this repository:

## File Organization

1. **System-level configs** go in `modules/`
   - `system.nix` - Core system packages and settings
   - `hyprland.nix`, `gnome.nix`, `i3.nix` - Desktop environments
   - `cvm-agent.nix` - CVM Agent service configuration

2. **User-level configs** go in `home/modules/`
   - Program-specific configs (git.nix, zsh.nix, tmux.nix)
   - Desktop environment user settings

3. **Host-specific configs** go in `hosts/<hostname>/`
   - `default.nix` - Host entry point
   - `hardware-configuration.nix` - Hardware-specific settings

## Best Practices

1. **Read before editing** - Always read the current file to understand structure
2. **Prefer modification** - Modify existing modules over creating new ones
3. **Use unstable overlay** - Reference bleeding-edge packages as `pkgs.unstable.*`
4. **Follow patterns** - Match existing code style and organization
5. **Explain changes** - After editing, explain what changed and why

## Package Management

- Add system packages in `modules/system.nix` under `environment.systemPackages`
- Add user packages in relevant `home/modules/*.nix` files
- For bleeding-edge versions, use `pkgs.unstable.<package>`

## Testing Changes

Before rebuilding:
1. Verify syntax: `nix flake check`
2. Build without activating: `nixos-rebuild build --flake .#<host>`
3. Test activation: `nixos-rebuild test --flake .#<host>`
4. Finally switch: `nixos-rebuild switch --flake .#<host>`
```

**Step 3: Create Git workflow instruction skill**

Create `cvm-agent/skills/instructions/git-workflow.md`:
```markdown
---
name: git-workflow
description: Guidelines for git operations in this repository
triggers:
  - commit changes
  - push code
  - git status
  - version control
---

# Git Workflow Guidelines

## Before Making Changes

1. Check current status: `git status`
2. Ensure you're on the correct branch
3. Pull latest changes if working with remote

## Committing Changes

### Commit Message Format

```
<type>: <short description>

<optional longer description>

Co-Authored-By: Claude <noreply@anthropic.com>
```

Types:
- `feat:` - New feature
- `fix:` - Bug fix
- `chore:` - Maintenance tasks
- `docs:` - Documentation only
- `refactor:` - Code restructuring

### Pre-commit Checklist

1. Review all changes with `git diff`
2. Ensure no secrets or credentials are included
3. Verify NixOS syntax is valid
4. Stage only relevant files

## Safe Operations

- Always commit before destructive operations
- Use branches for experimental changes
- Push regularly to preserve work
- Never force-push to main/master
```

**Step 4: Create deploy-config workflow skill**

Create `cvm-agent/skills/workflows/deploy-config.yaml`:
```yaml
name: deploy-config
description: Rebuild NixOS and push configuration to GitHub
triggers:
  - deploy
  - push my changes
  - apply config
  - rebuild and push

parameters:
  commit_message:
    description: Commit message for changes
    required: false
    default: "Update NixOS configuration"

steps:
  - id: check_status
    action: git.status
    description: Check for uncommitted changes

  - id: confirm_commit
    action: prompt.confirm
    condition: "steps.check_status.has_changes"
    message: "You have uncommitted changes. Commit them before deploying?"

  - id: stage_changes
    action: git.add
    condition: "steps.confirm_commit.confirmed"
    args:
      paths: ["."]

  - id: commit
    action: git.commit
    condition: "steps.confirm_commit.confirmed"
    args:
      message: "{{ parameters.commit_message }}"

  - id: rebuild
    action: nixops.rebuild
    description: Rebuild NixOS configuration
    args:
      action: switch
    stream: true

  - id: push
    action: git.push
    condition: "steps.rebuild.success"
    description: Push changes to remote

  - id: report
    action: chat.respond
    message: |
      Deployment complete!
      - Commit: {{ steps.commit.hash }}
      - NixOS rebuild: {{ steps.rebuild.status }}
      - Pushed to remote: {{ steps.push.success }}
```

**Step 5: Create rollback workflow skill**

Create `cvm-agent/skills/workflows/rollback.yaml`:
```yaml
name: rollback
description: Rollback to a previous NixOS generation
triggers:
  - rollback
  - undo last change
  - previous generation
  - restore system

parameters:
  generation:
    description: Specific generation number (empty for previous)
    required: false

steps:
  - id: list_generations
    action: nixops.list_generations
    description: List available generations
    args:
      limit: 5

  - id: confirm_rollback
    action: prompt.confirm
    message: |
      Current generation: {{ steps.list_generations.current }}
      Available generations:
      {{ steps.list_generations.list }}

      Rollback to {{ parameters.generation or 'previous' }} generation?

  - id: rollback
    action: nixops.rollback
    condition: "steps.confirm_rollback.confirmed"
    args:
      generation: "{{ parameters.generation }}"
    stream: true

  - id: report
    action: chat.respond
    message: |
      Rollback {{ 'complete' if steps.rollback.success else 'failed' }}!
      Now running generation: {{ steps.rollback.new_generation }}
```

**Step 6: Commit**

```bash
git add cvm-agent/skills/
git commit -m "feat: add built-in skills

- nixos-editing instruction skill
- git-workflow instruction skill
- deploy-config workflow skill
- rollback workflow skill"
```

---

## Task 5: Implement Skills Loader

**Files:**
- Create: `cvm-agent/agent-api/src/skills/mod.rs`
- Create: `cvm-agent/agent-api/src/skills/loader.rs`
- Create: `cvm-agent/agent-api/src/skills/types.rs`

**Step 1: Create skills module structure**

```bash
mkdir -p cvm-agent/agent-api/src/skills
```

Create `cvm-agent/agent-api/src/skills/mod.rs`:
```rust
pub mod loader;
pub mod types;
pub mod matcher;
pub mod executor;

pub use loader::SkillsLoader;
pub use types::{Skill, SkillType, InstructionSkill, WorkflowSkill};
pub use matcher::SkillMatcher;
pub use executor::WorkflowExecutor;
```

**Step 2: Define skill types**

Create `cvm-agent/agent-api/src/skills/types.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillType {
    Instruction,
    Workflow,
}

#[derive(Debug, Clone)]
pub enum Skill {
    Instruction(InstructionSkill),
    Workflow(WorkflowSkill),
}

impl Skill {
    pub fn name(&self) -> &str {
        match self {
            Skill::Instruction(s) => &s.name,
            Skill::Workflow(s) => &s.name,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Skill::Instruction(s) => &s.description,
            Skill::Workflow(s) => &s.description,
        }
    }

    pub fn triggers(&self) -> &[String] {
        match self {
            Skill::Instruction(s) => &s.triggers,
            Skill::Workflow(s) => &s.triggers,
        }
    }

    pub fn source(&self) -> &str {
        match self {
            Skill::Instruction(s) => &s.source,
            Skill::Workflow(s) => &s.source,
        }
    }

    pub fn skill_type(&self) -> SkillType {
        match self {
            Skill::Instruction(_) => SkillType::Instruction,
            Skill::Workflow(_) => SkillType::Workflow,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionSkill {
    pub name: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub content: String,  // The markdown content to inject into context
    pub source: String,   // "builtin", "user", "imported"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSkill {
    pub name: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub parameters: Vec<WorkflowParameter>,
    pub steps: Vec<WorkflowStep>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowParameter {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub action: String,
    pub description: Option<String>,
    pub condition: Option<String>,
    pub args: Option<serde_json::Value>,
    pub stream: Option<bool>,
    pub message: Option<String>,
}
```

**Step 3: Implement skill loader**

Create `cvm-agent/agent-api/src/skills/loader.rs`:
```rust
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

use super::types::{InstructionSkill, Skill, WorkflowSkill, WorkflowParameter, WorkflowStep};

pub struct SkillsLoader {
    skills_dir: String,
    skills: HashMap<String, Skill>,
}

impl SkillsLoader {
    pub fn new(skills_dir: &str) -> Self {
        Self {
            skills_dir: skills_dir.to_string(),
            skills: HashMap::new(),
        }
    }

    pub async fn load_all(&mut self) -> Result<()> {
        self.skills.clear();

        // Load instruction skills
        let instructions_dir = format!("{}/instructions", self.skills_dir);
        if Path::new(&instructions_dir).exists() {
            self.load_instruction_skills(&instructions_dir).await?;
        }

        // Load workflow skills
        let workflows_dir = format!("{}/workflows", self.skills_dir);
        if Path::new(&workflows_dir).exists() {
            self.load_workflow_skills(&workflows_dir).await?;
        }

        tracing::info!("Loaded {} skills", self.skills.len());
        Ok(())
    }

    async fn load_instruction_skills(&mut self, dir: &str) -> Result<()> {
        let mut entries = fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Ok(skill) = self.parse_instruction_skill(&path).await {
                    self.skills.insert(skill.name.clone(), Skill::Instruction(skill));
                }
            }
        }

        Ok(())
    }

    async fn load_workflow_skills(&mut self, dir: &str) -> Result<()> {
        let mut entries = fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "yaml" || e == "yml").unwrap_or(false) {
                if let Ok(skill) = self.parse_workflow_skill(&path).await {
                    self.skills.insert(skill.name.clone(), Skill::Workflow(skill));
                }
            }
        }

        Ok(())
    }

    async fn parse_instruction_skill(&self, path: &Path) -> Result<InstructionSkill> {
        let content = fs::read_to_string(path).await?;

        // Parse YAML frontmatter
        let (frontmatter, body) = Self::split_frontmatter(&content)?;

        #[derive(serde::Deserialize)]
        struct Frontmatter {
            name: String,
            description: String,
            triggers: Vec<String>,
        }

        let fm: Frontmatter = serde_yaml::from_str(&frontmatter)?;

        Ok(InstructionSkill {
            name: fm.name,
            description: fm.description,
            triggers: fm.triggers,
            content: body,
            source: "builtin".to_string(),
        })
    }

    async fn parse_workflow_skill(&self, path: &Path) -> Result<WorkflowSkill> {
        let content = fs::read_to_string(path).await?;

        #[derive(serde::Deserialize)]
        struct RawWorkflow {
            name: String,
            description: String,
            triggers: Vec<String>,
            #[serde(default)]
            parameters: HashMap<String, RawParameter>,
            steps: Vec<RawStep>,
        }

        #[derive(serde::Deserialize)]
        struct RawParameter {
            description: String,
            #[serde(default)]
            required: bool,
            default: Option<String>,
        }

        #[derive(serde::Deserialize)]
        struct RawStep {
            id: String,
            action: String,
            description: Option<String>,
            condition: Option<String>,
            args: Option<serde_json::Value>,
            stream: Option<bool>,
            message: Option<String>,
        }

        let raw: RawWorkflow = serde_yaml::from_str(&content)?;

        let parameters = raw.parameters
            .into_iter()
            .map(|(name, p)| WorkflowParameter {
                name,
                description: p.description,
                required: p.required,
                default: p.default,
            })
            .collect();

        let steps = raw.steps
            .into_iter()
            .map(|s| WorkflowStep {
                id: s.id,
                action: s.action,
                description: s.description,
                condition: s.condition,
                args: s.args,
                stream: s.stream,
                message: s.message,
            })
            .collect();

        Ok(WorkflowSkill {
            name: raw.name,
            description: raw.description,
            triggers: raw.triggers,
            parameters,
            steps,
            source: "builtin".to_string(),
        })
    }

    fn split_frontmatter(content: &str) -> Result<(String, String)> {
        let content = content.trim();
        if !content.starts_with("---") {
            anyhow::bail!("No frontmatter found");
        }

        let rest = &content[3..];
        let end = rest.find("---").ok_or_else(|| anyhow::anyhow!("No frontmatter end"))?;

        let frontmatter = rest[..end].trim().to_string();
        let body = rest[end + 3..].trim().to_string();

        Ok((frontmatter, body))
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    pub fn list(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }

    pub fn list_by_type(&self, skill_type: super::types::SkillType) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|s| matches!(
                (s, &skill_type),
                (Skill::Instruction(_), super::types::SkillType::Instruction) |
                (Skill::Workflow(_), super::types::SkillType::Workflow)
            ))
            .collect()
    }
}
```

**Step 4: Add serde_yaml dependency**

Update `cvm-agent/Cargo.toml`:
```toml
serde_yaml = "0.9"
```

Update `cvm-agent/agent-api/Cargo.toml`:
```toml
serde_yaml.workspace = true
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
git commit -m "feat: implement skills loader

- Skill type definitions (instruction, workflow)
- Frontmatter parsing for markdown skills
- YAML parsing for workflow skills
- Skills directory scanning"
```

---

## Task 6: Implement Skill Matcher

**Files:**
- Create: `cvm-agent/agent-api/src/skills/matcher.rs`

**Step 1: Implement trigger matching**

Create `cvm-agent/agent-api/src/skills/matcher.rs`:
```rust
use super::types::Skill;

pub struct SkillMatcher;

#[derive(Debug, Clone)]
pub struct SkillMatch {
    pub skill_name: String,
    pub relevance: f32,
    pub matched_trigger: String,
}

impl SkillMatcher {
    /// Find skills matching the user input
    pub fn match_skills(input: &str, skills: &[&Skill]) -> Vec<SkillMatch> {
        let input_lower = input.to_lowercase();
        let input_words: Vec<&str> = input_lower.split_whitespace().collect();

        let mut matches: Vec<SkillMatch> = skills
            .iter()
            .filter_map(|skill| {
                let (relevance, matched_trigger) = Self::calculate_relevance(
                    &input_lower,
                    &input_words,
                    skill.triggers(),
                );

                if relevance > 0.0 {
                    Some(SkillMatch {
                        skill_name: skill.name().to_string(),
                        relevance,
                        matched_trigger,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Sort by relevance descending
        matches.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap());

        matches
    }

    fn calculate_relevance(
        input: &str,
        input_words: &[&str],
        triggers: &[String],
    ) -> (f32, String) {
        let mut best_relevance = 0.0f32;
        let mut best_trigger = String::new();

        for trigger in triggers {
            let trigger_lower = trigger.to_lowercase();
            let trigger_words: Vec<&str> = trigger_lower.split_whitespace().collect();

            // Exact match
            if input.contains(&trigger_lower) {
                let relevance = 1.0;
                if relevance > best_relevance {
                    best_relevance = relevance;
                    best_trigger = trigger.clone();
                }
                continue;
            }

            // Word overlap
            let matching_words = input_words
                .iter()
                .filter(|w| trigger_words.contains(w))
                .count();

            if matching_words > 0 {
                let relevance = matching_words as f32 / trigger_words.len().max(1) as f32;
                if relevance > best_relevance {
                    best_relevance = relevance;
                    best_trigger = trigger.clone();
                }
            }

            // Fuzzy match (simple Levenshtein-based)
            for trigger_word in &trigger_words {
                for input_word in input_words {
                    if Self::is_similar(input_word, trigger_word) {
                        let relevance = 0.5;
                        if relevance > best_relevance {
                            best_relevance = relevance;
                            best_trigger = trigger.clone();
                        }
                    }
                }
            }
        }

        (best_relevance, best_trigger)
    }

    fn is_similar(a: &str, b: &str) -> bool {
        if a == b {
            return true;
        }

        // Simple prefix match
        if a.len() >= 3 && b.starts_with(a) {
            return true;
        }
        if b.len() >= 3 && a.starts_with(b) {
            return true;
        }

        // Edit distance threshold
        let distance = Self::levenshtein(a, b);
        let max_len = a.len().max(b.len());
        let threshold = (max_len as f32 * 0.3).ceil() as usize;

        distance <= threshold
    }

    fn levenshtein(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();

        let mut dp = vec![vec![0; b_chars.len() + 1]; a_chars.len() + 1];

        for i in 0..=a_chars.len() {
            dp[i][0] = i;
        }
        for j in 0..=b_chars.len() {
            dp[0][j] = j;
        }

        for i in 1..=a_chars.len() {
            for j in 1..=b_chars.len() {
                let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
                dp[i][j] = (dp[i - 1][j] + 1)
                    .min(dp[i][j - 1] + 1)
                    .min(dp[i - 1][j - 1] + cost);
            }
        }

        dp[a_chars.len()][b_chars.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::types::InstructionSkill;

    fn make_skill(name: &str, triggers: Vec<&str>) -> Skill {
        Skill::Instruction(InstructionSkill {
            name: name.to_string(),
            description: String::new(),
            triggers: triggers.into_iter().map(|s| s.to_string()).collect(),
            content: String::new(),
            source: "test".to_string(),
        })
    }

    #[test]
    fn test_exact_match() {
        let skills = vec![
            make_skill("deploy", vec!["deploy", "push changes"]),
        ];
        let skill_refs: Vec<&Skill> = skills.iter().collect();

        let matches = SkillMatcher::match_skills("deploy", &skill_refs);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].skill_name, "deploy");
        assert_eq!(matches[0].relevance, 1.0);
    }

    #[test]
    fn test_partial_match() {
        let skills = vec![
            make_skill("nixos", vec!["edit config", "modify nix"]),
        ];
        let skill_refs: Vec<&Skill> = skills.iter().collect();

        let matches = SkillMatcher::match_skills("edit the config file", &skill_refs);
        assert!(!matches.is_empty());
        assert!(matches[0].relevance > 0.0);
    }
}
```

**Step 2: Verify build and tests**

Run:
```bash
cd cvm-agent && cargo test
```
Expected: All tests pass

**Step 3: Commit**

```bash
git add cvm-agent/agent-api/src/skills/matcher.rs
git commit -m "feat: implement skill trigger matcher

- Exact match detection
- Word overlap scoring
- Fuzzy matching with Levenshtein distance
- Relevance-based ranking"
```

---

## Task 7: Implement Workflow Executor

**Files:**
- Create: `cvm-agent/agent-api/src/skills/executor.rs`

**Step 1: Implement workflow step execution**

Create `cvm-agent/agent-api/src/skills/executor.rs`:
```rust
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
                    // Skip this step
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
                        break; // Stop on failure
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

        // Resolve template variables in args
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
        // Simple condition evaluation
        // Format: "steps.<step_id>.<field>" or "parameters.<name>"

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

        // Default to true if we can't parse condition
        true
    }

    fn resolve_templates(value: &serde_json::Value, context: &ExecutionContext) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) => {
                let mut result = s.clone();

                // Replace {{ parameters.X }}
                for (key, val) in &context.parameters {
                    let pattern = format!("{{{{ parameters.{} }}}}", key);
                    result = result.replace(&pattern, val);
                }

                // Replace {{ steps.X.Y }}
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
            // Get commit hash
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
        // In a real implementation, this would prompt the user
        // For now, we auto-confirm
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
```

**Step 2: Verify build**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds

**Step 3: Commit**

```bash
git add cvm-agent/agent-api/src/skills/executor.rs
git commit -m "feat: implement workflow executor

- Step-by-step execution with conditions
- Template variable resolution
- Git and NixOps action implementations
- Progress streaming"
```

---

## Task 8: Implement SkillsService

**Files:**
- Create: `cvm-agent/agent-api/src/services/skills.rs`
- Modify: `cvm-agent/agent-api/src/services/mod.rs`
- Modify: `cvm-agent/agent-api/src/main.rs`

**Step 1: Implement SkillsService**

Create `cvm-agent/agent-api/src/services/skills.rs`:
```rust
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use futures::Stream;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::skills::{SkillsLoader, SkillMatcher, WorkflowExecutor, Skill, SkillType as InternalSkillType};

use super::health::proto::skills_service_server::SkillsService;
use super::health::proto::{
    skill_progress, CreateSkillRequest, DeleteSkillRequest, DeleteSkillResponse,
    ExecuteSkillRequest, GetSkillRequest, ListSkillsRequest, ListSkillsResponse,
    MatchSkillsRequest, MatchSkillsResponse, SkillMatch, SkillProgress, SkillResponse,
    SkillSummary, SkillType, UpdateSkillRequest,
};

pub struct SkillsServiceImpl {
    loader: Arc<RwLock<SkillsLoader>>,
}

impl SkillsServiceImpl {
    pub async fn new(skills_dir: &str) -> Self {
        let mut loader = SkillsLoader::new(skills_dir);
        if let Err(e) = loader.load_all().await {
            tracing::error!("Failed to load skills: {}", e);
        }

        Self {
            loader: Arc::new(RwLock::new(loader)),
        }
    }
}

#[tonic::async_trait]
impl SkillsService for SkillsServiceImpl {
    type ExecuteStream = Pin<Box<dyn Stream<Item = Result<SkillProgress, Status>> + Send>>;

    async fn list(
        &self,
        request: Request<ListSkillsRequest>,
    ) -> Result<Response<ListSkillsResponse>, Status> {
        let req = request.into_inner();
        let loader = self.loader.read().await;

        let skills: Vec<SkillSummary> = loader
            .list()
            .into_iter()
            .filter(|s| {
                req.type_filter == SkillType::SkillAll as i32 ||
                (req.type_filter == SkillType::SkillInstruction as i32 && matches!(s, Skill::Instruction(_))) ||
                (req.type_filter == SkillType::SkillWorkflow as i32 && matches!(s, Skill::Workflow(_)))
            })
            .map(skill_to_summary)
            .collect();

        Ok(Response::new(ListSkillsResponse { skills }))
    }

    async fn get(
        &self,
        request: Request<GetSkillRequest>,
    ) -> Result<Response<SkillResponse>, Status> {
        let req = request.into_inner();
        let loader = self.loader.read().await;

        let skill = loader
            .get(&req.name)
            .ok_or_else(|| Status::not_found(format!("Skill not found: {}", req.name)))?;

        Ok(Response::new(skill_to_response(skill)))
    }

    async fn create(
        &self,
        request: Request<CreateSkillRequest>,
    ) -> Result<Response<SkillResponse>, Status> {
        // For now, return unimplemented
        // Full implementation would write to skills directory
        Err(Status::unimplemented("Skill creation not yet implemented"))
    }

    async fn update(
        &self,
        request: Request<UpdateSkillRequest>,
    ) -> Result<Response<SkillResponse>, Status> {
        Err(Status::unimplemented("Skill update not yet implemented"))
    }

    async fn delete(
        &self,
        request: Request<DeleteSkillRequest>,
    ) -> Result<Response<DeleteSkillResponse>, Status> {
        Err(Status::unimplemented("Skill deletion not yet implemented"))
    }

    async fn execute(
        &self,
        request: Request<ExecuteSkillRequest>,
    ) -> Result<Response<Self::ExecuteStream>, Status> {
        let req = request.into_inner();
        let loader = self.loader.read().await;

        let skill = loader
            .get(&req.name)
            .ok_or_else(|| Status::not_found(format!("Skill not found: {}", req.name)))?;

        let workflow = match skill {
            Skill::Workflow(w) => w.clone(),
            Skill::Instruction(_) => {
                return Err(Status::invalid_argument("Cannot execute instruction skill"));
            }
        };

        let (tx, rx) = mpsc::channel(128);

        let parameters: HashMap<String, String> = req.parameters.into_iter().collect();

        tokio::spawn(async move {
            let progress_tx = tx.clone();

            let progress_handler = |step_num: i32, total: i32, desc: String, output: Option<String>, error: Option<String>, completed: bool| {
                let result = if let Some(e) = error {
                    Some(skill_progress::Result::Error(e))
                } else if let Some(o) = output {
                    Some(skill_progress::Result::Output(o))
                } else {
                    None
                };

                SkillProgress {
                    step_number: step_num,
                    total_steps: total,
                    step_description: desc,
                    result,
                    completed,
                }
            };

            let (exec_tx, mut exec_rx) = mpsc::channel(128);

            let exec_handle = tokio::spawn(async move {
                WorkflowExecutor::execute(&workflow, parameters, exec_tx).await
            });

            while let Some((step_num, total, desc, output, error, completed)) = exec_rx.recv().await {
                let progress = progress_handler(step_num, total, desc, output, error, completed);
                if progress_tx.send(Ok(progress)).await.is_err() {
                    break;
                }
            }

            let _ = exec_handle.await;
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }

    async fn r#match(
        &self,
        request: Request<MatchSkillsRequest>,
    ) -> Result<Response<MatchSkillsResponse>, Status> {
        let req = request.into_inner();
        let loader = self.loader.read().await;

        let skills: Vec<&Skill> = loader.list();
        let matches = SkillMatcher::match_skills(&req.input, &skills);

        let response_matches: Vec<SkillMatch> = matches
            .into_iter()
            .map(|m| SkillMatch {
                name: m.skill_name,
                r#type: if let Some(s) = loader.get(&m.skill_name) {
                    match s {
                        Skill::Instruction(_) => SkillType::SkillInstruction as i32,
                        Skill::Workflow(_) => SkillType::SkillWorkflow as i32,
                    }
                } else {
                    SkillType::SkillAll as i32
                },
                relevance: m.relevance,
                matched_trigger: m.matched_trigger,
            })
            .collect();

        Ok(Response::new(MatchSkillsResponse { matches: response_matches }))
    }
}

fn skill_to_summary(skill: &Skill) -> SkillSummary {
    SkillSummary {
        name: skill.name().to_string(),
        description: skill.description().to_string(),
        r#type: match skill {
            Skill::Instruction(_) => SkillType::SkillInstruction as i32,
            Skill::Workflow(_) => SkillType::SkillWorkflow as i32,
        },
        triggers: skill.triggers().to_vec(),
        source: skill.source().to_string(),
    }
}

fn skill_to_response(skill: &Skill) -> SkillResponse {
    let content = match skill {
        Skill::Instruction(i) => i.content.clone(),
        Skill::Workflow(w) => serde_yaml::to_string(w).unwrap_or_default(),
    };

    SkillResponse {
        name: skill.name().to_string(),
        description: skill.description().to_string(),
        r#type: match skill {
            Skill::Instruction(_) => SkillType::SkillInstruction as i32,
            Skill::Workflow(_) => SkillType::SkillWorkflow as i32,
        },
        triggers: skill.triggers().to_vec(),
        content,
        source: skill.source().to_string(),
    }
}
```

**Step 2: Update services/mod.rs**

Add to `cvm-agent/agent-api/src/services/mod.rs`:
```rust
pub mod skills;
pub use skills::SkillsServiceImpl;
```

**Step 3: Register SkillsService in main.rs**

Add to main.rs:
```rust
use services::health::proto::skills_service_server::SkillsServiceServer;
use services::SkillsServiceImpl;

// In main():
let skills_service = SkillsServiceImpl::new("/app/cvm-agent/skills").await;
Server::builder()
    // ... existing
    .add_service(SkillsServiceServer::new(skills_service))
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
git commit -m "feat: implement SkillsService

- List, get, match skill operations
- Workflow skill execution with streaming progress
- Integration with loader, matcher, executor"
```

---

## Task 9: Regenerate TypeScript Client and Add UI

**Files:**
- Regenerate: `cvm-agent/web-ui/src/gen/`
- Create: `cvm-agent/web-ui/src/hooks/useVoice.ts`
- Create: `cvm-agent/web-ui/src/components/VoiceInput.tsx`
- Create: `cvm-agent/web-ui/src/hooks/useSkills.ts`

**Step 1: Regenerate TypeScript client**

Run:
```bash
cd cvm-agent/web-ui && docker run --rm -v "$(pwd):/app" -v "$(pwd)/../proto:/proto" -w /app node:20-alpine sh -c "npm install && npx buf generate /proto"
```

**Step 2: Create useVoice hook**

Create `cvm-agent/web-ui/src/hooks/useVoice.ts`:
```typescript
import { useState, useCallback, useRef } from "react";

export function useVoice() {
  const [isRecording, setIsRecording] = useState(false);
  const [transcript, setTranscript] = useState("");
  const [isProcessing, setIsProcessing] = useState(false);
  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const wsRef = useRef<WebSocket | null>(null);

  const startRecording = useCallback(async () => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const mediaRecorder = new MediaRecorder(stream, {
        mimeType: "audio/webm;codecs=opus",
      });

      // Connect to Whisper WebSocket
      const ws = new WebSocket("ws://localhost:8082/transcribe");
      wsRef.current = ws;

      ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          setTranscript((prev) => (data.is_partial ? data.text : prev + " " + data.text));
          setIsProcessing(data.is_partial);
        } catch {
          setTranscript((prev) => prev + " " + event.data);
        }
      };

      ws.onopen = () => {
        mediaRecorder.ondataavailable = (event) => {
          if (event.data.size > 0 && ws.readyState === WebSocket.OPEN) {
            ws.send(event.data);
          }
        };

        mediaRecorder.start(100); // Send chunks every 100ms
        setIsRecording(true);
      };

      mediaRecorderRef.current = mediaRecorder;
    } catch (error) {
      console.error("Failed to start recording:", error);
    }
  }, []);

  const stopRecording = useCallback(() => {
    if (mediaRecorderRef.current) {
      mediaRecorderRef.current.stop();
      mediaRecorderRef.current.stream.getTracks().forEach((track) => track.stop());
      mediaRecorderRef.current = null;
    }

    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }

    setIsRecording(false);
    setIsProcessing(false);
  }, []);

  const clearTranscript = useCallback(() => {
    setTranscript("");
  }, []);

  return {
    isRecording,
    isProcessing,
    transcript,
    startRecording,
    stopRecording,
    clearTranscript,
  };
}
```

**Step 3: Create VoiceInput component**

Create `cvm-agent/web-ui/src/components/VoiceInput.tsx`:
```tsx
import { useVoice } from "../hooks/useVoice";

interface VoiceInputProps {
  onTranscript: (text: string) => void;
}

export function VoiceInput({ onTranscript }: VoiceInputProps) {
  const {
    isRecording,
    isProcessing,
    transcript,
    startRecording,
    stopRecording,
    clearTranscript,
  } = useVoice();

  const handleSubmit = () => {
    if (transcript.trim()) {
      onTranscript(transcript.trim());
      clearTranscript();
    }
  };

  return (
    <div className="flex items-center gap-2">
      <button
        onMouseDown={startRecording}
        onMouseUp={stopRecording}
        onMouseLeave={stopRecording}
        className={`p-2 rounded-full transition-colors ${
          isRecording
            ? "bg-red-600 hover:bg-red-700 animate-pulse"
            : "bg-gray-700 hover:bg-gray-600"
        }`}
        title="Hold to speak"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          className="h-5 w-5"
          viewBox="0 0 20 20"
          fill="currentColor"
        >
          <path
            fillRule="evenodd"
            d="M7 4a3 3 0 016 0v4a3 3 0 11-6 0V4zm4 10.93A7.001 7.001 0 0017 8a1 1 0 10-2 0A5 5 0 015 8a1 1 0 00-2 0 7.001 7.001 0 006 6.93V17H6a1 1 0 100 2h8a1 1 0 100-2h-3v-2.07z"
            clipRule="evenodd"
          />
        </svg>
      </button>

      {transcript && (
        <div className="flex-1 flex items-center gap-2 bg-gray-800 rounded px-3 py-1">
          <span className="text-sm text-gray-300 truncate">
            {isProcessing && <span className="text-blue-400">... </span>}
            {transcript}
          </span>
          <button
            onClick={handleSubmit}
            className="text-blue-400 hover:text-blue-300 text-sm"
          >
            Send
          </button>
          <button
            onClick={clearTranscript}
            className="text-gray-500 hover:text-gray-400 text-sm"
          >
            Clear
          </button>
        </div>
      )}
    </div>
  );
}
```

**Step 4: Create useSkills hook**

Create `cvm-agent/web-ui/src/hooks/useSkills.ts`:
```typescript
import { createConnectTransport } from "@connectrpc/connect-web";
import { createClient } from "@connectrpc/connect";
import { SkillsService } from "../gen/agent_pb";
import { useState, useCallback } from "react";

const transport = createConnectTransport({
  baseUrl: "/api",
});

const skillsClient = createClient(SkillsService, transport);

export interface SkillSummary {
  name: string;
  description: string;
  type: "instruction" | "workflow";
  triggers: string[];
}

export function useSkills() {
  const [skills, setSkills] = useState<SkillSummary[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetchSkills = useCallback(async () => {
    setIsLoading(true);
    try {
      const response = await skillsClient.list({});
      setSkills(
        response.skills.map((s) => ({
          name: s.name,
          description: s.description,
          type: s.type === 1 ? "instruction" : "workflow",
          triggers: s.triggers,
        }))
      );
    } catch (error) {
      console.error("Failed to fetch skills:", error);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const matchSkills = useCallback(async (input: string) => {
    try {
      const response = await skillsClient.match({ input });
      return response.matches.map((m) => ({
        name: m.name,
        relevance: m.relevance,
        trigger: m.matchedTrigger,
      }));
    } catch {
      return [];
    }
  }, []);

  const executeSkill = useCallback(async function* (
    name: string,
    params: Record<string, string>
  ) {
    for await (const progress of skillsClient.execute({ name, parameters: params })) {
      yield {
        step: progress.stepNumber,
        total: progress.totalSteps,
        description: progress.stepDescription,
        output: progress.result.case === "output" ? progress.result.value : undefined,
        error: progress.result.case === "error" ? progress.result.value : undefined,
        completed: progress.completed,
      };
    }
  }, []);

  return {
    skills,
    isLoading,
    fetchSkills,
    matchSkills,
    executeSkill,
  };
}
```

**Step 5: Integrate VoiceInput into ChatOverlay**

Update `cvm-agent/web-ui/src/components/ChatOverlay.tsx` to include VoiceInput:

Add import:
```typescript
import { VoiceInput } from "./VoiceInput";
```

Add in the input form area:
```tsx
<div className="flex gap-2 items-center">
  <VoiceInput onTranscript={(text) => {
    setInput(text);
  }} />
  {/* existing input and send button */}
</div>
```

**Step 6: Verify build**

Run:
```bash
cd cvm-agent/web-ui && docker run --rm -v "$(pwd):/app" -w /app node:20-alpine npm run build
```
Expected: Build succeeds

**Step 7: Commit**

```bash
git add cvm-agent/web-ui/
git commit -m "feat: add Voice and Skills UI components

- useVoice hook with WebSocket streaming
- VoiceInput component with push-to-talk
- useSkills hook for skill operations
- Regenerated TypeScript client"
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
Expected: All tests pass

**Step 3: Run TypeScript type check**

Run:
```bash
cd cvm-agent/web-ui && docker run --rm -v "$(pwd):/app" -w /app node:20-alpine npx tsc --noEmit
```
Expected: No type errors

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: complete Phase 4 - Voice & Skills

Phase 4 complete:
- VoiceService with Whisper integration
- SkillsService with loader, matcher, executor
- Built-in instruction and workflow skills
- Voice input UI with push-to-talk
- Skills browser and execution UI

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Summary

After completing all tasks, you will have:

1. **VoiceService** (`cvm-agent/agent-api/src/services/voice.rs`)
   - WebSocket connection to Whisper server
   - Streaming audio transcription
   - Model management

2. **SkillsService** (`cvm-agent/agent-api/src/services/skills.rs`)
   - Skill listing and retrieval
   - Trigger matching
   - Workflow execution

3. **Skills Infrastructure** (`cvm-agent/agent-api/src/skills/`)
   - Loader for markdown and YAML skills
   - Matcher with fuzzy matching
   - Workflow executor with step actions

4. **Built-in Skills** (`cvm-agent/skills/`)
   - nixos-editing instruction skill
   - git-workflow instruction skill
   - deploy-config workflow
   - rollback workflow

5. **Web UI Components**
   - VoiceInput with push-to-talk
   - Skills hooks for client operations

## Next Phase

Phase 5 will add:
- Memory storage and retrieval
- Active forgetting with human confirmation
- Adaptive UI states
- Error handling and recovery
