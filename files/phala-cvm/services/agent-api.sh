#!/bin/sh
# CVM Agent API Service
# Builds and starts the Rust gRPC agent API

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/common.sh"

# Configuration
SERVICE_NAME="agent-api"
DEFAULT_PORT=8080
BUILD_DIR="${AGENT_API_DIR:-/app/cvm-agent/agent-api}"
# Workspace puts target in workspace root, not member directory
WORKSPACE_DIR="${WORKSPACE_DIR:-/app/cvm-agent}"
BINARY="$WORKSPACE_DIR/target/release/agent-api"

# Get port from config or environment
PORT=$(get_service_port "$SERVICE_NAME" "$DEFAULT_PORT")
export AGENT_LISTEN_ADDR="0.0.0.0:$PORT"

# Build the agent API
build_agent() {
    if [ ! -d "$BUILD_DIR" ]; then
        log_error "Agent API source not found at $BUILD_DIR"
        return 1
    fi

    log_info "Building Agent API..."
    log_info "Build directory: $BUILD_DIR"

    cd "$BUILD_DIR"

    # Verify build tools are available (should be provided by nix develop)
    if ! command -v cargo > /dev/null 2>&1; then
        log_error "cargo not found in PATH. Ensure you're running inside 'nix develop .#cvm'"
        cd - > /dev/null
        return 1
    fi

    if ! command -v rustc > /dev/null 2>&1; then
        log_error "rustc not found in PATH. Ensure you're running inside 'nix develop .#cvm'"
        cd - > /dev/null
        return 1
    fi

    log_info "Using rustc: $(rustc --version)"
    log_info "Using cargo: $(cargo --version)"

    # PKG_CONFIG_PATH should already be set by nix develop shell
    # Only add OpenSSL path if not already configured
    if [ -z "$PKG_CONFIG_PATH" ]; then
        log_warn "PKG_CONFIG_PATH not set, attempting to find OpenSSL..."
        OPENSSL_PKG=$(find /nix/store -maxdepth 2 -name "openssl-*" -type d 2>/dev/null | grep -v "\-dev$" | head -1)
        if [ -n "$OPENSSL_PKG" ] && [ -d "$OPENSSL_PKG/lib/pkgconfig" ]; then
            export PKG_CONFIG_PATH="$OPENSSL_PKG/lib/pkgconfig"
            log_info "Set PKG_CONFIG_PATH=$PKG_CONFIG_PATH"
        fi
    else
        log_info "PKG_CONFIG_PATH already set: $PKG_CONFIG_PATH"
    fi

    # Run cargo build with verbose output on failure
    log_info "Running cargo build --release..."
    if cargo build --release 2>&1; then
        log_success "Agent API built successfully"

        # Find the actual binary - Cargo may use hyphens or underscores
        # Workspace builds output to workspace root target directory
        ACTUAL_BINARY=""
        for name in "agent-api" "agent_api"; do
            if [ -f "$WORKSPACE_DIR/target/release/$name" ]; then
                ACTUAL_BINARY="$WORKSPACE_DIR/target/release/$name"
                break
            fi
        done

        if [ -z "$ACTUAL_BINARY" ]; then
            log_error "Binary not found after successful build"
            log_info "Checking workspace target/release directory..."
            ls -la "$WORKSPACE_DIR/target/release/" 2>&1 | head -20
            cd - > /dev/null
            return 1
        fi

        # Make executable if needed
        chmod +x "$ACTUAL_BINARY" 2>/dev/null || true

        # Update BINARY variable for later use
        BINARY="$ACTUAL_BINARY"
        export BINARY
        log_info "Binary location: $BINARY"

        cd - > /dev/null
        return 0
    else
        log_error "Agent API build failed. Cargo output above."
        log_error "Try running manually: cd $BUILD_DIR && cargo build --release"
        cd - > /dev/null
        return 1
    fi
}

# Start the agent API service
start() {
    if ! is_service_enabled "$SERVICE_NAME"; then
        log_info "Agent API is disabled in config"
        return 0
    fi

    # Build if binary doesn't exist or isn't executable
    if [ ! -x "$BINARY" ]; then
        log_info "Agent API binary not found, building..."
        if ! build_agent; then
            log_error "Agent API build failed - service will not start"
            log_error "Try running manually: nix develop .#cvm -c cargo build --release"
            return 1
        fi
    fi

    # Verify binary exists after build
    if [ ! -x "$BINARY" ]; then
        log_error "Agent binary not found or not executable at $BINARY"
        log_error "Build appeared to succeed but binary is missing"
        return 1
    fi

    log_info "Starting Agent API from: $BINARY"
    local pid
    pid=$(start_service "Agent API" "$BINARY" "$PORT")
    local ret=$?
    if [ $ret -eq 0 ] && [ -n "$pid" ]; then
        echo "$pid" > /tmp/agent-api.pid
        return 0
    fi
    log_error "start_service returned $ret with pid='$pid'"
    return 1
}

# Stop the agent API service
stop() {
    if [ -f /tmp/agent-api.pid ]; then
        local pid=$(cat /tmp/agent-api.pid)
        stop_service "Agent API" "$pid"
        rm -f /tmp/agent-api.pid
    fi
}

# Check service status
status() {
    if [ -f /tmp/agent-api.pid ]; then
        local pid=$(cat /tmp/agent-api.pid)
        if is_running "$pid"; then
            log_success "Agent API is running (PID $pid, port $PORT)"
            return 0
        fi
    fi
    log_warn "Agent API is not running"
    return 1
}

# Main command dispatcher
case "${1:-start}" in
    start)
        start
        ;;
    stop)
        stop
        ;;
    restart)
        stop
        sleep 1
        start
        ;;
    status)
        status
        ;;
    build)
        build_agent
        ;;
    *)
        echo "Usage: $0 {start|stop|restart|status|build}"
        exit 1
        ;;
esac
