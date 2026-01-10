# Phala Cloud Confidential VM - NixOS Configuration

This directory contains the configuration for deploying a NixOS-based Confidential VM on Phala Cloud with GUI support via VNC/noVNC and an AI-powered CVM Agent.

## Features

- **Lightweight GUI**: XFCE desktop environment
- **CVM Agent**: AI-powered system administration interface
- **Remote Access**:
  - SSH on port 2222
  - VNC on port 5900
  - Web-based noVNC on port 6080
  - Agent Web UI on port 8081
  - Agent gRPC API on port 8080
- **Confidential Computing**: Runs on Phala Cloud's TEE infrastructure
- **User**: `confidant` with SSH key authentication

## Quick Start

### 1. Configure Environment

```bash
cd files/phala-cvm
cp .env.example .env
```

Edit `.env` with your values:

```bash
# Required
GITHUB_REPO=https://github.com/YOUR-USERNAME/wrlx-nixos-config.git
GIT_COMMIT_HASH=<your-commit-hash>
VNC_PASSWORD=your-secure-password

# CVM Agent (optional but recommended)
REDPILL_API_KEY=your-redpill-api-key
REDPILL_BASE_URL=https://api.redpill.ai
REDPILL_MODEL=gpt-4

# Voice transcription (optional)
WHISPER_MODEL=base
```

### 2. Start the CVM

```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f
```

### 3. Access Points

| Service | URL | Description |
|---------|-----|-------------|
| **noVNC** | http://localhost:6080/vnc.html | Web-based desktop access |
| **Agent UI** | http://localhost:8081 | AI agent web interface |
| **VNC** | vnc://localhost:5900 | Direct VNC connection |
| **SSH** | `ssh -p 2222 confidant@localhost` | Terminal access |
| **Agent API** | localhost:8080 | gRPC API endpoint |

## CVM Agent

The CVM Agent provides an AI-powered interface for managing the NixOS system.

### Services

| Service | Port | Description |
|---------|------|-------------|
| **ChatService** | 8080 | LLM-powered conversation |
| **ShellService** | 8080 | Command execution with streaming |
| **NixOpsService** | 8080 | Rebuild, rollback, list generations |
| **GitOpsService** | 8080 | Git operations with risk classification |
| **GUIService** | 8080 | Screenshot, element discovery, automation |
| **VoiceService** | 8080 | Whisper voice transcription |
| **SkillsService** | 8080 | Workflow automation |
| **MemoryService** | 8080 | 4-layer memory with active forgetting |

### Using the Agent

