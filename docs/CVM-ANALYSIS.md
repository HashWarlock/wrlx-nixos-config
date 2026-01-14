# CVM Agent Architecture Analysis

## Docker Compose Warnings

### Current Warnings
```
level=warning msg="The \"AGENT_DOMAIN\" variable is not set. Defaulting to a blank string."
level=warning msg="The \"DSTACK_APP_ID\" variable is not set. Defaulting to a blank string."
level=warning msg="The \"DSTACK_GATEWAY_DOMAIN\" variable is not set. Defaulting to a blank string."
level=warning msg="...docker-compose.yml: the attribute `version` is obsolete..."
```

### Analysis
| Warning | Purpose | Needed for Local Dev? |
|---------|---------|----------------------|
| `AGENT_DOMAIN` | Phala Cloud auto-assigned domain | No |
| `DSTACK_APP_ID` | DStack deployment ID | No |
| `DSTACK_GATEWAY_DOMAIN` | DStack gateway URL | No |
| `version: '3.8'` | Obsolete docker-compose syntax | No |

### Recommended Fixes
1. **Remove `version` attribute** - obsolete in modern Docker Compose
2. **Add `.env.example`** with optional Phala Cloud variables
3. **Make Phala variables optional** with sensible defaults

---

## Web UI Architecture for Transparent Chat Overlay

### Current Design
```
┌─────────────────────────────────────────────────────────┐
│                    NixOS VM Desktop                      │
│  ┌───────────────────────────────────────────────────┐  │
│  │                 XFCE Desktop (VNC)                 │  │
│  │                                                    │  │
│  │                                                    │  │
│  │   ┌──────────────────────────────────────────┐    │  │
│  │   │         Chat Overlay (Web UI)            │    │  │
│  │   │  ┌────────────────────────────────────┐  │    │  │
│  │   │  │ Ask the agent...              [>] │  │    │  │
│  │   │  └────────────────────────────────────┘  │    │  │
│  │   └──────────────────────────────────────────┘    │  │
│  │                                              [?]  │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### Key Components
1. **ChatOverlay** (`src/components/ChatOverlay.tsx`)
   - Transparent overlay with blur backdrop
   - Keyboard shortcut: `Cmd/Ctrl + Shift + Space`
   - Expandable from compact to full sidebar

2. **useAgent Hook** (`src/hooks/useAgent.ts`)
   - Uses ConnectRPC with streaming responses
   - Connects to `/api` (proxied to Agent API on :8080)
   - Handles streaming text responses

3. **Vite Dev Server** (vite.config.ts)
   - Proxies `/api` → `http://localhost:8080`
   - Required for gRPC-Web to work

### Current Problem
The container uses `python3 -m http.server` which:
- Serves static files correctly
- **Cannot proxy API requests** to the Agent API
- Results in 501 errors for gRPC-Web calls

---

## Invoking Agent from Within the VM

### Option 1: Run Vite Dev Server (Development)
```bash
cd /app/cvm-agent/web-ui
npm run dev -- --host 0.0.0.0
```
- Full proxy support
- Hot reload for development
- Requires Node.js (already in devshell)

