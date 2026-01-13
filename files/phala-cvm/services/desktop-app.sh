#!/bin/sh
# CVM Desktop App Service
# Manages the Tauri desktop application

. "$(dirname "$0")/common.sh"

DESKTOP_BIN="/app/cvm-agent/desktop/src-tauri/target/release/cvm-desktop"
DESKTOP_DEV_BIN="/app/cvm-agent/desktop/src-tauri/target/debug/cvm-desktop"
PIDFILE="/tmp/cvm-desktop.pid"

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
        log_error "Desktop app not built. Run: cd /app/cvm-agent/desktop && cargo tauri build"
        return 1
    fi

    log_info "Starting CVM Desktop App..."

    # Ensure DISPLAY is set for X11
    export DISPLAY="${DISPLAY:-:1}"
    export AGENT_GRPC_ADDR="${AGENT_GRPC_ADDR:-http://localhost:8080}"

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
    restart)
        stop
        sleep 1
        start
        ;;
    *)
        echo "Usage: $0 {start|stop|status|restart}"
        exit 1
        ;;
esac
