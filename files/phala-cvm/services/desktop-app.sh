#!/bin/sh
# CVM Desktop App Service
# Manages the Tauri desktop application (replaces web-ui)

. "$(dirname "$0")/common.sh"

DESKTOP_DIR="/app/cvm-agent/desktop"
# Workspace puts binaries in root target directory
DESKTOP_BIN="/app/cvm-agent/target/release/cvm-desktop"
DESKTOP_DEV_BIN="/app/cvm-agent/target/debug/cvm-desktop"
PIDFILE="/tmp/cvm-desktop.pid"

# Build the Tauri desktop app
build() {
    if [ ! -d "$DESKTOP_DIR" ]; then
        log_error "Desktop app source not found at $DESKTOP_DIR"
        return 1
    fi

    log_info "Building Tauri desktop app..."
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

    # Build frontend (required for Tauri - creates dist directory)
    log_info "Building frontend..."
    npm run build || {
        log_error "Frontend build failed"
        cd - > /dev/null
        return 1
    }

    # Build with cargo (release mode)
    log_info "Building Rust backend (this may take a few minutes)..."
    cd src-tauri || return 1
    if cargo build --release; then
        log_success "Desktop app built successfully"
        cd - > /dev/null
        return 0
    else
        log_error "Cargo build failed"
        cd - > /dev/null
        return 1
    fi
}

start() {
    if [ -f "$PIDFILE" ] && is_running "$(cat "$PIDFILE")"; then
        log_warn "Desktop app already running"
        return 0
    fi

    # Find the binary (prefer release, fall back to debug)
    local bin=""
    if [ -x "$DESKTOP_BIN" ]; then
        bin="$DESKTOP_BIN"
    elif [ -x "$DESKTOP_DEV_BIN" ]; then
        bin="$DESKTOP_DEV_BIN"
        log_warn "Using debug build"
    else
        log_warn "Desktop app not built, attempting to build..."
        if build; then
            # Try again after build
            if [ -x "$DESKTOP_BIN" ]; then
                bin="$DESKTOP_BIN"
            elif [ -x "$DESKTOP_DEV_BIN" ]; then
                bin="$DESKTOP_DEV_BIN"
            else
                log_error "Build succeeded but binary not found"
                return 1
            fi
        else
            log_error "Failed to build desktop app"
            return 1
        fi
    fi

    log_info "Starting CVM Desktop App..."

    # Ensure DISPLAY is set for X11
    export DISPLAY="${DISPLAY:-:1}"
    export AGENT_GRPC_ADDR="${AGENT_GRPC_ADDR:-http://localhost:8080}"

    # Disable WebKit hardware acceleration for Xvfb (software rendering)
    # Without this, WebKitGTK crashes with "Could not create default EGL display"
    export WEBKIT_DISABLE_COMPOSITING_MODE=1
    export WEBKIT_DISABLE_DMABUF_RENDERER=1

    "$bin" > /tmp/cvm-desktop.log 2>&1 &
    echo $! > "$PIDFILE"

    sleep 2
    if is_running "$(cat "$PIDFILE")"; then
        log_success "Desktop app started (PID $(cat "$PIDFILE"))"
        return 0
    else
        log_error "Desktop app failed to start. Check /tmp/cvm-desktop.log"
        return 1
    fi
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