**Via Web UI (http://localhost:8081):**
- Chat interface with the AI agent
- View and approve NixOS configuration changes
- Browse system generations
- Manage agent memory

**Via gRPC (localhost:8080):**
```bash
# Using grpcurl
grpcurl -plaintext localhost:8080 cvm.agent.HealthService/Check
```

### Agent Configuration

Environment variables for the agent:

| Variable | Default | Description |
|----------|---------|-------------|
| `REDPILL_API_KEY` | - | API key for LLM access (required for chat) |
| `REDPILL_BASE_URL` | https://api.redpill.ai | LLM API endpoint |
| `REDPILL_MODEL` | gpt-4 | Model to use for chat |
| `WHISPER_MODEL` | base | Whisper model size (tiny/base/small/medium/large) |
| `AGENT_DB_PATH` | ./data/agent.db | SQLite database path |

## Detailed Setup

### Prerequisites

1. SSH key pair for `confidant` user
2. Docker and docker-compose
3. (Optional) Phala Cloud account with CLI for production deployment
4. (Optional) Redpill API key for LLM features

### Add Your SSH Key

Edit `users/confidant/nixos.nix`:

```nix
openssh.authorizedKeys.keys = [
  "ssh-ed25519 AAAAC3Nza... your-key-here"
];
```

Commit and push:

```bash
git add users/confidant/nixos.nix
git commit -m "Add SSH key for confidant user"
git push
```

### Deploy to Phala Cloud

```bash
cd files/phala-cvm

# Deploy using Phala CLI
phala cvms create \
  --name nixos-cvm-agent \
  --compose docker-compose.yml \
  --teepod-id <your-teepod-id> \
  -e .env

# Check deployment
phala cvms list

# View logs
phala cvms logs nixos-cvm-agent
```

Access after deployment:
- noVNC: `https://{DSTACK_APP_ID}.{DSTACK_GATEWAY_DOMAIN}:6080/vnc.html`
- Agent UI: `https://{DSTACK_APP_ID}.{DSTACK_GATEWAY_DOMAIN}:8081`

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Docker Container                          │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                    NixOS System                         ││
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐              ││
│  │  │   XFCE   │  │   VNC    │  │  noVNC   │              ││
│  │  │ Desktop  │──│  Server  │──│  Server  │──► :6080     ││
│  │  └──────────┘  └──────────┘  └──────────┘              ││
│  │       │                                                 ││
│  │  ┌────┴─────────────────────────────────┐              ││
│  │  │            CVM Agent                  │              ││
│  │  │  ┌─────────┐  ┌─────────┐            │              ││
│  │  │  │ Agent   │  │  Web    │            │              ││
│  │  │  │  API    │──│   UI    │──────────────► :8081     ││
│  │  │  │ (Rust)  │  │ (React) │            │              ││
│  │  │  └────┬────┘  └─────────┘            │              ││
│  │  │       │ gRPC ─────────────────────────► :8080      ││
│  │  │  ┌────┴────┐                         │              ││
│  │  │  │ SQLite  │                         │              ││
│  │  │  │ Memory  │                         │              ││
│  │  │  └─────────┘                         │              ││
│  │  └──────────────────────────────────────┘              ││
│  │                                                         ││
│  │  ┌──────────┐                                          ││
│  │  │   SSH    │────────────────────────────► :2222       ││
│  │  │  Server  │                                          ││
│  │  └──────────┘                                          ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

## Configuration Structure

```
├── hosts/phala-cvm/
│   └── default.nix          # Host configuration (XFCE, VNC, SSH, Agent)
├── modules/
│   ├── cvm-system.nix       # Minimal system config (no hardware deps)
│   └── cvm-agent.nix        # CVM Agent NixOS module
├── users/confidant/
│   ├── nixos.nix            # User account & SSH keys
│   └── home.nix             # Home-manager config
├── cvm-agent/               # Agent implementation
│   ├── agent-api/           # Rust gRPC server
│   ├── web-ui/              # React frontend
│   ├── proto/               # Protocol definitions
│   └── skills/              # Built-in skills
└── files/phala-cvm/
    ├── docker-compose.yml   # Container orchestration
    ├── entrypoint.sh        # Startup script
    ├── .env.example         # Environment template
    └── README.md            # This file
```

## Troubleshooting

### Agent Not Starting

```bash
# Check agent logs
docker-compose logs phala-cvm | grep -i agent

# Verify agent binary exists
docker exec phala-cvm ls -la /app/cvm-agent/

# Check agent process
docker exec phala-cvm ps aux | grep agent-api
```

### LLM Features Not Working

1. Verify `REDPILL_API_KEY` is set in `.env`
2. Check API connectivity:
   ```bash
   docker exec phala-cvm curl -s https://api.redpill.ai/health
   ```

### Memory/Database Issues

```bash
# Check database
docker exec phala-cvm sqlite3 /app/data/agent.db '.tables'

# Check integrity
docker exec phala-cvm sqlite3 /app/data/agent.db 'PRAGMA integrity_check;'
```

### VNC/GUI Issues

1. Check Xvfb: `docker exec phala-cvm ps aux | grep Xvfb`
2. Check DISPLAY: `docker exec phala-cvm env | grep DISPLAY`
3. Check XFCE: `docker exec phala-cvm ps aux | grep xfce`

### SSH Connection Refused

1. Verify SSH daemon: `docker exec phala-cvm ps aux | grep sshd`
2. Check authorized_keys is configured
3. Verify port 2222 is accessible

## Security Notes

- **VNC Password**: Change immediately in production
- **SSH Keys**: Use Ed25519 keys
- **API Keys**: Never commit to repository; use `.env` file
- **Firewall**: Restrict access to known IPs in production
- **TEE Benefits**: Memory encryption via Phala's Intel TDX

## References

- [Phala Cloud Documentation](https://docs.phala.com)
- [NixOS & Flakes Book](https://nixos-and-flakes.thiscute.world/best-practices/intro)
- [noVNC Documentation](https://novnc.com/info.html)
- [CVM Agent README](../../cvm-agent/README.md)
