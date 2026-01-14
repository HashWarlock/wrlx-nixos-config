#!/bin/sh
# CVM Desktop App Service
# Serves the frontend via HTTP (alternative to Tauri for headless environments)

. "$(dirname "$0")/common.sh"

DESKTOP_DIR="/app/cvm-agent/desktop"
PIDFILE="/tmp/cvm-desktop.pid"
WEB_PORT=3000

# Check if we should use Tauri (native) or HTTP server mode
# Default to HTTP server for Xvfb/headless environments
USE_TAURI="${USE_TAURI:-false}"

# Build the frontend
build() {
    if [ ! -d "$DESKTOP_DIR" ]; then
        log_error "Desktop app source not found at $DESKTOP_DIR"
        return 1
    fi

    log_info "Building frontend..."
    cd "$DESKTOP_DIR" || return 1

    # Install frontend dependencies if needed
    if [ ! -d "node_modules" ]; then
        log_info "Installing npm dependencies..."
        npm install || {
            log_error "npm install failed"
            cd - > /dev/null
            return 1
        }
    fi

    # Build frontend (creates dist directory)
    npm run build || {
        log_error "Frontend build failed"
        cd - > /dev/null
        return 1
    }

    log_success "Frontend built successfully"
    cd - > /dev/null
    return 0
}

start_http_server() {
    log_info "Starting HTTP server for frontend on port $WEB_PORT..."

    cd "$DESKTOP_DIR/dist" || {
        log_warn "dist directory not found, building frontend first..."
        build || return 1
        cd "$DESKTOP_DIR/dist" || return 1
    }

    # Use Python's built-in HTTP server (available in the nix shell)
    python3 -m http.server $WEB_PORT > /tmp/cvm-desktop.log 2>&1 &
    echo $! > "$PIDFILE"

    sleep 2
    if is_running "$(cat "$PIDFILE")"; then
        log_success "Frontend server started on http://localhost:$WEB_PORT (PID $(cat "$PIDFILE"))"

        # Open Firefox to the URL automatically (if DISPLAY is set)
        if [ -n "$DISPLAY" ]; then
            log_info "Opening Firefox to http://localhost:$WEB_PORT..."
            firefox "http://localhost:$WEB_PORT" > /dev/null 2>&1 &
        fi

        return 0
    else
        log_error "Frontend server failed to start. Check /tmp/cvm-desktop.log"
        return 1
    fi
}

start_tauri() {
    # Original Tauri approach - requires GPU/EGL
    DESKTOP_BIN="/app/cvm-agent/target/release/cvm-desktop"
    DESKTOP_DEV_BIN="/app/cvm-agent/target/debug/cvm-desktop"

    # Find the binary
    local bin=""
    if [ -x "$DESKTOP_BIN" ]; then
        bin="$DESKTOP_BIN"
    elif [ -x "$DESKTOP_DEV_BIN" ]; then
        bin="$DESKTOP_DEV_BIN"
        log_warn "Using debug build"
    else
        log_warn "Tauri binary not found, falling back to HTTP server..."
        return 1
    fi

    log_info "Starting CVM Desktop App (Tauri)..."

    # Environment for X11 and gRPC
    export DISPLAY="${DISPLAY:-:1}"
    export AGENT_GRPC_ADDR="${AGENT_GRPC_ADDR:-http://localhost:8080}"

    # Try to force software rendering
    export LIBGL_ALWAYS_SOFTWARE=1
    export WEBKIT_DISABLE_COMPOSITING_MODE=1
    export WEBKIT_DISABLE_DMABUF_RENDERER=1

    "$bin" > /tmp/cvm-desktop.log 2>&1 &
    echo $! > "$PIDFILE"

    sleep 3
    if is_running "$(cat "$PIDFILE")"; then
        log_success "Desktop app started (PID $(cat "$PIDFILE"))"
        return 0
    else
        log_warn "Tauri app crashed, check /tmp/cvm-desktop.log"
        return 1
    fi
}

start() {
    if [ -f "$PIDFILE" ] && is_running "$(cat "$PIDFILE")"; then
        log_warn "Desktop app already running"
        return 0
    fi

    # Ensure DISPLAY is set
    export DISPLAY="${DISPLAY:-:1}"
    export AGENT_GRPC_ADDR="${AGENT_GRPC_ADDR:-http://localhost:8080}"

    if [ "$USE_TAURI" = "true" ]; then
        # Try Tauri first, fall back to HTTP server if it fails
        if start_tauri; then
            return 0
        fi
        log_info "Falling back to HTTP server mode..."
    fi

    # Use HTTP server mode (works in headless/Xvfb environments)
    start_http_server
}

stop() {
    if [ -f "$PIDFILE" ]; then
        local pid=$(cat "$PIDFILE")
        if is_running "$pid"; then
            log_info "Stopping Desktop app (PID $pid)..."
            kill "$pid" 2>/dev/null
            sleep 1
            if is_running "$pid"; then
                kill -9 "$pid" 2>/dev/null
            fi
            log_success "Desktop app stopped"
        fi
        rm -f "$PIDFILE"
    fi
}

status() {
    if [ -f "$PIDFILE" ] && is_running "$(cat "$PIDFILE")"; then
        echo "running"
        return 0
    else
        echo "stopped"
        return 1
    fi
}

case "$1" in
    start)  start ;;
    stop)   stop ;;
    status) status ;;
    build)  build ;;
    restart)
        stop
        sleep 1
        start
        ;;
    *)
        echo "Usage: $0 {start|stop|status|restart|build}"
        exit 1
        ;;
esac
