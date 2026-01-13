# CVM Agent Design Document

**Date:** 2026-01-08
**Status:** Draft
**Author:** HashWarlock + Claude

## Overview

CVM Agent is a browser-based intelligent assistant for controlling a Phala CVM NixOS environment through natural language (text or voice). It provides full system control including NixOS configuration management, GUI automation, git operations, and extensible skills - all accessible through a transparent overlay on the noVNC desktop view.

## Goals

1. Control the entire CVM from a chat interface (text or voice)
2. Edit NixOS configuration live with intelligent risk classification
3. Manage system generations and git history for robust rollback
4. Automate GUI interactions across all desktop applications
5. Provide extensible skills system for reusable workflows
6. Maintain context-aware memory with human-confirmed forgetting

## Non-Goals

- Native desktop client (browser-only approach chosen for accessibility)
- Local model inference in CVM (using Redpill API for confidential LLM access)
- Real-time collaboration (single-user system)

---

## Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                      Browser (any device)                        │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              CVM Web Interface (https://cvm-url)         │    │
│  │  ┌─────────────────────────────────────────────────┐    │    │
│  │  │                 noVNC Canvas                     │    │    │
│  │  │              (XFCE Desktop View)                 │    │    │
│  │  │                                                  │    │    │
│  │  │    ┌─────────────────────────────────┐          │    │    │
│  │  │    │     Agent Overlay (transparent) │          │    │    │
│  │  │    │     cmd+shift+space or button   │          │    │    │
│  │  │    └─────────────────────────────────┘          │    │    │
│  │  └─────────────────────────────────────────────────┘    │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
                              │ gRPC-web / WebSocket
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Phala CVM (NixOS Container)                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │
│  │  Agent API   │  │    noVNC     │  │   Whisper    │           │
│  │   (Rust)     │  │   Server     │  │   Server     │           │
│  └──────────────┘  └──────────────┘  └──────────────┘           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │
│  │    XFCE      │  │   NixOS      │  │     Git      │           │
│  │   Desktop    │  │   System     │  │    Repo      │           │
│  └──────────────┘  └──────────────┘  └──────────────┘           │
└─────────────────────────────────────────────────────────────────┘
```

### Key Architectural Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Client location | Browser-based | No install needed, access from anywhere, overlay on noVNC |
| Communication | gRPC | Strong typing, native streaming, scales with complexity |
| Voice processing | Whisper in CVM | Privacy (no cloud STT), works within TEE context |
| LLM provider | Redpill API | Confidential model access, Anthropic-compatible format |
| GUI perception | AT-SPI + Vision fallback | Fast structured data when available, vision for edge cases |
| Config editing | Hybrid (direct + approval) | Fast for simple changes, safe for complex ones |
| Rollback | Dual tracking (generations + git) | Quick undo via generations, targeted rollback via git |
| Skills | Dual system (instructions + workflows) | Flexible guidance + deterministic critical operations |
| Memory | Hybrid with active forgetting | Context-aware, human-confirmed cleanup |
| Authentication | API key (TEE attestation later) | Simple start, upgrade path to full TEE auth |

---

## Components

### 1. Agent API (Rust)

The core service running in the CVM, built with Axum and Tonic for gRPC.

#### Services

| Service | Responsibility |
|---------|----------------|
| **ChatService** | Main orchestrator - receives input, reasons via Redpill, dispatches to services |
| **NixOpsService** | Edit configs, `nixos-rebuild`, rollback generations, manage flake inputs |
| **GitOpsService** | Commit, push, pull, branch management, diff viewing |
| **GUIService** | AT-SPI element queries, xdotool actions, screenshot capture, vision analysis |
| **ShellService** | Execute commands with streaming output |
| **FileOpsService** | Read, write, list, search files in the config repo |
| **SkillsService** | CRUD for skills, skill execution |
| **MemoryService** | Store/retrieve history, structured facts, forgetting management |
| **VoiceService** | Audio transcription via Whisper |

#### Risk Classification for NixOS Edits

```
Low Risk (direct edit):
  - Add/remove packages
  - Toggle simple options (services.*.enable)
  - Environment variables
  - User-level home-manager changes

High Risk (requires approval):
  - Module structure changes
  - Bootloader/filesystem changes
  - Network configuration
  - New module imports
  - Security-related options
