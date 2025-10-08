# Phala Cloud Confidential VM - NixOS Configuration

This directory contains the configuration for deploying a NixOS-based Confidential VM on Phala Cloud with GUI support via VNC/noVNC.

## Features

- **Lightweight GUI**: XFCE desktop environment
- **Remote Access**:
  - SSH on port 2222
  - VNC on port 5900
  - Web-based noVNC on port 6080
- **Confidential Computing**: Runs on Phala Cloud's TEE infrastructure
- **User**: `confidant` with SSH key authentication

## Prerequisites

1. SSH key pair for `confidant` user
2. Phala Cloud account with CLI installed
3. Docker and docker-compose (for local testing)

## Setup Instructions

### 1. Fork and Configure Repository

**Fork the repository:**
```bash
# Fork https://github.com/yourusername/wrlx-nixos-config to your account
```

**Add your SSH public key** to `users/confidant/nixos.nix`:
```nix
openssh.authorizedKeys.keys = [
  "ssh-ed25519 AAAAC3Nza... your-key-here"
];
```

**Commit and push your changes:**
```bash
git add users/confidant/nixos.nix
git commit -m "Add SSH key for confidant user"
git push
```

**Get your commit hash:**
```bash
git rev-parse HEAD
# Example output: f98b4739124fa4b1e547eed36692bb7f28bfa934
```

### 2. Configure Environment Variables

**Copy the environment template:**
```bash
cd files/phala-cvm
cp .env.example .env
```

**Edit `.env` with your values:**
```bash
# Required: Update these values
GITHUB_REPO=https://github.com/YOUR-USERNAME/wrlx-nixos-config.git
GIT_COMMIT_HASH=f98b4739124fa4b1e547eed36692bb7f28bfa934
VNC_PASSWORD=your-secure-password

# Optional
VNC_RESOLUTION=1920x1080
UPDATE_CODE=false
```

### 3. Build the Configuration (Optional - Local Testing)

```bash
# From the repository root
nix flake check
nix build .#nixosConfigurations.phala-cvm.config.system.build.toplevel
```

## Deployment Options

### Option A: Local Testing with Docker Compose

```bash
cd files/phala-cvm

# Make sure .env is configured
docker-compose up -d

# Check logs
docker-compose logs -f

# Access:
# - SSH: ssh -p 2222 confidant@localhost
# - noVNC: http://localhost:6080/vnc.html
# - VNC: vnc://localhost:5900
```

### Option B: Deploy to Phala Cloud (Recommended)

```bash
cd files/phala-cvm

# Deploy using Phala CLI
phala cvms create \
  --name nixos-cvm-gui \
  --compose docker-compose.yml \
  --teepod-id <your-teepod-id> \
  -e .env

# Check deployment status
phala cvms list

# View logs
phala cvms logs nixos-cvm-gui
```

**After deployment, access your CVM:**
- noVNC: `https://{DSTACK_APP_ID}.{DSTACK_GATEWAY_DOMAIN}:6080/vnc.html`
- SSH: Use the Phala Cloud gateway endpoint

## Accessing the CVM

### SSH Access

```bash
# Replace <host> with your CVM's public IP or localhost for testing
ssh -p 2222 confidant@<host>

# SSH tunnel for secure VNC
ssh -p 2222 -L 5900:localhost:5900 confidant@<host>
```

### GUI Access

**Web Browser (noVNC):**
```
http://<host>:6080/vnc.html
```
- Password: Set via `VNC_PASSWORD` environment variable
- Best for quick access, works in any browser

**VNC Client:**
```
vnc://<host>:5900
```
- Password: Same as noVNC
- Better performance for desktop use
- Recommended: Use SSH tunnel for encryption

## Configuration Structure

```
├── hosts/phala-cvm/
│   └── default.nix          # Host configuration (XFCE, VNC, SSH)
├── modules/
│   └── cvm-system.nix       # Minimal system config (no hardware deps)
├── users/confidant/
│   ├── nixos.nix            # User account & SSH keys
│   └── home.nix             # Home-manager config (lightweight)
└── files/phala-cvm/
    ├── docker-compose.yml   # Container orchestration
    ├── entrypoint.sh        # Startup script (Xvfb, VNC, noVNC)
    └── README.md            # This file
```

## Customization

### Add More Applications

Edit `hosts/phala-cvm/default.nix` and add packages:

```nix
environment.systemPackages = with pkgs; [
  # Add your packages here
  vscode
  gimp
  libreoffice
];
```

### Switch Desktop Environment

Replace XFCE with another DE in `hosts/phala-cvm/default.nix`:

```nix
# For i3 window manager
services.xserver.windowManager.i3.enable = true;

# For GNOME (heavier)
services.xserver.desktopManager.gnome.enable = true;
```

### Adjust Display Resolution

Edit `entrypoint.sh`:

```bash
# Change from default 1920x1080
Xvfb :1 -screen 0 2560x1440x24 &
```

## Troubleshooting

### Services Not Starting

Check container logs:
```bash
docker-compose logs -f
```

### Can't Connect to VNC

1. Verify ports are exposed: `docker ps`
2. Check firewall rules on host
3. Ensure VNC server is running: `docker exec phala-cvm ps aux | grep vnc`

### SSH Connection Refused

1. Verify SSH daemon is running: `docker exec phala-cvm ps aux | grep sshd`
2. Check authorized_keys is configured
3. Verify port 2222 is accessible

### GUI Not Displaying

1. Check Xvfb is running: `docker exec phala-cvm ps aux | grep Xvfb`
2. Verify DISPLAY variable: `docker exec phala-cvm env | grep DISPLAY`
3. Check XFCE session: `docker exec phala-cvm ps aux | grep xfce`

## Security Notes

- **VNC Password**: Change the default password immediately
- **SSH Keys**: Use strong SSH key pairs (Ed25519 recommended)
- **Firewall**: Consider restricting access to known IPs
- **SSL/TLS**: For production, add SSL termination (nginx reverse proxy)
- **TEE Benefits**: Memory encryption provided by Phala's Intel TDX

## Building from Flake

To build the NixOS configuration:

```bash
# Build the system configuration
nix build .#nixosConfigurations.phala-cvm.config.system.build.toplevel

# Check flake
nix flake check

# Show flake outputs
nix flake show
```

## References

- [Phala Cloud Documentation](https://docs.phala.com)
- [NixOS & Flakes Book](https://nixos-and-flakes.thiscute.world/best-practices/intro)
- [noVNC Documentation](https://novnc.com/info.html)
- [TigerVNC Documentation](https://tigervnc.org/)