### Option 2: Nginx Reverse Proxy (Production)
```nginx
server {
    listen 3000;

    location / {
        root /app/cvm-agent/web-ui/dist;
        try_files $uri $uri/ /index.html;
    }

    location /api/ {
        proxy_pass http://localhost:8080/;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

### Option 3: Caddy Reverse Proxy (Simpler)
```caddyfile
:3000 {
    reverse_proxy /api/* localhost:8080
    file_server {
        root /app/cvm-agent/web-ui/dist
    }
}
```

### How to Access from Within VM
1. Open Firefox in the XFCE desktop
2. Navigate to `http://localhost:3000`
3. Press `Ctrl+Shift+Space` to open chat overlay
4. Type commands to control the VM

---

## NixOS VM Configuration Simplification

### Current Architecture
```
┌─────────────────────────────────────────────┐
│           Docker Container                   │
│  ┌───────────────────────────────────────┐  │
│  │         nixos/nix:latest              │  │
│  │  ┌─────────────────────────────────┐  │  │
│  │  │     nix develop .#cvm           │  │  │
│  │  │  ┌───────────────────────────┐  │  │  │
│  │  │  │   cvm-start.sh            │  │  │  │
│  │  │  │   - X11 (Xvfb)            │  │  │  │
│  │  │  │   - XFCE Desktop          │  │  │  │
│  │  │  │   - VNC Server            │  │  │  │
│  │  │  │   - noVNC                 │  │  │  │
│  │  │  │   - Agent API             │  │  │  │
│  │  │  │   - Web UI                │  │  │  │
│  │  │  │   - Whisper               │  │  │  │
│  │  │  └───────────────────────────┘  │  │  │
│  │  └─────────────────────────────────┘  │  │
│  └───────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

### Issues Identified
1. **No proper reverse proxy** for Web UI
2. **Screenshot service** uses scrot with unsupported flags
3. **AT-SPI not configured** for accessibility tree
4. **Services start sequentially** (could be parallel)

### Recommended Simplifications

#### 1. Replace Python HTTP Server with Caddy
```diff
- python3 -m http.server $PORT --bind 0.0.0.0
+ caddy run --config /app/files/phala-cvm/Caddyfile
```

Benefits:
- Automatic HTTPS (when domain is set)
- Built-in reverse proxy
- Single binary, no dependencies

#### 2. Fix scrot Command
```diff
- scrot --hidecursor -o "$output_file"
+ scrot -o "$output_file"
```
Or use alternative:
```bash
import -window root "$output_file"  # ImageMagick
```

#### 3. Simplify Service Startup
Current: Sequential with sleep delays
Better: Use process supervisor (s6, runit, or supervisord)

```yaml
# supervisord.conf
[program:xvfb]
command=Xvfb :1 -screen 0 1920x1080x24

[program:vnc]
command=x0vncserver -display :1 -passwordfile /root/.vnc/passwd
depends_on=xvfb

[program:novnc]
command=novnc --vnc localhost:5900 --listen 6080
depends_on=vnc

[program:agent-api]
command=/app/cvm-agent/target/release/agent-api

[program:web-ui]
command=caddy run --config /app/Caddyfile
depends_on=agent-api
```

#### 4. Remove Unused NixOS Modules
The container doesn't actually run NixOS - it uses `nix develop` for package management. The NixOS modules (`cvm-system.nix`, `cvm-agent.nix`) are for future native NixOS deployment.

For Docker container:
- Remove systemd service definitions (not used)
- Simplify to just package lists
- Use shell scripts or supervisord for services

---

## Rendering Issues & Fixes

### Issue: X11 Display Conflicts
```
(EE) Server is already active for display 1
```

**Cause**: Xvfb already running when script tries to start it again.

**Fix**: Check if display is in use:
```bash
if ! pgrep -f "Xvfb :1" > /dev/null; then
    Xvfb :1 -screen 0 1920x1080x24 &
fi
```

### Issue: VNC Password Not Set
**Fix**: Ensure password file exists:
```bash
mkdir -p /root/.vnc
echo "$VNC_PASSWORD" | vncpasswd -f > /root/.vnc/passwd
chmod 600 /root/.vnc/passwd
```

### Issue: XFCE Not Starting
**Cause**: Session files or dbus issues.

**Fix**: Clean start:
```bash
rm -rf ~/.cache/sessions ~/.config/xfce4/xfconf
dbus-launch --exit-with-session startxfce4
```

---

## Summary of Recommendations

| Category | Current | Recommended |
|----------|---------|-------------|
| Web Server | python http.server | Caddy or nginx |
| Process Mgmt | Shell scripts | supervisord |
| Screenshot | scrot --hidecursor | scrot (no flag) or import |
| Service Start | Sequential | Parallel with dependencies |
| Config Files | Hardcoded | Environment-based |

### Priority Order
1. **P0**: Fix Web UI proxy (enable chat overlay)
2. **P1**: Fix scrot command (enable screenshots)
3. **P2**: Add process supervisor (reliability)
4. **P3**: Clean up docker-compose warnings

---

## Configuration Gap Analysis

### Two Deployment Modes

The current configuration has **two parallel setups** that serve different purposes:

| Mode | Configuration | Purpose | Currently Used |
|------|---------------|---------|----------------|
| **Native NixOS** | `hosts/phala-cvm/default.nix` + `modules/cvm-*.nix` | Production NixOS VM | **Not yet** |
| **Docker + Nix** | `flake.nix#devShells.cvm` + shell scripts | Development/Testing | **Yes** |

### Package Availability Gap

**Firefox is defined in two places:**

```
hosts/phala-cvm/default.nix:44  →  firefox  ✓ (NixOS module)
flake.nix#devShells.cvm         →  (missing) ✗ (Docker devshell)
```

This explains why `firefox: command not found` when trying to invoke the agent from within the VM.

### Devshell Package Categories

Current packages in `.#cvm` devshell:

| Category | Packages | Status |
|----------|----------|--------|
| VNC/GUI | tigervnc, xorg.xorgserver, xfce.*, novnc | ✓ Complete |
| System | dbus, openssh, procps, which, bash, coreutils | ✓ Complete |
| GUI Automation | xdotool, scrot, at-spi2-core, pyatspi | ✓ Complete |
| Voice | whisper-cpp | ✓ Complete |
| Build Tools | rustc, cargo, gcc, pkg-config, openssl, protobuf | ✓ Complete |
| Web Dev | python3, nodejs, npm | ✓ Complete |
| **Browser** | **(none)** | **✗ Missing** |

### Missing Browser Options

To enable in-VM agent invocation, add ONE of these to the devshell:

```nix
# Option 1: Firefox (recommended - more compatible)
firefox

# Option 2: Chromium (alternative)
chromium

# Option 3: Lightweight (minimal)
midori
```

---

## Simplified NixOS VM Architecture

### Proposed Minimal Configuration

For consistent rendering and easy customization, use this layered approach:

```
┌─────────────────────────────────────────────────────────────┐
│                    Simplified CVM Stack                      │
├─────────────────────────────────────────────────────────────┤
│  Layer 4: Agent Customization (via CVM Agent prompts)        │
│           - Install packages: "Add VS Code to the VM"        │
│           - Configure desktop: "Set wallpaper to..."         │
│           - Modify config: "Enable dark mode"                │
├─────────────────────────────────────────────────────────────┤
│  Layer 3: Application Services                               │
│           - Agent API (gRPC on :8080)                        │
│           - Web UI (Caddy on :3000 with /api proxy)          │
│           - Whisper (voice on :8082)                         │
├─────────────────────────────────────────────────────────────┤
│  Layer 2: Desktop Environment                                │
│           - XFCE (minimal, stable, well-documented)          │
│           - Xvfb + TigerVNC + noVNC                          │
│           - Firefox for in-VM browsing                       │
├─────────────────────────────────────────────────────────────┤
│  Layer 1: Base System                                        │
│           - Nix devshell with deterministic packages         │
│           - D-Bus, SSH, core utilities                       │
│           - Single configuration file                        │
└─────────────────────────────────────────────────────────────┘
```

### Core Principles for Easy Customization

1. **Single Source of Truth**: All packages in `flake.nix#devShells.cvm`
2. **Environment Variables**: Configure via `.env` file
3. **Modular Services**: Each service in its own script
4. **Agent-Controllable**: Standard commands for common operations

### Agent Customization Commands

The CVM Agent can modify the VM via these patterns:

```bash
# Install a package (via nix profile)
nix profile install nixpkgs#vscode

# Modify XFCE settings
xfconf-query -c xfwm4 -p /general/theme -s "Adwaita-dark"

# Change wallpaper
xfconf-query -c xfce4-desktop -p /backdrop/screen0/monitor0/workspace0/last-image -s "/path/to/image.png"

# Add autostart application
cat > ~/.config/autostart/myapp.desktop << 'EOF'
[Desktop Entry]
Type=Application
Name=My App
Exec=/path/to/app
EOF
```

### Configuration File: `.env.cvm`

```bash
# Display
VNC_PASSWORD=changeme
VNC_RESOLUTION=1920x1080

# Services (disable with "false")
ENABLE_VNC=true
ENABLE_NOVNC=true
ENABLE_SSH=true
ENABLE_AGENT_API=true
ENABLE_WEB_UI=true
ENABLE_WHISPER=true

# Ports
VNC_PORT=5900
NOVNC_PORT=6080
SSH_PORT=22
AGENT_API_PORT=8080
WEB_UI_PORT=3000
WHISPER_PORT=8082

# API Keys (optional)
REDPILL_API_KEY=
REDPILL_MODEL=claude-3-5-sonnet-20241022

# Phala Cloud (optional - only for cloud deployment)
# AGENT_DOMAIN=
# DSTACK_APP_ID=
# DSTACK_GATEWAY_DOMAIN=
```

---

## Recommended Devshell Changes

### Add to `flake.nix#devShells.cvm`

```nix
devShells.x86_64-linux.cvm = cvmPkgs.mkShell {
  packages = with cvmPkgs; [
    # ... existing packages ...

    # ADD: Browser for in-VM agent invocation
    firefox

    # ADD: Proper reverse proxy (replace python http.server)
    caddy

    # OPTIONAL: Better screenshot tool
    imagemagick  # provides 'import' command
  ];
};
```

### Create Caddyfile for Web UI Proxy

```caddyfile
# /app/files/phala-cvm/Caddyfile
:3000 {
    handle /api/* {
        reverse_proxy localhost:8080
    }
    handle {
        root * /app/cvm-agent/web-ui/dist
        try_files {path} /index.html
        file_server
    }
}
```

### Update web-ui.sh to Use Caddy

```bash
#!/bin/sh
# files/phala-cvm/services/web-ui.sh

case "$1" in
    start)
        cd /app/cvm-agent/web-ui
        npm install && npm run build
        caddy run --config /app/files/phala-cvm/Caddyfile &
        ;;
    stop)
        pkill -f "caddy run"
        ;;
esac
```

---

## Native Chat Overlay Application

### Requirements

Instead of requiring a browser, the chat overlay should be:
- **Summoned by hotkey** (like Spotlight/Alfred/Rofi)
- **Floating window** that appears on any workspace
- **Transparent/minimal** chrome, focuses on input
- **Quick dismiss** with Escape or click-outside

### Architecture Options

| Approach | Pros | Cons | Complexity |
|----------|------|------|------------|
| **Tauri** | Rust backend, lightweight WebView | Requires build step | Medium |
| **Electron** | Full Node.js, easy to package | Heavy (100MB+) | Low |
| **GTK + WebKitGTK** | Native Linux, very lightweight | GTK knowledge needed | Medium |
| **PyWebView** | Python, cross-platform, simple | Python dependency | Low |
| **Rofi + gRPC client** | No web tech, pure terminal | Limited UI | Low |

### Recommended: Tauri-based Overlay

Since we already have Rust for the Agent API, Tauri is the natural choice:

```
┌─────────────────────────────────────────────┐
│              Tauri Chat Overlay              │
├─────────────────────────────────────────────┤
│  Frontend: Existing React Web UI            │
│  Backend: Rust (direct gRPC, no proxy)      │
│  Hotkey: Global shortcut registration       │
│  Window: Frameless, transparent, floating   │
└─────────────────────────────────────────────┘
```

**Benefits:**
- Reuses existing React components
- Native Rust backend can call gRPC directly
- ~10MB binary (vs 100MB+ Electron)
- Global hotkey support built-in
- Transparent window support

### Alternative: PyWebView Quick Implementation

For rapid prototyping, use Python + pywebview:

```python
#!/usr/bin/env python3
# cvm-overlay.py - Native chat overlay
import webview
import keyboard  # for global hotkey

window = None

def toggle_overlay():
    global window
    if window:
        if window.minimized:
            window.restore()
        else:
            window.minimize()

def create_overlay():
    global window
    window = webview.create_window(
        'CVM Agent',
        'http://localhost:3000',
        width=400,
        height=600,
        frameless=True,
        on_top=True,
        transparent=True,
    )
    # Register global hotkey
    keyboard.add_hotkey('ctrl+shift+space', toggle_overlay)
    webview.start()

if __name__ == '__main__':
    create_overlay()
```

### Rofi-style Terminal Overlay (Simplest)

For immediate use without new packages:

```bash
#!/bin/bash
# cvm-chat.sh - Rofi-style chat interface

# Get user input via rofi
query=$(rofi -dmenu -p "Ask CVM Agent:" -theme-str 'window {width: 50%;}')

if [ -n "$query" ]; then
    # Call Agent API via gRPC
    response=$(grpcurl -plaintext -d "{\"message\": \"$query\"}" \
        localhost:8080 agent.ChatService/SendMessage 2>/dev/null | jq -r '.text')

    # Show response in notification
    notify-send "CVM Agent" "$response"
fi
```

**XFCE Hotkey Setup:**
```bash
# Settings > Keyboard > Application Shortcuts
# Command: /app/cvm-chat.sh
# Shortcut: Ctrl+Shift+Space
```

### Implementation Plan

**Phase 1 (Immediate):** Rofi + gRPC script
- Uses existing tools in devshell
- Works immediately after adding `rofi` package
- Simple but functional

**Phase 2 (Short-term):** PyWebView overlay
- Better UI with existing Web UI
- Add `python3Packages.pywebview` to devshell
- Hotkey via xdotool or pynput

**Phase 3 (Full):** Tauri native app
- Package as standalone binary
- Add to devshell as custom package
- Full feature parity with Web UI

### Required Devshell Additions

```nix
# For Phase 1 (Rofi)
rofi
jq
libnotify

# For Phase 2 (PyWebView)
python3Packages.pywebview
python3Packages.pynput  # for global hotkeys

# For Phase 3 (Tauri) - build dependencies
cargo-tauri
webkit2gtk
```

---

## Testing In-VM Agent Invocation (After Fixes)

### With Browser (Basic)
1. **Rebuild container** with updated devshell (includes Firefox + Caddy)
2. **Start services** with Caddy serving Web UI
3. **Open Firefox** in VNC desktop
4. **Navigate to** `http://localhost:3000`
5. **Press** `Ctrl+Shift+Space` to open chat overlay
6. **Type** "List installed packages" to verify agent responds
7. **Execute** "Install htop" to test agent customization

### With Native Overlay (Recommended)
1. **Add rofi/jq/libnotify** to devshell
2. **Create** `/app/cvm-chat.sh` script
3. **Configure** XFCE keyboard shortcut (Ctrl+Shift+Space)
4. **Press** `Ctrl+Shift+Space` anywhere in desktop
5. **Type** query in rofi popup
6. **View** response in notification
