# Tauri Native App Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace web-ui + Caddy reverse proxy with a native Tauri desktop app that directly calls the Agent API via gRPC and responds to a global hotkey (Ctrl+Shift+Space).

**Architecture:** Tauri (Rust backend + WebView) provides a native desktop app that embeds React UI and makes direct gRPC calls to the Agent API at localhost:8080. No browser or reverse proxy needed.

**Tech Stack:** Tauri 2.x, Rust, React (existing web-ui migrated), gRPC (tonic), WebKit2GTK

---

## Prerequisites

Before starting:
1. Ensure the CVM container is running: `docker-compose up -d`
2. Verify Agent API is accessible: `grpcurl -plaintext localhost:8080 list`
3. Understand gRPC protobuf definitions: `cvm-agent/proto/agent.proto`

---

## Task 1: Add Tauri Dependencies to Devshell

**Files:**
- Modify: `flake.nix:129-153` (devShells.cvm packages section)

**Context:** The devshell provides all build tools inside the Docker container. We need to add Tauri's native dependencies (webkit2gtk, gtk3, etc.) for building the desktop app.

**Step 1: Read current devshell packages**

Run: `nix eval --json .#devShells.x86_64-linux.cvm.packages 2>/dev/null | jq '.' || echo "Eval not available, check flake.nix directly"`

Verify these exist: rustc, cargo, gcc, pkg-config, openssl, nodejs, npm

**Step 2: Edit flake.nix to add Tauri dependencies**

Add these packages after the protobuf line in devShells.cvm.packages:

```nix
        # Tauri 2.x native dependencies
        webkitgtk_4_1  # WebView for Tauri
        gtk3
        libsoup_3
        glib
        glib-networking  # TLS support
        gsettings-desktop-schemas
        libayatana-appindicator  # System tray
```

**Step 3: Verify the flake evaluates**

Run: `nix flake check --no-build 2>&1 | head -20`
Expected: No errors related to unknown packages

**Step 4: Test devshell enters**

Run: `nix develop .#cvm --command bash -c "echo 'Devshell works'"`
Expected: `Devshell works`

**Step 5: Commit**

```bash
git add flake.nix
git commit -m "feat(devshell): add Tauri native dependencies

Add webkit2gtk, gtk3, libsoup, and system tray support for building
Tauri desktop applications in the CVM devshell."
```

---

## Task 2: Initialize Tauri Project Structure

**Files:**
- Create: `cvm-agent/desktop/src-tauri/Cargo.toml`
- Create: `cvm-agent/desktop/src-tauri/tauri.conf.json`
- Create: `cvm-agent/desktop/src-tauri/build.rs`
- Create: `cvm-agent/desktop/src-tauri/src/main.rs` (minimal)
- Create: `cvm-agent/desktop/package.json`
- Modify: `cvm-agent/Cargo.toml` (add desktop to workspace)

**Context:** We're creating a new Tauri project that will embed the existing React UI. The project structure follows Tauri 2.x conventions.

**Step 1: Create directory structure**

Run:
```bash
mkdir -p cvm-agent/desktop/src-tauri/src
mkdir -p cvm-agent/desktop/src-tauri/icons
```

**Step 2: Create Tauri Cargo.toml**

Create `cvm-agent/desktop/src-tauri/Cargo.toml`:

```toml
[package]
name = "cvm-desktop"
version = "0.1.0"
edition = "2021"
description = "CVM Agent Desktop Application"
license = "MIT"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-shell = "2"
tauri-plugin-global-shortcut = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tonic = "0.12"
prost = "0.13"

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

**Step 3: Create build.rs**

Create `cvm-agent/desktop/src-tauri/build.rs`:

```rust
fn main() {
    tauri_build::build()
}
```

**Step 4: Create minimal main.rs**

Create `cvm-agent/desktop/src-tauri/src/main.rs`:

```rust
// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 5: Create tauri.conf.json**

Create `cvm-agent/desktop/src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "CVM Agent",
  "version": "0.1.0",
  "identifier": "com.cvm.agent",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:5173",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "CVM Agent",
        "width": 400,
        "height": 600,
        "resizable": true,
        "decorations": true
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/icon.png"
    ]
  }
}
```

**Step 6: Create package.json for frontend**

Create `cvm-agent/desktop/package.json`:

