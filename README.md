# WRLX NixOS Config

Personal NixOS configuration using flakes and home-manager. Learning as I go.

## Overview

This repository manages both system-level (NixOS) and user-level (home-manager) configurations in a declarative manner. It includes configurations for personal workstations and a Confidential VM (CVM) deployment with an AI-powered agent.

## Host Configurations

| Host | User | Purpose | Desktop |
|------|------|---------|---------|
| `asus-g512lw` | `hashwarlock` | Primary laptop | Hyprland/GNOME with GDM |
| `phala-cvm` | `confidant` | Confidential VM | XFCE + VNC/noVNC + CVM Agent |

## Features

### Personal Workstation (`asus-g512lw`)
- **Hyprland** as primary window manager with GNOME as fallback
- **Catppuccin** theming (Macchiato flavor with lavender accent)
- Home-manager integration for user-level configuration
- Unstable packages overlay for bleeding-edge software

### Confidential VM (`phala-cvm`)
- Lightweight XFCE desktop accessible via VNC/noVNC
- **CVM Agent**: AI-powered system administration interface
- Designed for Phala Cloud's TEE infrastructure
- No hardware-specific dependencies

## CVM Agent

The CVM Agent provides an AI-powered interface for managing NixOS systems remotely. See [`cvm-agent/`](cvm-agent/) for the full implementation.

### Capabilities

| Service | Description |
|---------|-------------|
| **ChatService** | LLM-powered conversation with Redpill API |
| **ShellService** | Command execution with streaming output |
| **NixOpsService** | Rebuild, rollback, list generations |
| **GitOpsService** | Status, diff, commit, push with risk classification |
| **GUIService** | Screenshot, element discovery, click/type automation |
| **VoiceService** | Whisper-based voice transcription |
| **SkillsService** | Workflow automation with trigger matching |
| **MemoryService** | 4-layer memory with active forgetting |

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Web UI (React)                         │
│  ChatOverlay │ DiffPreview │ MemoryPanel │ GUIInspector    │
└─────────────────────────┬───────────────────────────────────┘
                          │ gRPC-web
┌─────────────────────────┴───────────────────────────────────┐
│                    Agent API (Rust)                         │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐           │
│  │  Chat   │ │  Shell  │ │ NixOps  │ │ GitOps  │           │
│  └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘           │
│       │           │           │           │                 │
│  ┌────┴────┐ ┌────┴────┐ ┌────┴────┐ ┌────┴────┐           │
│  │   GUI   │ │  Voice  │ │ Skills  │ │ Memory  │           │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘           │
│                          │                                  │
│              ┌───────────┴───────────┐                      │
│              │  SQLite + Forgetting  │                      │
│              └───────────────────────┘                      │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

### Personal Workstation

```bash
# Clone the repository
git clone https://github.com/HashWarlock/wrlx-nixos-config.git
cd wrlx-nixos-config

# Rebuild and switch
sudo nixos-rebuild switch --flake .#asus-g512lw
```

### Confidential VM

See [`files/phala-cvm/README.md`](files/phala-cvm/README.md) for detailed deployment instructions.

```bash
# Local testing with Docker
cd files/phala-cvm
cp .env.example .env
# Edit .env with your values
docker-compose up -d

# Access:
# - noVNC: http://localhost:6080/vnc.html
# - Agent UI: http://localhost:8081
# - SSH: ssh -p 2222 confidant@localhost
```

## Common Commands

```bash
# Rebuild and switch configuration
sudo nixos-rebuild switch --flake .#<hostname>

# Test configuration (doesn't persist to boot)
sudo nixos-rebuild test --flake .#<hostname>

# Update flake inputs
nix flake update

# Check flake for errors
nix flake check

# Build CVM configuration
nix build .#nixosConfigurations.phala-cvm.config.system.build.toplevel
```

## Directory Structure

```
├── flake.nix              # Main flake configuration
├── hosts/                 # Host-specific configurations
│   ├── asus-g512lw/       # Personal laptop
│   └── phala-cvm/         # Confidential VM
├── modules/               # System-level NixOS modules
├── users/                 # Per-user configurations
├── home/                  # Home-manager modules
├── overlays/              # Nixpkgs overlays
├── files/phala-cvm/       # CVM deployment files
│   ├── docker-compose.yml
│   ├── entrypoint.sh
│   └── README.md
└── cvm-agent/             # AI agent implementation
    ├── agent-api/         # Rust gRPC server
    ├── web-ui/            # React frontend
    ├── proto/             # Protocol buffer definitions
    └── skills/            # Built-in workflow skills
```

## Development

### CVM Agent Development

```bash
cd cvm-agent

# Run Rust tests
cargo test -p agent-api

# Build release
cargo build -p agent-api --release

# Web UI development
cd web-ui
npm install
npm run dev
```

## NixOS Configs That Heavily Influenced My NixOS Config

- https://github.com/ryan4yin/nix-config
- https://github.com/AlexNabokikh/nix-config

## License

MIT
