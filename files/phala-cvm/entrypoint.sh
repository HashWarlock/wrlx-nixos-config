#!/bin/sh
set -e

echo "=== Phala CVM Declarative Environment ==="

# Verify repository was cloned by docker-compose
if [ ! -d "/app" ]; then
    echo "ERROR: /app directory not found. Repository should be cloned by docker-compose."
    exit 1
fi

cd /app

# Detect if we're in a Docker container
# Docker containers already provide isolation, so nix sandbox is redundant
# and can cause seccomp issues especially on Docker Desktop (macOS/Windows)
IS_DOCKER_CONTAINER=false

# Check for Docker container indicators
if [ -f /.dockerenv ]; then
    IS_DOCKER_CONTAINER=true
elif grep -sq docker /proc/1/cgroup 2>/dev/null; then
    IS_DOCKER_CONTAINER=true
elif grep -sq docker /proc/self/cgroup 2>/dev/null; then
    IS_DOCKER_CONTAINER=true
fi

# Also allow explicit override via environment variable
if [ "$DISABLE_NIX_SANDBOX" = "true" ]; then
    IS_DOCKER_CONTAINER=true
fi

# Set NIX_OPTIONS based on environment
if [ "$IS_DOCKER_CONTAINER" = "true" ]; then
    echo "Detected Docker container - disabling nix sandbox (container provides isolation)"
    NIX_OPTIONS="--option sandbox false --option filter-syscalls false"
else
    echo "Detected native Linux environment - using default nix options"
    NIX_OPTIONS=""
fi

echo "Using flake-locked packages from flake.lock..."
echo "All package versions are deterministic and reproducible."

# Ensure nix experimental features are enabled
mkdir -p ~/.config/nix
echo "experimental-features = nix-command flakes" > ~/.config/nix/nix.conf

# Enter the CVM development shell and run the startup script
# This loads all packages from the locked nixpkgs version (nixos-25.11)
# --accept-flake-config trusts the flake's extra-substituters without prompting
# --verbose shows download progress
echo "Starting nix develop (this may take several minutes on first run)..."
exec nix develop --accept-flake-config --verbose $NIX_OPTIONS .#cvm --command /app/files/phala-cvm/cvm-start.sh