```json
{
  "name": "cvm-desktop",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "tauri": "tauri"
  },
  "dependencies": {
    "react": "^18.2.0",
    "react-dom": "^18.2.0"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@types/react": "^18.2.0",
    "@types/react-dom": "^18.2.0",
    "@vitejs/plugin-react": "^4.2.0",
    "typescript": "^5.3.0",
    "vite": "^5.0.0"
  }
}
```

**Step 7: Update workspace Cargo.toml**

Edit `cvm-agent/Cargo.toml` to add desktop to members:

```toml
[workspace]
members = ["agent-api", "desktop/src-tauri"]
```

**Step 8: Create placeholder icon**

Run:
```bash
# Create a simple 32x32 PNG placeholder
convert -size 32x32 xc:blue cvm-agent/desktop/src-tauri/icons/icon.png 2>/dev/null || \
  echo "iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAFklEQVR42mNkGAWjYBSMglEwCkYBDQAABTgAAV9BLlYAAAAASUVORK5CYII=" | base64 -d > cvm-agent/desktop/src-tauri/icons/icon.png
```

**Step 9: Verify Cargo workspace**

Run: `cd cvm-agent && cargo check --workspace 2>&1 | tail -10`
Expected: Compilation proceeds (may have warnings, but no errors about workspace structure)

**Step 10: Commit**

```bash
git add cvm-agent/desktop/ cvm-agent/Cargo.toml
git commit -m "feat(desktop): initialize Tauri project structure

Create minimal Tauri 2.x project with:
- Cargo.toml with tauri, tonic, and gRPC dependencies
- tauri.conf.json for window configuration
- Minimal main.rs entry point
- package.json for React frontend build"
```

---

## Task 3: Create Minimal React Frontend

**Files:**
- Create: `cvm-agent/desktop/index.html`
- Create: `cvm-agent/desktop/vite.config.ts`
- Create: `cvm-agent/desktop/tsconfig.json`
- Create: `cvm-agent/desktop/src/main.tsx`
- Create: `cvm-agent/desktop/src/App.tsx`
- Create: `cvm-agent/desktop/src/App.css`

**Context:** We need a minimal React frontend that Tauri will embed. We'll start simple and migrate from web-ui later.

**Step 1: Create index.html**

Create `cvm-agent/desktop/index.html`:

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/png" href="/icon.png" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>CVM Agent</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

**Step 2: Create vite.config.ts**

Create `cvm-agent/desktop/vite.config.ts`:

```typescript
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: ["es2021", "chrome100", "safari13"],
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
```

**Step 3: Create tsconfig.json**

Create `cvm-agent/desktop/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

Create `cvm-agent/desktop/tsconfig.node.json`:

```json
{
  "compilerOptions": {
    "composite": true,
    "skipLibCheck": true,
    "module": "ESNext",
    "moduleResolution": "bundler",
    "allowSyntheticDefaultImports": true
  },
  "include": ["vite.config.ts"]
}
```

**Step 4: Create main.tsx**

Create `cvm-agent/desktop/src/main.tsx`:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./App.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

**Step 5: Create App.tsx**

Create `cvm-agent/desktop/src/App.tsx`:

```tsx
import { useState } from "react";

function App() {
  const [message, setMessage] = useState("");
  const [response, setResponse] = useState("");

  const sendMessage = async () => {
    // TODO: Call Tauri command to send gRPC message
    setResponse(`Echo: ${message}`);
  };

  return (
    <div className="container">
      <h1>CVM Agent</h1>
      <div className="chat-input">
        <input
          type="text"
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && sendMessage()}
          placeholder="Type a message..."
        />
        <button onClick={sendMessage}>Send</button>
      </div>
      {response && <div className="response">{response}</div>}
    </div>
  );
}

export default App;
```

**Step 6: Create App.css**

Create `cvm-agent/desktop/src/App.css`:

```css
:root {
  font-family: Inter, system-ui, sans-serif;
  background-color: #1a1a2e;
  color: #eee;
}

.container {
  padding: 20px;
  max-width: 400px;
  margin: 0 auto;
}

h1 {
  font-size: 1.5rem;
  margin-bottom: 20px;
  color: #7c3aed;
}

.chat-input {
  display: flex;
  gap: 8px;
}

