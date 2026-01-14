# CVM Service Startup Fixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix the service startup blocking issue so that Web UI and Whisper services start successfully after Agent API.

**Architecture:** The CVM uses a startup script (cvm-start.sh) that orchestrates service startup via modular service scripts. The `start_service()` helper function in `common.sh` backgrounds processes but currently has a bug that corrupts PID capture. Additionally, build steps for agent-api and web-ui may fail silently in the container environment.

**Tech Stack:** Shell scripts (POSIX sh), Docker, NixOS, Rust/Cargo, Node.js/npm

---

## Root Cause Analysis

From E2E testing on 2026-01-12, we discovered:

| Symptom | Root Cause |
|---------|------------|
| Web UI never starts | `start_service()` logs to stdout, corrupting PID capture in command substitution |
| Whisper never starts | Same issue - services downstream from first failure don't run |
| Container logs show "Starting Agent API from..." but no "Starting Web UI service..." | `set -e` in cvm-start.sh causes exit on first failure |

### Technical Details

1. **common.sh `start_service()` bug** (lines 78-102):
   ```sh
   AGENT_PID=$(start_service "Agent API" "$BINARY" "$PORT")
   ```
   The `log_info` and `log_success` calls inside `start_service` echo to stdout, which gets captured along with the PID. Result: `AGENT_PID` contains "[INFO] Starting...\n[OK] Started...\n1234" instead of just "1234".

2. **Missing pre-built artifacts**: Both cargo (Rust) and npm (Node.js) builds happen at container startup, potentially failing due to missing dependencies or taking too long.

---

## Task 1: Fix common.sh Log Functions to Use stderr

**Files:**
- Modify: `files/phala-cvm/services/common.sh:21-35`
- Test: Manual verification via container restart

**Step 1: Read current log functions**

Verify current implementation echoes to stdout:
```sh
log_info() {
    echo "${BLUE}[INFO]${NC} $1"
}
```

**Step 2: Modify log functions to write to stderr**

Change all log functions to redirect to stderr so they don't pollute command substitution captures:

```sh
log_info() {
    echo "${BLUE}[INFO]${NC} $1" >&2
}

log_success() {
    echo "${GREEN}[OK]${NC} $1" >&2
}

log_warn() {
    echo "${YELLOW}[WARN]${NC} $1" >&2
}

log_error() {
    echo "${RED}[ERROR]${NC} $1" >&2
}
```

**Step 3: Verify the edit**

Run: `grep -n ">&2" files/phala-cvm/services/common.sh`
Expected: All 4 log functions should have `>&2` redirect

**Step 4: Commit**

```bash
git add files/phala-cvm/services/common.sh
git commit -m "fix(cvm): redirect log functions to stderr

Fixes command substitution capturing log messages along with
return values, which corrupted PID files and broke service
startup sequence."
```

---

## Task 2: Add Pre-Build Step for Agent API

**Files:**
- Modify: `files/phala-cvm/services/agent-api.sh:103-130`
- Test: Container restart and verify agent-api starts

**Step 1: Read current start function**

Current behavior: Builds if binary doesn't exist, then starts. Problem: Build may fail silently or take too long.

**Step 2: Add explicit build failure handling**

Modify the `start()` function to log build output and handle failures explicitly:

```sh
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
```

**Step 3: Verify the edit**

Run: `grep -A5 "start()" files/phala-cvm/services/agent-api.sh`
Expected: New start function with explicit error handling

**Step 4: Commit**

```bash
git add files/phala-cvm/services/agent-api.sh
git commit -m "fix(cvm): improve agent-api build failure handling

- Add explicit logging when build fails
- Capture start_service return code properly
- Provide manual build command in error message"
```

---

## Task 3: Add Pre-Build Step for Web UI

**Files:**
- Modify: `files/phala-cvm/services/web-ui.sh:56-86`
- Test: Container restart and verify web-ui starts

**Step 1: Read current start function**

Current behavior: Checks for dist directory, builds if missing, then starts.

**Step 2: Improve error handling and logging**

```sh
start() {
    if ! is_service_enabled "$SERVICE_NAME"; then
        log_info "Web UI is disabled in config"
        return 0
    fi

    # Check for pre-built dist
    if [ ! -d "$SERVE_DIR" ]; then
        log_warn "Web UI dist not found at $SERVE_DIR"
        if command -v npm > /dev/null 2>&1; then
            log_info "Attempting to build Web UI..."
            if ! build_webui; then
                log_error "Web UI build failed"
                log_error "Try running manually: cd $BUILD_DIR && npm install && npm run build"
                return 1
            fi
        else
            log_error "npm not available and dist not pre-built"
            log_error "Pre-build the Web UI or ensure npm is in PATH"
            return 1
        fi
    fi

    if [ ! -d "$SERVE_DIR" ]; then
        log_error "Web UI dist still not found after build attempt"
        return 1
    fi

    cd "$SERVE_DIR" || return 1
    local pid
    pid=$(start_service "Web UI" "python3 -m http.server $PORT --bind 0.0.0.0" "$PORT")
    local ret=$?
    cd - > /dev/null || true

    if [ $ret -eq 0 ] && [ -n "$pid" ]; then
        echo "$pid" > /tmp/web-ui.pid
        return 0
    fi
    log_error "start_service returned $ret with pid='$pid'"
    return 1
}
```

