#!/bin/sh
set -e

echo "=== Phala CVM NixOS Environment Setup ==="

# Verify repository was cloned by docker-compose
if [ ! -d "/app" ]; then
    echo "ERROR: /app directory not found. Repository should be cloned by docker-compose."
    exit 1
fi

echo "Working directory: $(pwd)"
echo "Repository cloned at: /app"

# Install necessary packages for VNC/GUI setup
echo "Installing VNC and GUI components..."
nix-env -iA \
    nixpkgs.tigervnc \
    nixpkgs.xorg.xorgserver \
    nixpkgs.xfce.xfce4-session \
    nixpkgs.xfce.xfce4-panel \
    nixpkgs.xfce.xfwm4 \
    nixpkgs.xfce.xfdesktop \
    nixpkgs.xfce.xfce4-settings \
    nixpkgs.xfce.xfce4-terminal \
    nixpkgs.xfce.xfce4-appfinder \
    nixpkgs.xfce.xfconf \
    nixpkgs.xfce.thunar \
    nixpkgs.dbus \
    nixpkgs.openssh \
    nixpkgs.procps \
    nixpkgs.which \
    nixpkgs.bash \
    nixpkgs.hostname \
    nixpkgs.fontconfig \
    nixpkgs.coreutils \
    nixpkgs.gnused

# Install noVNC
if ! command -v novnc > /dev/null 2>&1; then
    echo "Installing noVNC..."
    nix-env -iA nixpkgs.novnc
fi

# Set VNC password
VNC_PASSWORD=${VNC_PASSWORD:-"changeme"}
VNC_RESOLUTION=${VNC_RESOLUTION:-"1920x1080"}
mkdir -p ~/.vnc
echo "$VNC_PASSWORD" | vncpasswd -f > ~/.vnc/passwd
chmod 600 ~/.vnc/passwd
echo "VNC password configured"

# Start Xvfb (virtual framebuffer)
echo "Starting Xvfb with resolution ${VNC_RESOLUTION}..."
Xvfb :1 -screen 0 ${VNC_RESOLUTION}x24 &
XVFB_PID=$!
sleep 2

# Export display
export DISPLAY=:1

# Setup machine-id for D-Bus
echo "Setting up machine-id..."
mkdir -p /var/lib/dbus /etc
if [ ! -f /var/lib/dbus/machine-id ]; then
    dbus-uuidgen --ensure=/var/lib/dbus/machine-id
fi
if [ ! -f /etc/machine-id ]; then
    ln -s /var/lib/dbus/machine-id /etc/machine-id || cp /var/lib/dbus/machine-id /etc/machine-id
fi

# Setup D-Bus session config
echo "Setting up D-Bus configuration..."
mkdir -p /etc/dbus-1/session.d /etc/dbus-1/system.d
# Always create a minimal session.conf to avoid circular inclusion issues
cat > /etc/dbus-1/session.conf << 'EOF'
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=/tmp</listen>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
EOF

# Start D-Bus session
echo "Starting D-Bus session..."
eval $(dbus-launch --sh-syntax)
export DBUS_SESSION_BUS_ADDRESS
export DBUS_SESSION_BUS_PID

# Set XDG environment variables for XFCE
echo "Setting up XDG environment variables..."
export XDG_DATA_DIRS="/nix/var/nix/profiles/default/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
export XDG_CONFIG_DIRS="/nix/var/nix/profiles/default/etc/xdg:${XDG_CONFIG_DIRS:-/etc/xdg}"

# Find and add XFCE session directory to XDG_CONFIG_DIRS
XFCE_SESSION_DIR=$(find /nix/store -name "xfce4-session-*" -type d 2>/dev/null | head -1)
if [ -n "$XFCE_SESSION_DIR" ]; then
    export XDG_CONFIG_DIRS="${XFCE_SESSION_DIR}/etc:${XDG_CONFIG_DIRS}"
    echo "Added XFCE session directory: ${XFCE_SESSION_DIR}/etc"
fi

# Start xfconfd (XFCE configuration daemon) if available
if command -v xfconfd > /dev/null 2>&1; then
    echo "Starting xfconfd..."
    xfconfd &
    sleep 1
else
    echo "xfconfd not found, skipping..."
fi

# Start window manager first (required for _NET_* properties)
echo "Starting xfwm4 window manager..."
xfwm4 --daemon &
sleep 2

# Start desktop manager
echo "Starting xfdesktop..."
xfdesktop &
sleep 1

# Start XFCE panel
echo "Starting xfce4-panel..."
xfce4-panel &
sleep 1

# Start VNC server
echo "Starting VNC server on display :1..."
x0vncserver -display :1 -passwordfile ~/.vnc/passwd -rfbport 5900 &
VNC_PID=$!

# Start noVNC websocket proxy
echo "Starting noVNC..."
novnc --vnc localhost:5900 --listen 6080 &
NOVNC_PID=$!

# Start SSH daemon
echo "Starting SSH daemon..."
mkdir -p /run/sshd /etc/ssh

# Generate host keys if they don't exist
if [ ! -f /etc/ssh/ssh_host_rsa_key ]; then
    echo "Generating SSH host keys..."
    ssh-keygen -A 2>/dev/null || true
fi

# Create sshd_config if it doesn't exist
if [ ! -f /etc/ssh/sshd_config ]; then
    echo "Creating sshd_config..."
    cat > /etc/ssh/sshd_config << 'EOF'
