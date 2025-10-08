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
nix-env -iA nixpkgs.tigervnc nixpkgs.xorg.xorgserver nixpkgs.xfce.xfce4-session nixpkgs.xfce.xfce4-panel nixpkgs.xfce.thunar nixpkgs.openssh nixpkgs.procps nixpkgs.which nixpkgs.bash

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

# Start XFCE session
echo "Starting XFCE session..."
xfce4-session &
sleep 3

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
mkdir -p /run/sshd
/nix/store/$(ls /nix/store | grep openssh | head -n1)/bin/sshd -D -e &
SSH_PID=$!

echo "=== CVM Services Started Successfully ==="
echo "Repository: $GITHUB_REPO"
echo "Commit: $GIT_COMMIT_HASH"
echo "SSH: port 22 (mapped to host port 2222)"
echo "VNC: port 5900"
echo "noVNC: http://localhost:6080/vnc.html"
echo "Display Resolution: $VNC_RESOLUTION"
if [ -n "$AGENT_DOMAIN" ]; then
    echo "Phala Cloud Domain: $AGENT_DOMAIN"
fi
echo "==="

# Trap signals for graceful shutdown
trap 'echo "Shutting down..."; kill $XVFB_PID $VNC_PID $NOVNC_PID $SSH_PID 2>/dev/null; exit 0' SIGTERM SIGINT

# Wait for all processes
wait $XVFB_PID $VNC_PID $NOVNC_PID $SSH_PID