.chat-input input {
  flex: 1;
  padding: 10px;
  border: 1px solid #333;
  border-radius: 6px;
  background: #2a2a3e;
  color: #eee;
}

.chat-input button {
  padding: 10px 20px;
  background: #7c3aed;
  color: white;
  border: none;
  border-radius: 6px;
  cursor: pointer;
}

.chat-input button:hover {
  background: #6d28d9;
}

.response {
  margin-top: 20px;
  padding: 15px;
  background: #2a2a3e;
  border-radius: 6px;
  white-space: pre-wrap;
}
```

**Step 7: Install npm dependencies and verify build**

Run:
```bash
cd cvm-agent/desktop && npm install
npm run build
```
Expected: Build completes, `dist/` directory created

**Step 8: Commit**

```bash
git add cvm-agent/desktop/
git commit -m "feat(desktop): add minimal React frontend

Create basic React app with:
- Vite build configuration
- Simple chat UI with input and response display
- Dark theme styling
- TypeScript configuration"
```

---

## Task 4: Add Global Hotkey Support

**Files:**
- Modify: `cvm-agent/desktop/src-tauri/src/main.rs`
- Modify: `cvm-agent/desktop/src-tauri/Cargo.toml`

**Context:** The core feature - pressing Ctrl+Shift+Space anywhere should toggle the CVM Agent overlay.

**Step 1: Update Cargo.toml for global-shortcut plugin**

The dependency is already added. Verify in Cargo.toml:
```toml
tauri-plugin-global-shortcut = "2"
```

**Step 2: Update main.rs with hotkey handler**

Replace `cvm-agent/desktop/src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Manager, AppHandle};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

