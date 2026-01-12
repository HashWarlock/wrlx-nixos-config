#!/bin/sh
# CVM Web UI Service
# Serves the static web UI files

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/common.sh"

# Configuration
SERVICE_NAME="web-ui"
DEFAULT_PORT=3000
SERVE_DIR="${WEB_UI_DIR:-/app/cvm-agent/web-ui/dist}"
BUILD_DIR="${WEB_UI_BUILD_DIR:-/app/cvm-agent/web-ui}"

# Get port from config or environment
PORT=$(get_service_port "$SERVICE_NAME" "$DEFAULT_PORT")

# Build the web UI
build_webui() {
    if [ ! -d "$BUILD_DIR" ]; then
        log_error "Web UI source not found at $BUILD_DIR"
        return 1
    fi

    log_info "Building Web UI..."
    cd "$BUILD_DIR"

    # Check if npm is available
    if ! command -v npm > /dev/null 2>&1; then
        log_error "npm not found - cannot build Web UI"
        cd - > /dev/null
        return 1
    fi

    # Install dependencies if needed
    if [ ! -d "node_modules" ]; then
        log_info "Installing npm dependencies..."
        npm install || {
            log_error "npm install failed"
            cd - > /dev/null
            return 1
        }
    fi

    # Build
    if npm run build; then
        log_success "Web UI built successfully"
        cd - > /dev/null
        return 0
    else
        log_error "Web UI build failed"
        cd - > /dev/null
        return 1
    fi
}

# Start the web UI server
start() {
    if ! is_service_enabled "$SERVICE_NAME"; then
        log_info "Web UI is disabled in config"
        return 0
    fi

    if [ ! -d "$SERVE_DIR" ]; then
        log_warn "Web UI dist not found at $SERVE_DIR"
        log_info "Attempting to build Web UI..."
        build_webui || {
            log_error "Run 'npm run build' in the web-ui directory to build it"
            return 1
        }
    fi

    if [ ! -d "$SERVE_DIR" ]; then
        log_error "Web UI dist still not found after build"
        return 1
    fi

    cd "$SERVE_DIR"
    WEB_UI_PID=$(start_service "Web UI" "python3 -m http.server $PORT --bind 0.0.0.0" "$PORT")
    if [ $? -eq 0 ]; then
        echo "$WEB_UI_PID" > /tmp/web-ui.pid
        cd - > /dev/null
        return 0
    fi
    cd - > /dev/null
    return 1
}

# Stop the web UI server
stop() {
    if [ -f /tmp/web-ui.pid ]; then
        local pid=$(cat /tmp/web-ui.pid)
        stop_service "Web UI" "$pid"
        rm -f /tmp/web-ui.pid
    fi
}

# Check service status
status() {
    if [ -f /tmp/web-ui.pid ]; then
        local pid=$(cat /tmp/web-ui.pid)
        if is_running "$pid"; then
            log_success "Web UI is running (PID $pid, port $PORT)"
            return 0
        fi
    fi
    log_warn "Web UI is not running"
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
        build_webui
        ;;
    *)
        echo "Usage: $0 {start|stop|restart|status|build}"
        exit 1
        ;;
esac
