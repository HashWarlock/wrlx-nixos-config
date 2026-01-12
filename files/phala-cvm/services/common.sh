#!/bin/sh
# Common utilities for CVM service management
# Source this file in service scripts: . "$(dirname "$0")/common.sh"

# Colors for output (if terminal supports it)
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    NC='\033[0m' # No Color
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    NC=''
fi

# Log functions
log_info() {
    echo "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo "${GREEN}[OK]${NC} $1"
}

log_warn() {
    echo "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo "${RED}[ERROR]${NC} $1"
}

# Check if a process is running by PID
is_running() {
    local pid=$1
    [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null
}

# Wait for a process to start (with timeout)
wait_for_start() {
    local pid=$1
    local name=$2
    local timeout=${3:-5}
    local count=0

    while [ $count -lt $timeout ]; do
        if is_running "$pid"; then
            return 0
        fi
        sleep 1
        count=$((count + 1))
    done
    return 1
}

# Wait for a port to be listening (with timeout)
wait_for_port() {
    local port=$1
    local timeout=${2:-10}
    local count=0

    while [ $count -lt $timeout ]; do
        if nc -z localhost "$port" 2>/dev/null; then
            return 0
        fi
        sleep 1
        count=$((count + 1))
    done
    return 1
}

# Start a service and track its PID
# Usage: start_service "name" "command" [port]
start_service() {
    local name=$1
    local cmd=$2
    local port=$3

    log_info "Starting $name..."

    eval "$cmd" &
    local pid=$!

    sleep 2

    if is_running "$pid"; then
        if [ -n "$port" ]; then
            log_success "$name started on port $port (PID $pid)"
        else
            log_success "$name started (PID $pid)"
        fi
        echo "$pid"
        return 0
    else
        log_error "$name failed to start"
        return 1
    fi
}

# Stop a service by PID
stop_service() {
    local name=$1
    local pid=$2

    if [ -n "$pid" ] && is_running "$pid"; then
        log_info "Stopping $name (PID $pid)..."
        kill "$pid" 2>/dev/null
        sleep 1
        if is_running "$pid"; then
            kill -9 "$pid" 2>/dev/null
        fi
        log_success "$name stopped"
    fi
}

# Parse INI-style config file
# Usage: get_config "section" "key" "default"
get_config() {
    local section=$1
    local key=$2
    local default=$3
    local config_file="${CONFIG_FILE:-/app/files/phala-cvm/config/services.conf}"

    if [ ! -f "$config_file" ]; then
        echo "$default"
        return
    fi

    local in_section=0
    while IFS= read -r line || [ -n "$line" ]; do
        # Remove leading/trailing whitespace
        line=$(echo "$line" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')

        # Skip empty lines and comments
        case "$line" in
            ''|'#'*) continue ;;
        esac

        # Check for section header
        case "$line" in
            "[$section]")
                in_section=1
                continue
                ;;
            '['*']')
                in_section=0
                continue
                ;;
        esac

        # If in the right section, look for the key
        if [ $in_section -eq 1 ]; then
            case "$line" in
                "$key="*)
                    echo "${line#*=}"
                    return
                    ;;
            esac
        fi
    done < "$config_file"

    echo "$default"
}

# Check if a service is enabled in config
is_service_enabled() {
    local service=$1
    local enabled=$(get_config "$service" "enabled" "true")
    [ "$enabled" = "true" ]
}

# Get service port from config
get_service_port() {
    local service=$1
    local default=$2
    get_config "$service" "port" "$default"
}

# Export functions for use in subshells
export -f log_info log_success log_warn log_error 2>/dev/null || true
export -f is_running wait_for_start wait_for_port 2>/dev/null || true
export -f start_service stop_service 2>/dev/null || true
export -f get_config is_service_enabled get_service_port 2>/dev/null || true
