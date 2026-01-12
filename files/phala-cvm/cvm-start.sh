#!/bin/sh
set -e

# =============================================================================
# Phala CVM NixOS Environment Startup Script
# =============================================================================
# This script initializes the CVM environment including:
# - Virtual display (Xvfb) and VNC server
# - D-Bus session for desktop integration
# - XFCE desktop environment
# - SSH server
# - Application services (Agent API, Web UI, Whisper)
#
# Services are managed via modular scripts in the services/ directory.
# Configuration is loaded from config/services.conf.
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SERVICES_DIR="$SCRIPT_DIR/services"
CONFIG_FILE="$SCRIPT_DIR/config/services.conf"
export CONFIG_FILE

# Source common utilities
. "$SERVICES_DIR/common.sh"

echo "=== Phala CVM NixOS Environment Setup ==="
echo "All packages loaded from flake.lock (deterministic)"

# Verify repository was cloned
if [ ! -d "/app" ]; then
    log_error "/app directory not found."
    exit 1
fi

log_info "Working directory: $(pwd)"
log_info "Repository at: /app"
log_info "Config file: $CONFIG_FILE"

# =============================================================================
# Display and VNC Setup
# =============================================================================

export DISPLAY=:1
VNC_PASSWORD=${VNC_PASSWORD:-"changeme"}
VNC_RESOLUTION=$(get_config "vnc" "resolution" "${VNC_RESOLUTION:-1920x1080}")

# Setup VNC password
mkdir -p ~/.vnc
echo "$VNC_PASSWORD" | vncpasswd -f > ~/.vnc/passwd
chmod 600 ~/.vnc/passwd
log_success "VNC password configured"

# Start Xvfb (virtual framebuffer)
log_info "Starting Xvfb with resolution ${VNC_RESOLUTION}..."
Xvfb :1 -screen 0 ${VNC_RESOLUTION}x24 &
XVFB_PID=$!
sleep 2

# =============================================================================
# D-Bus Setup
# =============================================================================

# Setup machine-id for D-Bus
log_info "Setting up machine-id..."
mkdir -p /var/lib/dbus /etc
if [ ! -f /var/lib/dbus/machine-id ]; then
    dbus-uuidgen --ensure=/var/lib/dbus/machine-id
fi
if [ ! -f /etc/machine-id ]; then
    ln -sf /var/lib/dbus/machine-id /etc/machine-id
fi

# Setup D-Bus session config
log_info "Setting up D-Bus configuration..."
mkdir -p /etc/dbus-1/session.d /etc/dbus-1/system.d
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
log_info "Starting D-Bus session..."
eval $(dbus-launch --sh-syntax)
export DBUS_SESSION_BUS_ADDRESS
export DBUS_SESSION_BUS_PID

# =============================================================================
# XFCE Desktop Environment
# =============================================================================

# Set XDG environment variables for XFCE
log_info "Setting up XDG environment variables..."
export XDG_DATA_DIRS="/nix/var/nix/profiles/default/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
export XDG_CONFIG_DIRS="/nix/var/nix/profiles/default/etc/xdg:${XDG_CONFIG_DIRS:-/etc/xdg}"

# Find and add XFCE session directory to XDG_CONFIG_DIRS
XFCE_SESSION_DIR=$(find /nix/store -name "xfce4-session-*" -type d 2>/dev/null | head -1)
if [ -n "$XFCE_SESSION_DIR" ]; then
    export XDG_CONFIG_DIRS="${XFCE_SESSION_DIR}/etc:${XDG_CONFIG_DIRS}"
    log_info "Added XFCE session directory: ${XFCE_SESSION_DIR}/etc"
fi

# Start xfconfd (XFCE configuration daemon) if available
if command -v xfconfd > /dev/null 2>&1; then
    log_info "Starting xfconfd..."
    xfconfd &
    sleep 1
fi

# Start window manager first (required for _NET_* properties)
log_info "Starting xfwm4 window manager..."
xfwm4 --daemon &
sleep 2

# Start desktop manager
log_info "Starting xfdesktop..."
xfdesktop &
sleep 1

# Start XFCE panel
log_info "Starting xfce4-panel..."
xfce4-panel &
sleep 1

# =============================================================================
# VNC Server
# =============================================================================

if is_service_enabled "vnc"; then
    VNC_PORT=$(get_service_port "vnc" "5900")
    log_info "Starting VNC server on display :1..."
    x0vncserver -display :1 -passwordfile ~/.vnc/passwd -rfbport $VNC_PORT &
    VNC_PID=$!
fi

# =============================================================================
# noVNC Web Client
# =============================================================================

if is_service_enabled "novnc"; then
    NOVNC_PORT=$(get_service_port "novnc" "6080")
    log_info "Starting noVNC on port $NOVNC_PORT..."
    novnc --vnc localhost:${VNC_PORT:-5900} --listen $NOVNC_PORT &
    NOVNC_PID=$!
