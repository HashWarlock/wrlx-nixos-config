#!/bin/sh
# CVM Whisper Service
# Starts the Whisper voice transcription server

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/common.sh"

# Configuration
SERVICE_NAME="whisper"
DEFAULT_PORT=8082
MODEL="${WHISPER_MODEL:-base}"

# Get port from config or environment
PORT=$(get_service_port "$SERVICE_NAME" "$DEFAULT_PORT")

# Find the Whisper model
find_model() {
    local model_pattern="/nix/store/*/share/whisper-cpp/models/ggml-${MODEL}.bin"
    local found=$(ls $model_pattern 2>/dev/null | head -1)
    echo "$found"
}

# Start the Whisper server
start() {
    if ! is_service_enabled "$SERVICE_NAME"; then
        log_info "Whisper is disabled in config"
        return 0
    fi

    # Check if whisper-server is available
    if ! command -v whisper-server > /dev/null 2>&1; then
        log_warn "whisper-server not found, voice input disabled"
        return 0
    fi

    # Find model
    local model_path=$(find_model)
    if [ -z "$model_path" ]; then
        log_warn "Whisper model '$MODEL' not found, voice input disabled"
        log_info "Available models can be configured via WHISPER_MODEL env var"
        return 0
    fi

    log_info "Using Whisper model: $MODEL"
    WHISPER_PID=$(start_service "Whisper" "whisper-server --model $model_path --port $PORT" "$PORT")
    if [ $? -eq 0 ]; then
        echo "$WHISPER_PID" > /tmp/whisper.pid
        return 0
    fi
    return 1
}

# Stop the Whisper server
stop() {
    if [ -f /tmp/whisper.pid ]; then
        local pid=$(cat /tmp/whisper.pid)
        stop_service "Whisper" "$pid"
        rm -f /tmp/whisper.pid
    fi
}

# Check service status
status() {
    if [ -f /tmp/whisper.pid ]; then
        local pid=$(cat /tmp/whisper.pid)
        if is_running "$pid"; then
            log_success "Whisper is running (PID $pid, port $PORT, model: $MODEL)"
            return 0
        fi
    fi
    log_warn "Whisper is not running"
    return 1
}

# List available models
list_models() {
    log_info "Available Whisper models:"
    ls /nix/store/*/share/whisper-cpp/models/ggml-*.bin 2>/dev/null | \
        sed 's|.*/ggml-||;s|\.bin||' | sort -u | while read model; do
        if [ "$model" = "$MODEL" ]; then
            echo "  * $model (current)"
        else
            echo "    $model"
        fi
    done
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
    models)
        list_models
        ;;
    *)
        echo "Usage: $0 {start|stop|restart|status|models}"
        exit 1
        ;;
esac