```

### 2. Web UI (TypeScript/React)

Browser-based overlay served alongside noVNC.

#### Adaptive States

**State 1: Hidden (default)**
- noVNC desktop at full view
- Floating action button in corner
- Trigger: `cmd+shift+space` OR click FAB

**State 2: Quick Input (minimal)**
- Small input bar overlaid on desktop
- For quick commands, simple questions
- Escape or click-outside to dismiss

**State 3: Expanded Panel**
- Side panel with conversation history
- Diff previews for config changes
- Approval buttons for risky operations
- Streaming output display
- Auto-expands for: approvals, multi-step ops, errors, long conversations

### 3. Voice Input

Browser microphone → WebSocket → Whisper server → text

#### Activation Options
1. **Push-to-talk**: Hold microphone button while speaking
2. **Toggle mode**: Click to start/stop listening
3. **Wake word** (optional): "Hey CVM" activates

#### UI Feedback
- Live transcription while speaking
- Editable final transcription before sending
- Visual recording indicator

### 4. Memory System

#### Memory Layers

| Layer | Contents | TTL |
|-------|----------|-----|
| Working Context | Current conversation | Session duration |
| Conversation Archive | Summarized past conversations | 30 days without reference → prompt for deletion |
| System Facts | NixOS generation, packages, git state | Refreshed on change |
| Learned Preferences | User patterns and preferences | Decays if contradicted |

#### Active Forgetting

| Trigger | Action |
|---------|--------|
| NixOS rebuild succeeds | Purge previous generation's package list |
| Git commit/push | Archive pre-commit state |
| Error resolved | Move error context to archive, summarize |
| 30 days no reference | Prompt user to confirm deletion |
| Preference contradicted 3x | Remove learned preference |
| System fact stale | Re-query and replace |

#### Human-Confirmed Forgetting

- **Auto-forget**: Stale system facts, expired session context, duplicates
- **Confirm before forgetting**: Learned preferences, conversation summaries, resolution patterns
- **Never auto-forget**: User-pinned memories, active skills, bookmarked conversations

Forgetting sweep presents candidates in a UI for user decision, and learns from keep/forget patterns.

### 5. Skills System

Dual-skill architecture supporting both instruction-based and workflow-based skills.

#### Instruction Skills (Claude-style)

Natural language guidance documents loaded into LLM context.

```markdown
# skills/instructions/nixos-editing.md
---
name: nixos-editing
description: Guidelines for editing NixOS configuration
triggers:
  - edit config
  - modify nix
  - change system
---

When editing NixOS configuration files:

1. Always read the file first to understand current structure
2. Prefer modifying existing modules over creating new ones
3. Use `pkgs.unstable.*` for bleeding-edge packages
4. Follow the repository's pattern of separating:
   - System config in `modules/`
   - User config in `home/modules/`
5. After editing, explain what changed and why
```

#### Workflow Skills (Deterministic)

Step-by-step procedures for critical operations.

```yaml
# skills/workflows/deploy-config.yaml
name: deploy-config
description: Rebuild NixOS and push config to GitHub
triggers:
  - deploy
  - push my changes

steps:
  - action: git.status
  - action: prompt.confirm_if
    condition: has_uncommitted_changes
    message: "Commit changes first?"
  - action: git.commit
  - action: nixos.rebuild
    args: [switch]
    stream: true
  - action: git.push
```

#### Skill Resolution Priority

1. **Workflow skills** - checked first for exact trigger match
2. **Instruction skills** - loaded into context for relevant domains
3. **Natural language** - LLM reasons freely if no skills apply

#### Skill Sources

- **Built-in**: Core skills shipped with agent
- **User-created**: Created via chat ("Save this as a skill")
- **Imported**: Load from git repos, share between CVMs

---

## gRPC API Definition

```protobuf
syntax = "proto3";
package cvm.agent;

// Main chat service - orchestrates everything
service ChatService {
  rpc SendMessage(ChatRequest) returns (stream ChatResponse);
  rpc GetHistory(HistoryRequest) returns (HistoryResponse);
}

// NixOS operations
service NixOpsService {
  rpc Rebuild(RebuildRequest) returns (stream RebuildProgress);
  rpc Rollback(RollbackRequest) returns (RollbackResponse);
  rpc ListGenerations(Empty) returns (GenerationsResponse);
  rpc EditConfig(EditRequest) returns (EditResponse);
  rpc PreviewDiff(DiffRequest) returns (DiffResponse);
}

// Git operations
service GitOpsService {
  rpc Status(Empty) returns (GitStatusResponse);
  rpc Commit(CommitRequest) returns (CommitResponse);
  rpc Push(PushRequest) returns (PushResponse);
  rpc Log(LogRequest) returns (LogResponse);
}