fi

# =============================================================================
# SSH Server
# =============================================================================

if is_service_enabled "ssh"; then
    SSH_PORT=$(get_service_port "ssh" "22")
    log_info "Starting SSH daemon on port $SSH_PORT..."
    mkdir -p /run/sshd /etc/ssh

    # Generate host keys if they don't exist
    if [ ! -f /etc/ssh/ssh_host_rsa_key ]; then
        log_info "Generating SSH host keys..."
        ssh-keygen -A 2>/dev/null || true
    fi

    # Create sshd_config if it doesn't exist
    if [ ! -f /etc/ssh/sshd_config ]; then
        log_info "Creating sshd_config..."
        SFTP_SERVER=$(find /nix/store -name "sftp-server" 2>/dev/null | head -1)
        cat > /etc/ssh/sshd_config << EOFSSH
Port $SSH_PORT
PermitRootLogin no
PasswordAuthentication no
PubkeyAuthentication yes
UsePAM no
X11Forwarding yes
PrintMotd no
AcceptEnv LANG LC_*
Subsystem sftp ${SFTP_SERVER:-/usr/lib/openssh/sftp-server}
EOFSSH
    fi

    # Start SSH daemon
    SSHD_BIN=$(which sshd 2>/dev/null)
    if [ -n "$SSHD_BIN" ]; then
        $SSHD_BIN -D -e &
        SSH_PID=$!
    else
        log_warn "sshd not found in PATH"
    fi
fi

# =============================================================================
# Application Services (Modular)
# =============================================================================

log_info "Starting application services..."

# Start Agent API
"$SERVICES_DIR/agent-api.sh" start
AGENT_STARTED=$?

# Start Web UI
"$SERVICES_DIR/web-ui.sh" start
WEBUI_STARTED=$?

# Start Whisper
"$SERVICES_DIR/whisper.sh" start
WHISPER_STARTED=$?

# =============================================================================
# Startup Summary
# =============================================================================

echo ""
echo "=== CVM Services Started Successfully ==="
echo "Repository: ${GITHUB_REPO:-local}"
echo "Commit: ${GIT_COMMIT_HASH:-HEAD}"
echo ""
echo "Infrastructure:"
[ -n "$SSH_PID" ] && echo "  SSH: port $(get_service_port ssh 22) (mapped to host port 2222)"
echo "  VNC: port $(get_service_port vnc 5900)"
echo "  noVNC: http://localhost:$(get_service_port novnc 6080)/vnc.html"
echo "  Display Resolution: $VNC_RESOLUTION"
echo ""
echo "Application Services:"
[ $AGENT_STARTED -eq 0 ] && echo "  Agent API: http://localhost:$(get_service_port agent-api 8080) (gRPC)"
[ $WEBUI_STARTED -eq 0 ] && echo "  Web UI: http://localhost:$(get_service_port web-ui 3000)"
[ $WHISPER_STARTED -eq 0 ] && echo "  Whisper: http://localhost:$(get_service_port whisper 8082) (model: ${WHISPER_MODEL:-base})"
echo ""
[ -n "$AGENT_DOMAIN" ] && echo "Phala Cloud Domain: $AGENT_DOMAIN"
echo "==="

# =============================================================================
# Signal Handling and Cleanup
# =============================================================================

cleanup() {
    echo ""
    log_info "Shutting down CVM services..."

    # Stop application services
    "$SERVICES_DIR/whisper.sh" stop 2>/dev/null
    "$SERVICES_DIR/web-ui.sh" stop 2>/dev/null
    "$SERVICES_DIR/agent-api.sh" stop 2>/dev/null

    # Stop infrastructure services
    [ -n "$SSH_PID" ] && kill $SSH_PID 2>/dev/null
    [ -n "$NOVNC_PID" ] && kill $NOVNC_PID 2>/dev/null
    [ -n "$VNC_PID" ] && kill $VNC_PID 2>/dev/null
    [ -n "$XVFB_PID" ] && kill $XVFB_PID 2>/dev/null
    [ -n "$DBUS_SESSION_BUS_PID" ] && kill $DBUS_SESSION_BUS_PID 2>/dev/null

    log_success "All services stopped"
    exit 0
}
trap cleanup SIGTERM SIGINT

# =============================================================================
# Wait for Services
# =============================================================================

# Wait for infrastructure processes
WAIT_PIDS="$XVFB_PID"
[ -n "$VNC_PID" ] && WAIT_PIDS="$WAIT_PIDS $VNC_PID"
[ -n "$NOVNC_PID" ] && WAIT_PIDS="$WAIT_PIDS $NOVNC_PID"
[ -n "$SSH_PID" ] && WAIT_PIDS="$WAIT_PIDS $SSH_PID"

wait $WAIT_PIDS
