#!/bin/sh
set -e

echo "=== Phala CVM Declarative Environment ==="

# Verify repository was cloned by docker-compose
if [ ! -d "/app" ]; then
    echo "ERROR: /app directory not found. Repository should be cloned by docker-compose."
    exit 1
fi

cd /app

# Detect if running on macOS (Docker emulation) vs native Linux
# Seccomp/sandbox issues occur on macOS/ARM emulation
IS_MACOS_EMULATION=false

if [ -f /proc/version ] && grep -qi "darwin\|mac\|rosetta" /proc/version 2>/dev/null; then
    IS_MACOS_EMULATION=true
elif [ "$(uname -m)" = "aarch64" ] && [ -f /sys/devices/system/cpu/cpu0/regs/identification/midr_el1 ] 2>/dev/null; then
    # Check for Apple Silicon fingerprint in CPU identification
    IS_MACOS_EMULATION=true
elif [ -n "$DOCKER_HOST" ] && echo "$DOCKER_HOST" | grep -qi "darwin"; then
    IS_MACOS_EMULATION=true
fi

# Additional check: Rosetta leaves markers in /proc/sys
if [ "$IS_MACOS_EMULATION" = "false" ]; then
    if dmesg 2>/dev/null | grep -qi "rosetta\|apple"; then
        IS_MACOS_EMULATION=true
    fi
fi

# Set NIX_OPTIONS based on environment
if [ "$IS_MACOS_EMULATION" = "true" ]; then
    echo "Detected macOS/emulation environment - disabling nix sandbox and seccomp"
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
