#!/bin/sh
set -e

echo "=== Phala CVM NixOS Entrypoint ==="

# Install git if not present (needed to clone config)
if ! command -v git > /dev/null 2>&1; then
    echo "Installing git..."
    nix-env -iA nixpkgs.git
fi

# Clone NixOS configuration if not present
if [ ! -d "/etc/nixos/wrlx-nixos-config" ]; then
    echo "Cloning NixOS configuration..."
    cd /etc/nixos
    git clone https://github.com/yourusername/wrlx-nixos-config.git
    cd wrlx-nixos-config
else
    echo "NixOS configuration already present"
    cd /etc/nixos/wrlx-nixos-config
    git pull
fi

# Install necessary packages for VNC/GUI setup
echo "Installing VNC and GUI components..."
nix-env -iA nixpkgs.tigervnc nixpkgs.xorg.xorgserver nixpkgs.xfce.xfce4-session nixpkgs.xfce.xfce4-panel nixpkgs.xfce.thunar nixpkgs.openssh nixpkgs.procps nixpkgs.which

# Install noVNC
if ! command -v novnc > /dev/null 2>&1; then
    echo "Installing noVNC..."
    nix-env -iA nixpkgs.novnc
fi

# Set VNC password
VNC_PASSWORD=${VNC_PASSWORD:-"changeme"}
mkdir -p ~/.vnc
echo "$VNC_PASSWORD" | vncpasswd -f > ~/.vnc/passwd
chmod 600 ~/.vnc/passwd

# Start Xvfb (virtual framebuffer)
echo "Starting Xvfb..."
Xvfb :1 -screen 0 1920x1080x24 &
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

echo "=== CVM Services Started ==="
echo "SSH: port 2222"
echo "VNC: port 5900"
echo "noVNC: http://localhost:6080/vnc.html"
echo "VNC Password: $VNC_PASSWORD"
echo "==="

# Wait for all processes
wait $XVFB_PID $VNC_PID $NOVNC_PID $SSH_PID