Port 22
PermitRootLogin no
PasswordAuthentication no
PubkeyAuthentication yes
UsePAM no
X11Forwarding yes
PrintMotd no
AcceptEnv LANG LC_*
Subsystem sftp /nix/store/*/libexec/sftp-server
EOF
    # Fix sftp-server path
    SFTP_SERVER=$(find /nix/store -name "sftp-server" 2>/dev/null | head -1)
    if [ -n "$SFTP_SERVER" ]; then
        sed -i "s|/nix/store/\*/libexec/sftp-server|$SFTP_SERVER|" /etc/ssh/sshd_config
    fi
fi

# Use which to find sshd binary
SSHD_BIN=$(which sshd)
if [ -n "$SSHD_BIN" ]; then
    $SSHD_BIN -D -e &
    SSH_PID=$!
else
    echo "WARNING: sshd not found in PATH"
    SSH_PID=""
fi

# =========================
# CVM Agent Service Setup
# =========================
echo "Setting up CVM Agent service..."

# Install Rust toolchain if not present
if ! command -v cargo > /dev/null 2>&1; then
    echo "Installing Rust toolchain..."
    nix-env -iA nixpkgs.rustc nixpkgs.cargo nixpkgs.gcc nixpkgs.pkg-config nixpkgs.openssl
fi

# Build the agent binary if source exists
AGENT_API_DIR="/app/cvm-agent/agent-api"
AGENT_BINARY="/app/cvm-agent/agent-api/target/release/agent-api"

if [ -d "$AGENT_API_DIR" ]; then
    echo "Building CVM Agent API..."
    cd "$AGENT_API_DIR"

    # Set OpenSSL environment for build
    export PKG_CONFIG_PATH=$(find /nix/store -name "pkgconfig" -type d 2>/dev/null | head -1):$PKG_CONFIG_PATH

    # Build in release mode
    if cargo build --release; then
        echo "Agent API built successfully"
    else
        echo "WARNING: Agent API build failed"
    fi

    cd /app
else
    echo "WARNING: Agent API source not found at $AGENT_API_DIR"
fi

# Start the agent API service
AGENT_PID=""
if [ -x "$AGENT_BINARY" ]; then
    echo "Starting CVM Agent API on port 8080..."
    $AGENT_BINARY &
    AGENT_PID=$!
    sleep 2

    if kill -0 $AGENT_PID 2>/dev/null; then
        echo "Agent API started with PID $AGENT_PID"
    else
        echo "WARNING: Agent API failed to start"
        AGENT_PID=""
    fi
else
    echo "WARNING: Agent binary not found or not executable at $AGENT_BINARY"
fi

# Start web UI server
WEB_UI_DIR="/app/cvm-agent/web-ui/dist"
WEB_UI_PID=""

if [ -d "$WEB_UI_DIR" ]; then
    echo "Starting Web UI server on port 8081..."

    # Install python if not present (for simple HTTP server)
    if ! command -v python3 > /dev/null 2>&1; then
        echo "Installing python3 for web server..."
        nix-env -iA nixpkgs.python3
    fi

    cd "$WEB_UI_DIR"
    python3 -m http.server 8081 --bind 0.0.0.0 &
    WEB_UI_PID=$!
    sleep 1

    if kill -0 $WEB_UI_PID 2>/dev/null; then
        echo "Web UI server started with PID $WEB_UI_PID"
    else
        echo "WARNING: Web UI server failed to start"
        WEB_UI_PID=""
    fi

    cd /app
else
    echo "WARNING: Web UI dist not found at $WEB_UI_DIR"
    echo "Run 'npm run build' in the web-ui directory to build it"
fi

echo "=== CVM Services Started Successfully ==="
echo "Repository: $GITHUB_REPO"
echo "Commit: $GIT_COMMIT_HASH"
echo "SSH: port 22 (mapped to host port 2222)"
echo "VNC: port 5900"
echo "noVNC: http://localhost:6080/vnc.html"
echo "Display Resolution: $VNC_RESOLUTION"
if [ -n "$AGENT_PID" ]; then
    echo "Agent API: http://localhost:8080 (gRPC)"
fi
if [ -n "$WEB_UI_PID" ]; then
    echo "Web UI: http://localhost:8081"
fi
if [ -n "$AGENT_DOMAIN" ]; then
    echo "Phala Cloud Domain: $AGENT_DOMAIN"
fi
echo "==="

# Trap signals for graceful shutdown
cleanup() {
    echo "Shutting down..."
    [ -n "$WEB_UI_PID" ] && kill $WEB_UI_PID 2>/dev/null
    [ -n "$AGENT_PID" ] && kill $AGENT_PID 2>/dev/null
    [ -n "$SSH_PID" ] && kill $SSH_PID 2>/dev/null
    [ -n "$NOVNC_PID" ] && kill $NOVNC_PID 2>/dev/null
    [ -n "$VNC_PID" ] && kill $VNC_PID 2>/dev/null
    [ -n "$XVFB_PID" ] && kill $XVFB_PID 2>/dev/null
    [ -n "$DBUS_SESSION_BUS_PID" ] && kill $DBUS_SESSION_BUS_PID 2>/dev/null
    exit 0
}
trap cleanup SIGTERM SIGINT

# Wait for all processes (only wait for those that exist)
WAIT_PIDS="$XVFB_PID $VNC_PID $NOVNC_PID"
[ -n "$SSH_PID" ] && WAIT_PIDS="$WAIT_PIDS $SSH_PID"
[ -n "$AGENT_PID" ] && WAIT_PIDS="$WAIT_PIDS $AGENT_PID"
[ -n "$WEB_UI_PID" ] && WAIT_PIDS="$WAIT_PIDS $WEB_UI_PID"
wait $WAIT_PIDS