// GUI automation
service GUIService {
  rpc Screenshot(ScreenshotRequest) returns (ScreenshotResponse);
  rpc GetElements(ElementQuery) returns (ElementsResponse);
  rpc Click(ClickRequest) returns (ActionResponse);
  rpc Type(TypeRequest) returns (ActionResponse);
  rpc Describe(DescribeRequest) returns (VisionResponse);
}

// Shell execution
service ShellService {
  rpc Execute(ShellRequest) returns (stream ShellOutput);
}

// Skills management
service SkillsService {
  rpc List(Empty) returns (SkillsListResponse);
  rpc Get(SkillRequest) returns (SkillResponse);
  rpc Create(CreateSkillRequest) returns (SkillResponse);
  rpc Execute(ExecuteSkillRequest) returns (stream SkillProgress);
}

// Memory management
service MemoryService {
  rpc Query(MemoryQuery) returns (MemoryResponse);
  rpc GetForgetCandidates(Empty) returns (ForgetCandidatesResponse);
  rpc ConfirmForget(ForgetDecision) returns (Empty);
  rpc Pin(PinRequest) returns (Empty);
}

// Voice transcription
service VoiceService {
  rpc Transcribe(stream AudioChunk) returns (stream TranscriptChunk);
}
```

---

## Project Structure

```
wrlx-nixos-config/
├── flake.nix                          # Add agent packages
├── hosts/phala-cvm/
│   └── default.nix                    # Add agent service
├── modules/
│   └── cvm-agent.nix                  # NixOS module for agent
│
├── cvm-agent/                         # Agent monorepo
│   ├── Cargo.toml                     # Rust workspace
│   │
│   ├── proto/                         # gRPC definitions
│   │   ├── agent.proto
│   │   └── buf.yaml
│   │
│   ├── agent-api/                     # Rust API server
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── services/
│   │       │   ├── chat.rs
│   │       │   ├── nixops.rs
│   │       │   ├── gitops.rs
│   │       │   ├── gui.rs
│   │       │   ├── shell.rs
│   │       │   ├── skills.rs
│   │       │   ├── memory.rs
│   │       │   └── voice.rs
│   │       └── llm/
│   │           └── redpill.rs
│   │
│   ├── web-ui/                        # Browser overlay
│   │   ├── package.json
│   │   ├── src/
│   │   │   ├── App.tsx
│   │   │   ├── components/
│   │   │   │   ├── Overlay.tsx
│   │   │   │   ├── ChatPanel.tsx
│   │   │   │   ├── VoiceInput.tsx
│   │   │   │   └── DiffViewer.tsx
│   │   │   ├── grpc/
│   │   │   └── hooks/
│   │   │       ├── useVoice.ts
│   │   │       └── useAgent.ts
│   │   └── vite.config.ts
│   │
│   └── skills/                        # Built-in skills
│       ├── instructions/
│       │   ├── nixos-editing.md
│       │   └── git-workflow.md
│       └── workflows/
│           ├── deploy-config.yaml
│           ├── rollback.yaml
│           └── morning-setup.yaml
│
├── files/phala-cvm/
│   ├── docker-compose.yml             # Updated with agent ports
│   └── entrypoint.sh                  # Start agent services
```

---

## Deployment

### Port Mapping

```yaml
# docker-compose.yml
services:
  cvm:
    ports:
      - "2222:22"      # SSH (existing)
      - "6080:6080"    # noVNC (existing)
      - "5900:5900"    # VNC direct (existing, optional)
      - "8080:8080"    # Agent API (gRPC-web)
      - "8081:8081"    # Web UI
      - "8082:8082"    # Whisper WebSocket
```

### Unified Access (nginx)

```
https://<cvm-url>/
├── /                  → noVNC (desktop view)
├── /agent/            → Web UI overlay (static files)
├── /api/              → gRPC-web proxy to Agent API
└── /voice/            → WebSocket proxy to Whisper server
```

### Environment Variables

```bash
# LLM Configuration (Redpill)
REDPILL_API_KEY=                       # Required: Redpill API key
REDPILL_BASE_URL=https://api.redpill.ai
REDPILL_MODEL=z-ai/glm-4.6

# Anthropic-compatible (optional, overrides Redpill if set)
ANTHROPIC_AUTH_TOKEN=                  # Optional: Anthropic or Redpill key
ANTHROPIC_BASE_URL=                    # Optional: API base URL
ANTHROPIC_MODEL=                       # Optional: model override
ANTHROPIC_DEFAULT_HAIKU_MODEL=         # Optional: for lighter tasks
ANTHROPIC_DEFAULT_SONNET_MODEL=        # Optional: for standard tasks
ANTHROPIC_DEFAULT_OPUS_MODEL=          # Optional: for complex reasoning