fn toggle_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, _event| {
                    let toggle_shortcut = Shortcut::new(
                        Some(Modifiers::CONTROL | Modifiers::SHIFT),
                        Code::Space
                    );
                    if shortcut == &toggle_shortcut {
                        toggle_window(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            // Register the global shortcut
            let shortcut = Shortcut::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Code::Space
            );
            app.global_shortcut().register(shortcut)?;

            println!("CVM Agent started. Press Ctrl+Shift+Space to toggle.");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 3: Test compilation**

Run:
```bash
cd cvm-agent/desktop/src-tauri && cargo check
```
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add cvm-agent/desktop/src-tauri/
git commit -m "feat(desktop): add global hotkey support

Register Ctrl+Shift+Space to toggle window visibility.
Uses tauri-plugin-global-shortcut for system-wide hotkey."
```

---

## Task 5: Add gRPC Client for Agent API

**Files:**
- Create: `cvm-agent/desktop/src-tauri/src/grpc.rs`
- Create: `cvm-agent/desktop/src-tauri/build.rs` (update for proto)
- Modify: `cvm-agent/desktop/src-tauri/src/main.rs`
- Copy: Proto files to desktop project

**Context:** The Tauri app needs to call the Agent API directly via gRPC. We'll generate Rust code from the proto files and create a client.

**Step 1: Copy proto files**

Run:
```bash
mkdir -p cvm-agent/desktop/src-tauri/proto
cp cvm-agent/proto/*.proto cvm-agent/desktop/src-tauri/proto/
```

**Step 2: Update build.rs for proto compilation**

Replace `cvm-agent/desktop/src-tauri/build.rs`:

```rust
fn main() {
    // Compile protobuf files
    tonic_build::configure()
        .build_server(false)  // Client only
        .compile_protos(&["proto/agent.proto"], &["proto"])
        .expect("Failed to compile protos");

    tauri_build::build()
}
```

**Step 3: Add tonic-build dependency**

Edit `cvm-agent/desktop/src-tauri/Cargo.toml`, add to `[build-dependencies]`:

```toml
[build-dependencies]
tauri-build = { version = "2", features = [] }
tonic-build = "0.12"
```

**Step 4: Create grpc.rs client module**

Create `cvm-agent/desktop/src-tauri/src/grpc.rs`:

```rust
//! gRPC client for Agent API

pub mod agent {
    tonic::include_proto!("agent");
}

use agent::chat_service_client::ChatServiceClient;
use agent::ChatRequest;
use anyhow::Result;
use tonic::transport::Channel;

pub struct AgentClient {
    chat: ChatServiceClient<Channel>,
}

impl AgentClient {
    pub async fn connect(addr: &str) -> Result<Self> {
        let chat = ChatServiceClient::connect(addr.to_string()).await?;
        Ok(Self { chat })
    }

    pub async fn send_message(&mut self, message: &str) -> Result<String> {
        let request = tonic::Request::new(ChatRequest {
            message: message.to_string(),
            context: None,
        });

        let response = self.chat.send_message(request).await?;
        Ok(response.into_inner().response)
    }
}
```

**Step 5: Update main.rs with Tauri commands**

Replace `cvm-agent/desktop/src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod grpc;

use std::sync::Mutex;
use tauri::{Manager, AppHandle, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

struct AppState {
    grpc_addr: String,
}

fn toggle_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

#[tauri::command]
async fn send_message(
    message: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<String, String> {
    let addr = {
        let state = state.lock().map_err(|e| e.to_string())?;
        state.grpc_addr.clone()
    };

    let mut client = grpc::AgentClient::connect(&addr)
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    client
        .send_message(&message)
        .await
        .map_err(|e| format!("Send failed: {}", e))
}

#[tauri::command]
async fn health_check(state: State<'_, Mutex<AppState>>) -> Result<bool, String> {
    let addr = {
        let state = state.lock().map_err(|e| e.to_string())?;
        state.grpc_addr.clone()
    };

    match grpc::AgentClient::connect(&addr).await {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, _event| {
                    let toggle_shortcut = Shortcut::new(
                        Some(Modifiers::CONTROL | Modifiers::SHIFT),
                        Code::Space
                    );
                    if shortcut == &toggle_shortcut {
                        toggle_window(app);
                    }
                })
                .build(),
        )
        .manage(Mutex::new(AppState {
            grpc_addr: std::env::var("AGENT_GRPC_ADDR")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
        }))
        .setup(|app| {
            let shortcut = Shortcut::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Code::Space
            );
            app.global_shortcut().register(shortcut)?;

            println!("CVM Agent started. Press Ctrl+Shift+Space to toggle.");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![send_message, health_check])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 6: Test compilation**

Run:
```bash
cd cvm-agent/desktop/src-tauri && cargo check
```
Expected: Compiles (may have warnings about unused code)

**Step 7: Commit**

```bash
git add cvm-agent/desktop/
git commit -m "feat(desktop): add gRPC client for Agent API

- Add proto compilation in build.rs
- Create AgentClient with send_message method
- Add Tauri commands: send_message, health_check
- Configure gRPC address via AGENT_GRPC_ADDR env var"
```

---

## Task 6: Connect Frontend to Tauri Backend

**Files:**
- Modify: `cvm-agent/desktop/src/App.tsx`
- Create: `cvm-agent/desktop/src/hooks/useAgent.ts`

**Context:** Wire up the React frontend to call Tauri commands for gRPC communication.

**Step 1: Install Tauri API package**

Run:
```bash
cd cvm-agent/desktop && npm install @tauri-apps/api
```

**Step 2: Create useAgent hook**

Create `cvm-agent/desktop/src/hooks/useAgent.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";

export async function sendMessage(message: string): Promise<string> {
  return invoke<string>("send_message", { message });
}

export async function healthCheck(): Promise<boolean> {
  return invoke<boolean>("health_check");
}
```

**Step 3: Update App.tsx to use Tauri commands**

Replace `cvm-agent/desktop/src/App.tsx`:

```tsx
import { useState, useEffect } from "react";
import { sendMessage, healthCheck } from "./hooks/useAgent";

function App() {
  const [message, setMessage] = useState("");
  const [response, setResponse] = useState("");
  const [loading, setLoading] = useState(false);
  const [connected, setConnected] = useState(false);

  useEffect(() => {
    // Check connection on mount
    healthCheck()
      .then(setConnected)
      .catch(() => setConnected(false));
  }, []);

  const handleSend = async () => {
    if (!message.trim()) return;

    setLoading(true);
    try {
      const result = await sendMessage(message);
      setResponse(result);
      setMessage("");
    } catch (err) {
      setResponse(`Error: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="container">
      <header>
        <h1>CVM Agent</h1>
        <span className={`status ${connected ? "connected" : "disconnected"}`}>
          {connected ? "Connected" : "Disconnected"}
        </span>
      </header>

      <div className="chat-input">
        <input
          type="text"
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && !loading && handleSend()}
          placeholder="Type a message..."
          disabled={loading}
        />
        <button onClick={handleSend} disabled={loading || !message.trim()}>
          {loading ? "..." : "Send"}
        </button>
      </div>

      {response && (
        <div className="response">
          {response}
        </div>
      )}
    </div>
  );
}

export default App;
```

**Step 4: Update styles**

Add to `cvm-agent/desktop/src/App.css`:

```css
header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.status {
  font-size: 0.75rem;
  padding: 4px 8px;
  border-radius: 4px;
}

.status.connected {
  background: #10b981;
  color: white;
}

.status.disconnected {
  background: #ef4444;
  color: white;
}

.chat-input button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
```

**Step 5: Verify frontend builds**

Run:
```bash
cd cvm-agent/desktop && npm run build
```
Expected: Build succeeds

**Step 6: Commit**

```bash
git add cvm-agent/desktop/
git commit -m "feat(desktop): connect frontend to Tauri backend

- Add useAgent hook with Tauri invoke calls
- Show connection status indicator
- Handle loading and error states
- Wire up send button to gRPC call"
```

---

## Task 7: Create Desktop Service Script

**Files:**
- Create: `files/phala-cvm/services/desktop-app.sh`
- Modify: `files/phala-cvm/services/cvm-start.sh` (if needed)

**Context:** Replace web-ui.sh with a service script that launches the Tauri desktop app.

**Step 1: Create desktop-app.sh**

Create `files/phala-cvm/services/desktop-app.sh`:

```bash
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
```

**Step 2: Make executable**

Run:
```bash
chmod +x files/phala-cvm/services/desktop-app.sh
```

**Step 3: Commit**

```bash
git add files/phala-cvm/services/desktop-app.sh
git commit -m "feat(services): add desktop app service script

Manages Tauri desktop application lifecycle:
- Supports release and debug builds
- Sets DISPLAY and AGENT_GRPC_ADDR environment
- Provides start/stop/status/restart commands"
```

---

## Task 8: Build and Test Desktop App

**Files:**
- None (testing only)

**Context:** Build the complete Tauri application and verify it works with the Agent API.

**Step 1: Enter the container devshell**

Run:
```bash
docker exec -it phala-cvm bash
cd /app
nix develop .#cvm
```

**Step 2: Build the frontend**

Run:
```bash
cd /app/cvm-agent/desktop
npm install
npm run build
```
Expected: `dist/` directory created with bundled frontend

**Step 3: Build the Tauri app (debug)**

Run:
```bash
cd /app/cvm-agent/desktop/src-tauri
cargo build
```
Expected: Binary at `target/debug/cvm-desktop`

**Step 4: Start Agent API (if not running)**

Run:
```bash
/app/files/phala-cvm/services/agent-api.sh start
```

**Step 5: Test the desktop app**

Run:
```bash
export DISPLAY=:1
export AGENT_GRPC_ADDR=http://localhost:8080
/app/cvm-agent/desktop/src-tauri/target/debug/cvm-desktop &
```

**Step 6: Verify in VNC**

1. Connect to VNC at localhost:6080
2. Window should appear
3. Check connection status shows "Connected"
4. Type "hello" and click Send
5. Should see response from Agent API

**Step 7: Test hotkey**

1. Press Ctrl+Shift+Space - window should hide
2. Press Ctrl+Shift+Space again - window should show

**Step 8: Build release (optional)**

Run:
```bash
cd /app/cvm-agent/desktop/src-tauri
cargo build --release
```
Expected: Binary at `target/release/cvm-desktop` (~10MB)

---

## Verification Checklist

After completing all tasks:

- [ ] `nix flake check` passes
- [ ] `cargo check --workspace` in cvm-agent/ passes
- [ ] `npm run build` in desktop/ succeeds
- [ ] `cargo build` in desktop/src-tauri/ succeeds
- [ ] Desktop app starts and shows window
- [ ] Connection status shows "Connected" when Agent API running
- [ ] Sending message returns response
- [ ] Ctrl+Shift+Space toggles window visibility
- [ ] desktop-app.sh start/stop/status work

---

## Rollback Plan

If something breaks:

```bash
# Revert all desktop changes
git checkout HEAD~N -- cvm-agent/desktop/ flake.nix

# Or revert specific commits
git log --oneline  # Find commit hash
git revert <hash>
```

The existing web-ui remains untouched and can still be used.
