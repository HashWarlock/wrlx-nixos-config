# CVM Agent

AI-powered system administration agent for NixOS Confidential VMs.

## Overview

The CVM Agent provides a conversational interface for managing NixOS systems running in Phala Cloud's Confidential VMs. It combines LLM capabilities with system automation to enable natural language system administration.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Web UI (React)                         │
│                        Port 3000                            │
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
│                        Port 8080                            │
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
│  │                 Core Modules                           │ │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐     │ │
│  │  │ Config  │ │ Context │ │  Skill  │ │   LLM   │     │ │
│  │  │ ::get() │ │ (DI)    │ │Registry │ │ Client  │     │ │
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
│   │   ├── config.rs       # Centralized configuration (Config::get())
│   │   ├── context.rs      # Service context for dependency injection
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
│   │   │   └── memory.rs   # Memory service
│   │   └── skills/         # Skills engine
│   │       ├── registry.rs # SkillAction trait & registry
│   │       ├── actions/    # Built-in skill actions
│   │       │   ├── mod.rs      # Action registration
│   │       │   ├── prelude.rs  # Common imports
│   │       │   ├── git.rs      # Git operations
│   │       │   ├── nixops.rs   # NixOS operations
│   │       │   ├── prompt.rs   # Chat/prompt actions
│   │       │   └── template.rs.example  # New action template
│   │       ├── loader.rs   # Load skills from disk
│   │       ├── matcher.rs  # Trigger matching
│   │       ├── executor.rs # Workflow execution
│   │       └── types.rs    # Skill types
│   ├── Cargo.toml
│   ├── build.rs            # Proto compilation
│   └── SKILLS.md           # Skill development guide
├── web-ui/                 # React frontend (port 3000)
│   ├── src/
│   │   ├── App.tsx         # Main application
│   │   ├── components/     # UI components
│   │   └── hooks/          # React hooks
│   ├── package.json
│   └── buf.gen.yaml        # Proto generation config
├── proto/                  # Protocol definitions
│   ├── agent.proto         # All service definitions
│   └── buf.yaml            # Buf configuration
└── skills/                 # Built-in skill workflows (YAML)
    ├── nixos-rebuild.yaml
    └── system-update.yaml
```

## Development

### Prerequisites

Using Nix (recommended):
```bash
cd cvm-agent/agent-api
nix develop .#cvm
```

Or manually:
- Rust 1.75+
- Node.js 20+
- Protocol Buffers compiler

### Build

```bash
# Build Rust server
cargo build

# Build release
cargo build --release

# Build web UI
cd ../web-ui
npm install
npm run build
```

### Run Tests

```bash
# All Rust tests (use single thread to avoid env var race conditions)
cargo test -- --test-threads=1

# Specific test
cargo test test_memory_lifecycle

# With output
cargo test -- --nocapture
```

### Development Server

```bash
# Terminal 1: Run API server
RUST_LOG=debug cargo run

# Terminal 2: Run web UI dev server
cd ../web-ui
npm run dev
```

### Adding New Skill Actions

New actions can be added in ~30 seconds:

```bash
# 1. Copy template
cp src/skills/actions/template.rs.example src/skills/actions/myaction.rs

# 2. Edit: rename struct, change action name, implement execute()

# 3. Register in src/skills/actions/mod.rs:
#    - Add: mod myaction;
#    - Add: pub use myaction::MyAction;
#    - In register_all(): registry.register(MyAction);

# 4. Build
cargo build
```

See `SKILLS.md` for detailed documentation.

### Regenerate Proto Types

```bash
# TypeScript client
cd ../web-ui
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
cargo test -- --test-threads=1

# Test summary:
# - 66 tests passing
# - 10 ignored (require display for GUI tests)
```

### Integration Tests

```bash
# Memory lifecycle test
cargo test test_memory_lifecycle_integration

# Cross-layer search test
cargo test test_cross_layer_search_integration
```

## Deployment

See `../files/phala-cvm/README.md` for full deployment documentation.

### Quick Start (Local Docker)

```bash
cd ../files/phala-cvm

# Configure environment
cp .env.example .env
# Edit .env with your values (REDPILL_API_KEY, VNC_PASSWORD, etc.)

# Start all services
docker-compose up -d

# View logs
docker-compose logs -f
```

### Access Points

| Service | URL | Description |
|---------|-----|-------------|
| **Agent UI** | http://localhost:3000 | Web interface |
| **Agent API** | localhost:8080 | gRPC endpoint |
| **noVNC** | http://localhost:6080/vnc.html | Desktop access |
| **SSH** | `ssh -p 2222 confidant@localhost` | Terminal |
| **Whisper** | localhost:8082 | Voice API |

### Phala Cloud Deployment

```bash
cd ../files/phala-cvm

# Deploy to Phala Cloud
phala cvms create \
  --name nixos-cvm-agent \
  --compose docker-compose.yml \
  --teepod-id <your-teepod-id> \
  -e .env
```

## License

MIT