# Voice
WHISPER_MODEL=base                     # small|base|medium

# Agent
AGENT_LOG_LEVEL=info                   # debug|info|warn|error
MEMORY_RETENTION_DAYS=30               # Before forgetting prompt

# Existing CVM variables
VNC_PASSWORD=...
VNC_RESOLUTION=1920x1080
```

### NixOS Module

```nix
# modules/cvm-agent.nix
{ config, pkgs, ... }:
{
  systemd.services.cvm-agent = {
    description = "CVM Agent API";
    wantedBy = [ "multi-user.target" ];
    after = [ "network.target" ];
    serviceConfig = {
      ExecStart = "${pkgs.cvm-agent}/bin/agent-api";
      Restart = "always";
      EnvironmentFile = "/etc/cvm-agent/env";
    };
  };

  systemd.services.whisper-server = {
    description = "Whisper Transcription Server";
    wantedBy = [ "multi-user.target" ];
    serviceConfig = {
      ExecStart = "${pkgs.whisper-cpp}/bin/whisper-server --model ${config.services.cvm-agent.whisperModel}";
      Restart = "always";
    };
  };

  services.nginx = {
    enable = true;
    virtualHosts."cvm" = {
      locations."/" = {
        proxyPass = "http://127.0.0.1:6080";  # noVNC
        proxyWebsocket = true;
      };
      locations."/agent/" = {
        alias = "${pkgs.cvm-agent-web}/share/";
      };
      locations."/api/" = {
        proxyPass = "http://127.0.0.1:8080";
        proxyWebsocket = true;
      };
      locations."/voice/" = {
        proxyPass = "http://127.0.0.1:8082";
        proxyWebsocket = true;
      };
    };
  };
}
```

---

## Security Considerations

### Current (Phase 1)
- API key authentication between browser and Agent API
- All traffic over HTTPS (handled by Phala gateway)
- Redpill API key stored in CVM environment

### Future (Phase 2)
- TEE attestation-based authentication
- Client verifies CVM attestation before connecting
- End-to-end encryption within TEE boundary

### Risk Mitigations
- High-risk NixOS changes require user approval
- Git commits before destructive operations
- Generation rollback always available
- Memory forgetting requires human confirmation for important data

---

## Implementation Phases

### Phase 1: Core Infrastructure
- [ ] Set up Rust workspace and proto definitions
- [ ] Implement basic ChatService with Redpill integration
- [ ] Implement ShellService with streaming
- [ ] Basic web UI with input and response display
- [ ] Hotkey activation (cmd+shift+space in browser)

### Phase 2: NixOS Operations
- [ ] NixOpsService: rebuild, rollback, list generations
- [ ] GitOpsService: status, commit, push
- [ ] Risk classification for edits
- [ ] Diff preview and approval UI

### Phase 3: GUI Automation
- [ ] AT-SPI integration for element discovery
- [ ] xdotool wrapper for click/type actions
- [ ] Screenshot capture
- [ ] Vision fallback via Redpill

### Phase 4: Voice & Skills
- [ ] Whisper server integration
- [ ] Voice input UI with live transcription
- [ ] Instruction skills loader
- [ ] Workflow skills executor
- [ ] Skill creation via chat

### Phase 5: Memory & Polish
- [ ] Memory storage and retrieval
- [ ] Active forgetting with human confirmation
- [ ] Adaptive UI states
- [ ] Error handling and recovery

---

## Open Questions

1. **Whisper model size**: Base model balances speed/accuracy, but should we allow per-session override for accuracy-critical tasks?

2. **Skill sharing**: Should skills be shareable between CVMs? If so, what's the trust model for imported skills?

3. **Multi-CVM**: Future consideration - could one browser session control multiple CVMs?

---

## References

- [nixosandbox](https://github.com/HashWarlock/nixosandbox) - REST API patterns, browser automation, skills system
- [NixOS Flakes Best Practices](https://nixos-and-flakes.thiscute.world/best-practices/intro)
- [Phala Cloud CVM Documentation](https://docs.phala.network/)
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) - Local speech recognition
- [Tonic gRPC](https://github.com/hyperium/tonic) - Rust gRPC implementation
- [gRPC-web](https://github.com/grpc/grpc-web) - Browser gRPC client
