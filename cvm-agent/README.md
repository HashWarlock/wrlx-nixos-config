# CVM Agent

AI-powered system administration agent for NixOS Confidential VMs.

## Overview

The CVM Agent provides a conversational interface for managing NixOS systems running in Phala Cloud's Confidential VMs. It combines LLM capabilities with system automation to enable natural language system administration.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Web UI (React)                         │
│                                                             │
│  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐  │
│  │   Chat    │ │   Diff    │ │  Memory   │ │    GUI    │  │
│  │  Overlay  │ │  Preview  │ │   Panel   │ │ Inspector │  │
│  └─────┬─────┘ └─────┬─────┘ └─────┬─────┘ └─────┬─────┘  │
│        └─────────────┴─────────────┴─────────────┘         │
│                          │ gRPC-web (connect-rpc)          │
└──────────────────────────┼─────────────────────────────────┘
                           │
┌──────────────────────────┼─────────────────────────────────┐
│                    Agent API (Rust/Tonic)                   │
│                          │                                  │
│  ┌───────────────────────┴───────────────────────────────┐ │
│  │                    gRPC Services                       │ │
│  │  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐        │ │
│  │  │Health│ │ Chat │ │Shell │ │NixOps│ │GitOps│        │ │
│  │  └──────┘ └──────┘ └──────┘ └──────┘ └──────┘        │ │
│  │  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐                 │ │
│  │  │ GUI  │ │Voice │ │Skills│ │Memory│                 │ │
│  │  └──────┘ └──────┘ └──────┘ └──────┘                 │ │
│  └───────────────────────────────────────────────────────┘ │
│                          │                                  │
│  ┌───────────────────────┴───────────────────────────────┐ │
│  │                 Support Modules                        │ │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐     │ │
│  │  │   LLM   │ │  Risk   │ │Forgetting│ │Recovery │     │ │
│  │  │ Client  │ │Classify │ │ Manager │ │ Actions │     │ │
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘     │ │
│  └───────────────────────────────────────────────────────┘ │
│                          │                                  │
│  ┌───────────────────────┴───────────────────────────────┐ │
│  │                   SQLite Database                      │ │
│  │              (Memory + FTS5 Search)                    │ │
│  └───────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Services

### Core Services

| Service | Description |
|---------|-------------|
| **HealthService** | Health check endpoint |
| **ChatService** | LLM-powered conversation via Redpill API |
| **ShellService** | Command execution with streaming output |

### NixOS Operations

| Service | Description |
|---------|-------------|
| **NixOpsService** | Rebuild, rollback, list/switch generations |
| **GitOpsService** | Status, diff, commit, push with risk classification |

### Automation

| Service | Description |
|---------|-------------|
| **GUIService** | Screenshot, AT-SPI elements, xdotool actions, vision |
| **VoiceService** | Whisper-based voice transcription |
| **SkillsService** | Workflow automation with trigger matching |

### Memory

| Service | Description |
|---------|-------------|
| **MemoryService** | 4-layer memory (working, archive, facts, preferences) |

## Directory Structure