**Step 3: Verify the edit**

Run: `grep -A20 "^start()" files/phala-cvm/services/web-ui.sh`
Expected: New start function with improved error handling

**Step 4: Commit**

```bash
git add files/phala-cvm/services/web-ui.sh
git commit -m "fix(cvm): improve web-ui build failure handling

- Add explicit logging when dist directory missing
- Check for npm availability before attempting build
- Provide manual build command in error message"
```

---

## Task 4: Remove set -e from cvm-start.sh

**Files:**
- Modify: `files/phala-cvm/cvm-start.sh:2`
- Test: Container restart, verify all services attempt to start

**Step 1: Read current shebang and set options**

```sh
#!/bin/sh
set -e
```

**Step 2: Remove set -e**

The `set -e` causes the entire script to exit on the first command failure. This prevents subsequent services from starting if one fails. Remove it to allow graceful degradation.

Change line 2 from:
```sh
set -e
```
To:
```sh
# Note: set -e intentionally removed to allow graceful degradation
# Individual service failures are logged but don't stop other services
```

**Step 3: Verify the edit**

Run: `head -5 files/phala-cvm/cvm-start.sh`
Expected: No `set -e` on line 2

**Step 4: Commit**

```bash
git add files/phala-cvm/cvm-start.sh
git commit -m "fix(cvm): remove set -e to allow graceful service degradation

Individual service failures are already logged. Removing set -e
allows remaining services to start even if one fails, improving
the user experience and debugging capability."
```

---

## Task 5: Test All Fixes

**Files:**
- None (testing only)
- Test: Container restart and Playwright MCP verification

**Step 1: Rebuild and restart container**

```bash
cd files/phala-cvm
docker-compose down
docker-compose up -d
```

**Step 2: Wait for services to initialize**

```bash
sleep 30
```

**Step 3: Check container logs for service startup**

```bash
docker-compose logs | grep -E "(Starting|started|failed)"
```

Expected output should show:
- "Starting Agent API service..."
- "Agent API started successfully"
- "Starting Web UI service..."
- "Web UI started successfully"
- "Starting Whisper service..."
- "Whisper started successfully" (or "Whisper model not found" warning)

**Step 4: Test port connectivity**

```bash
# Agent API (gRPC)
nc -z localhost 8080 && echo "Agent API: OK" || echo "Agent API: FAIL"

# Web UI
nc -z localhost 3000 && echo "Web UI: OK" || echo "Web UI: FAIL"

# noVNC
nc -z localhost 6080 && echo "noVNC: OK" || echo "noVNC: FAIL"
```

**Step 5: Test gRPC health endpoint**

```bash
grpcurl -plaintext \
  -import-path cvm-agent/proto \
  -proto agent.proto \
  localhost:8080 cvm.agent.HealthService.Check
```

Expected: `{"healthy": true}`

**Step 6: Test Web UI with Playwright MCP**

Use browser_navigate to `http://localhost:3000` and verify page loads.

**Step 7: Commit test results**

Update `progress.md` with test results.

---

## Task 6: Document Changes

**Files:**
- Modify: `files/phala-cvm/README.md` (if exists)
- Create: `files/phala-cvm/TROUBLESHOOTING.md`

**Step 1: Create troubleshooting guide**

```markdown
# CVM Troubleshooting Guide

## Service Startup Issues

### Agent API doesn't start
1. Check if cargo is available: `which cargo`
2. Try manual build: `cd /app/cvm-agent/agent-api && cargo build --release`
3. Check logs: `docker-compose logs | grep -i agent`

### Web UI doesn't start
1. Check if dist exists: `ls /app/cvm-agent/web-ui/dist`
2. Try manual build: `cd /app/cvm-agent/web-ui && npm install && npm run build`
3. Check logs: `docker-compose logs | grep -i web-ui`

### Whisper doesn't start
1. Check if model exists: `ls /nix/store/*/share/whisper-cpp/models/`
2. Set model: `export WHISPER_MODEL=base`
3. Check logs: `docker-compose logs | grep -i whisper`

## Port Conflicts
Default ports:
- 3000: Web UI
- 6080: noVNC
- 8080: Agent API (gRPC)
- 8082: Whisper
- 5900: VNC
- 2222: SSH (mapped from 22)
```

**Step 2: Commit documentation**

```bash
git add files/phala-cvm/TROUBLESHOOTING.md
git commit -m "docs(cvm): add troubleshooting guide for service startup"
```

---

## Verification Checklist

After all tasks are complete, verify:

- [ ] Container starts without errors
- [ ] Agent API responds on port 8080
- [ ] Web UI loads on port 3000
- [ ] noVNC connects on port 6080
- [ ] Whisper responds on port 8082 (if model available)
- [ ] All gRPC endpoints tested in E2E scenarios work
- [ ] Screenshots captured via Playwright MCP

---

## Rollback Plan

If fixes cause new issues:

1. Revert commits: `git revert HEAD~N` (N = number of commits)
2. Rebuild container: `docker-compose down && docker-compose build --no-cache`
3. Test original behavior
