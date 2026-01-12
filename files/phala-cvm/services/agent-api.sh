#!/bin/sh
# CVM Agent API Service
# Builds and starts the Rust gRPC agent API

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/common.sh"

# Configuration
SERVICE_NAME="agent-api"
DEFAULT_PORT=8080
BUILD_DIR="${AGENT_API_DIR:-/app/cvm-agent/agent-api}"
BINARY="$BUILD_DIR/target/release/agent-api"

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
    cd "$BUILD_DIR"

    # Set OpenSSL environment for build
    export PKG_CONFIG_PATH=$(find /nix/store -name "pkgconfig" -type d 2>/dev/null | head -1):$PKG_CONFIG_PATH

    if cargo build --release; then
        log_success "Agent API built successfully"
        cd - > /dev/null
        return 0
    else
        log_error "Agent API build failed"
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

    # Build if binary doesn't exist
    if [ ! -x "$BINARY" ]; then
        build_agent || return 1
    fi

    if [ ! -x "$BINARY" ]; then
        log_error "Agent binary not found or not executable at $BINARY"
        return 1
    fi

    AGENT_PID=$(start_service "Agent API" "$BINARY" "$PORT")
    if [ $? -eq 0 ]; then
        echo "$AGENT_PID" > /tmp/agent-api.pid
        return 0
    fi
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