```
cvm-agent/
├── agent-api/              # Rust gRPC server
│   ├── src/
│   │   ├── main.rs         # Entry point, service registration
│   │   ├── db/             # SQLite database layer
│   │   │   ├── mod.rs      # Connection pool
│   │   │   ├── schema.rs   # MemoryRecord struct
│   │   │   ├── repository.rs # CRUD operations
│   │   │   └── migrations.rs # Schema setup
│   │   ├── llm/            # LLM client
│   │   │   └── mod.rs      # Redpill API client
│   │   ├── risk/           # Risk classification
│   │   │   └── mod.rs      # NixOS change risk analysis
│   │   ├── services/       # gRPC service implementations
│   │   │   ├── health.rs   # Health check
│   │   │   ├── chat.rs     # Chat with LLM
│   │   │   ├── shell.rs    # Command execution
│   │   │   ├── nixops.rs   # NixOS operations
│   │   │   ├── gitops.rs   # Git operations
│   │   │   ├── gui/        # GUI automation
│   │   │   ├── voice.rs    # Voice transcription
│   │   │   ├── skills.rs   # Skills management
│   │   │   ├── memory.rs   # Memory service
│   │   │   ├── forgetting.rs # Active forgetting
│   │   │   └── recovery.rs # Error recovery
│   │   └── skills/         # Skills engine
│   │       ├── loader.rs   # Load skills from disk
│   │       ├── matcher.rs  # Trigger matching
│   │       ├── executor.rs # Workflow execution
│   │       └── types.rs    # Skill types
│   ├── Cargo.toml
│   └── build.rs            # Proto compilation
├── web-ui/                 # React frontend
│   ├── src/
│   │   ├── App.tsx         # Main application
│   │   ├── components/     # UI components
│   │   │   ├── ChatOverlay.tsx
│   │   │   ├── DiffPreview.tsx
│   │   │   ├── GenerationsList.tsx
│   │   │   ├── GUIInspector.tsx
│   │   │   ├── MemoryPanel.tsx
│   │   │   ├── ForgetConfirmDialog.tsx
│   │   │   └── VoiceInput.tsx
│   │   ├── hooks/          # React hooks
│   │   │   ├── useAgent.ts
│   │   │   ├── useNixOps.ts
│   │   │   ├── useGitOps.ts
│   │   │   ├── useGUI.ts
│   │   │   ├── useVoice.ts
│   │   │   ├── useSkills.ts
│   │   │   ├── useMemory.ts
│   │   │   └── useOverlayState.ts
│   │   └── gen/            # Generated proto types
│   │       └── agent_pb.ts
│   ├── package.json
│   └── buf.gen.yaml        # Proto generation config
├── proto/                  # Protocol definitions
│   ├── agent.proto         # All service definitions
│   └── buf.yaml            # Buf configuration
└── skills/                 # Built-in skills
    ├── nixos-rebuild.yaml
    ├── system-update.yaml
    └── ...
```

## Development

### Prerequisites

- Rust 1.75+
- Node.js 20+
- Protocol Buffers compiler

### Build

```bash
# Build Rust server
cargo build -p agent-api

# Build release
cargo build -p agent-api --release

# Build web UI
cd web-ui
npm install
npm run build
```

### Run Tests

```bash
# All Rust tests
cargo test -p agent-api

# Specific test
cargo test -p agent-api test_memory_lifecycle

# With output
cargo test -p agent-api -- --nocapture
```

### Development Server

```bash
# Terminal 1: Run API server
RUST_LOG=debug cargo run -p agent-api

# Terminal 2: Run web UI dev server
cd web-ui
npm run dev
```

### Regenerate Proto Types

```bash
# TypeScript client
cd web-ui
npx buf generate ../proto
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level (trace/debug/info/warn/error) |
| `AGENT_DB_PATH` | `./data/agent.db` | SQLite database path |
| `REDPILL_API_KEY` | - | Redpill API key for LLM |
| `REDPILL_BASE_URL` | `https://api.redpill.ai` | LLM API endpoint |
| `REDPILL_MODEL` | `gpt-4` | LLM model to use |
| `WHISPER_MODEL` | `base` | Whisper model size |

### Memory Layers

| Layer | Purpose | Auto-Forget |
|-------|---------|-------------|
| **Working** | Current session context | Yes (when expired) |
| **Archive** | Summarized past sessions | No (needs confirmation) |
| **Facts** | System state facts | Yes (when stale) |
| **Preferences** | Learned user preferences | No (needs confirmation) |

### Risk Classification

NixOS configuration changes are classified by risk:

| Level | Examples |
|-------|----------|
| **Low** | Adding packages, updating comments |
| **Medium** | Service configuration changes |
| **High** | Boot loader, filesystem changes |
| **Critical** | Security settings, SSH config |

## API Examples

### Health Check

```bash
grpcurl -plaintext localhost:8080 cvm.agent.HealthService/Check
```

### Execute Command

```bash
grpcurl -plaintext -d '{"command": "uname -a"}' \
  localhost:8080 cvm.agent.ShellService/Execute
```

### Query Memory

```bash
grpcurl -plaintext -d '{"layers": [1, 3], "limit": 10}' \
  localhost:8080 cvm.agent.MemoryService/Query
```

## Testing

### Unit Tests

```bash
# Run all tests
cargo test -p agent-api

# Test summary:
# - 73 tests passing
# - 10 ignored (require display for GUI tests)
```

### Integration Tests

```bash
# Memory lifecycle test
cargo test -p agent-api test_memory_lifecycle_integration

# Cross-layer search test
cargo test -p agent-api test_cross_layer_search_integration
```

## License

MIT
