# CVM Agent Phase 3 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add GUI automation capabilities including screenshot capture, AT-SPI element discovery, xdotool actions, and vision-based fallback for element identification.

**Architecture:** GUIService wraps system tools (scrot for screenshots, atspi2 for accessibility tree, xdotool for input). Vision fallback sends screenshots to Redpill API for element identification when AT-SPI fails. Web UI gets controls for GUI inspection and actions.

**Tech Stack:** Rust (Tonic), AT-SPI2 (via atspi crate or CLI), xdotool, scrot/maim, base64 encoding, Redpill vision API

---

## Prerequisites

Before starting, ensure you're in the worktree:
```bash
cd /Users/hashwarlock/Projects/wrlx-nixos-config/.worktrees/cvm-agent
```

Phase 2 must be complete (NixOpsService, GitOpsService, risk classification).

---

## Task 1: Add GUIService Proto Definitions

**Files:**
- Modify: `cvm-agent/proto/agent.proto`

**Step 1: Add GUIService definitions to agent.proto**

Add after the existing GitOpsService definition:

```protobuf
// GUI automation service
service GUIService {
  // Capture screenshot of display or window
  rpc Screenshot(ScreenshotRequest) returns (ScreenshotResponse);

  // Query accessible UI elements via AT-SPI
  rpc GetElements(ElementQuery) returns (ElementsResponse);

  // Get element tree under a root element
  rpc GetElementTree(ElementTreeRequest) returns (ElementTreeResponse);

  // Perform click action
  rpc Click(ClickRequest) returns (ActionResponse);

  // Type text
  rpc Type(TypeRequest) returns (ActionResponse);

  // Press key combination
  rpc KeyPress(KeyPressRequest) returns (ActionResponse);

  // Move mouse to position
  rpc MoveMouse(MoveMouseRequest) returns (ActionResponse);

  // Describe screenshot using vision model
  rpc Describe(DescribeRequest) returns (VisionResponse);

  // Find element by description using vision
  rpc FindByVision(FindByVisionRequest) returns (FindByVisionResponse);
}

message ScreenshotRequest {
  string window_id = 1;           // Specific window, or empty for full screen
  bool include_cursor = 2;
  int32 quality = 3;              // JPEG quality 1-100, 0 for PNG
}

message ScreenshotResponse {
  bytes image_data = 1;           // PNG or JPEG bytes
  string format = 2;              // "png" or "jpeg"
  int32 width = 3;
  int32 height = 4;
}

message ElementQuery {
  string application = 1;         // App name filter (e.g., "firefox")
  string role = 2;                // Element role filter (e.g., "button", "text")
  string name = 3;                // Element name/label contains
  int32 max_depth = 4;            // Max tree depth to search (default: 10)
}

message ElementsResponse {
  repeated UIElement elements = 1;
}

message UIElement {
  string id = 1;                  // Unique element identifier
  string application = 2;
  string role = 3;                // button, text, menu, etc.
  string name = 4;                // Accessible name/label
  string description = 5;
  BoundingBox bounds = 6;
  repeated string states = 7;     // focused, checked, expanded, etc.
  repeated string actions = 8;    // Available actions
}

message BoundingBox {
  int32 x = 1;
  int32 y = 2;
  int32 width = 3;
  int32 height = 4;
}

message ElementTreeRequest {
  string element_id = 1;          // Root element ID, or empty for desktop
  int32 max_depth = 2;
}

message ElementTreeResponse {
  ElementNode root = 1;
}

message ElementNode {
  UIElement element = 1;
  repeated ElementNode children = 2;
}

message ClickRequest {
  oneof target {
    string element_id = 1;        // Click element by ID
    Coordinates position = 2;     // Click at coordinates
  }
  string button = 3;              // "left", "right", "middle" (default: left)
  int32 clicks = 4;               // Number of clicks (default: 1)
}

message Coordinates {
  int32 x = 1;
  int32 y = 2;
}

message TypeRequest {
  string text = 1;
  string element_id = 2;          // Optional: focus element first
  int32 delay_ms = 3;             // Delay between keystrokes (default: 0)
  bool clear_first = 4;           // Clear existing text first
}

message KeyPressRequest {
  repeated string keys = 1;       // e.g., ["ctrl", "c"] or ["Return"]
  string element_id = 2;          // Optional: focus element first
}

message MoveMouseRequest {
  int32 x = 1;
  int32 y = 2;
  bool smooth = 3;                // Animate movement
}

message ActionResponse {
  bool success = 1;
  string error = 2;
}

message DescribeRequest {
  bytes screenshot = 1;           // Optional: provide screenshot, or capture new
  string focus_area = 2;          // Natural language description of area to focus on
  string question = 3;            // Specific question about the UI
}

message VisionResponse {
  string description = 1;
  repeated IdentifiedElement elements = 2;
}

message IdentifiedElement {
  string description = 1;         // What the element appears to be
  BoundingBox estimated_bounds = 2;
  float confidence = 3;           // 0-1 confidence score
}

message FindByVisionRequest {
  string description = 1;         // Natural language: "the blue submit button"
  bytes screenshot = 2;           // Optional: provide screenshot
}

message FindByVisionResponse {
  bool found = 1;
  Coordinates center = 2;         // Center of found element
  BoundingBox bounds = 3;
  float confidence = 4;
  string element_description = 5;
}
```

**Step 2: Verify proto compiles**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds

**Step 3: Commit**

```bash
git add cvm-agent/proto/agent.proto
git commit -m "feat: add GUIService proto definitions

- Screenshot capture with window selection
- AT-SPI element query and tree traversal
- Click, type, keypress, mouse move actions
- Vision-based element description and finding"
```

---

## Task 2: Add GUI Dependencies

**Files:**
- Modify: `cvm-agent/Cargo.toml`
- Modify: `cvm-agent/agent-api/Cargo.toml`
- Modify: `files/phala-cvm/entrypoint.sh`

**Step 1: Add Rust dependencies**

Update `cvm-agent/Cargo.toml` workspace dependencies:
```toml
[workspace.dependencies]
# ... existing deps
base64 = "0.22"
image = "0.25"
```

Update `cvm-agent/agent-api/Cargo.toml`:
```toml
[dependencies]
# ... existing deps
base64.workspace = true
image.workspace = true
```

**Step 2: Add system packages to entrypoint.sh**

Add to the nix-env install section in `files/phala-cvm/entrypoint.sh`:
```bash
# GUI automation tools
nix-env -iA \
    nixpkgs.xdotool \
    nixpkgs.scrot \
    nixpkgs.at-spi2-core \
    nixpkgs.python3Packages.pyatspi
```

**Step 3: Verify build**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds

**Step 4: Commit**

```bash
git add cvm-agent/Cargo.toml cvm-agent/agent-api/Cargo.toml files/phala-cvm/entrypoint.sh
git commit -m "feat: add GUI automation dependencies

- base64 and image crates for screenshot handling
- xdotool, scrot, at-spi2-core packages in entrypoint"
```

---

## Task 3: Implement Screenshot Service

**Files:**
- Create: `cvm-agent/agent-api/src/services/gui/mod.rs`
- Create: `cvm-agent/agent-api/src/services/gui/screenshot.rs`

**Step 1: Create GUI module structure**

```bash
mkdir -p cvm-agent/agent-api/src/services/gui
```

Create `cvm-agent/agent-api/src/services/gui/mod.rs`:
```rust
pub mod screenshot;
pub mod atspi;
pub mod xdotool;
pub mod vision;

mod service;

pub use service::GUIServiceImpl;
```

**Step 2: Implement screenshot capture**

Create `cvm-agent/agent-api/src/services/gui/screenshot.rs`:
```rust
use anyhow::Result;
use std::process::Command;
use tokio::fs;

pub struct ScreenshotCapture;

impl ScreenshotCapture {
    /// Capture full screen or specific window
    pub async fn capture(
        window_id: Option<&str>,
        include_cursor: bool,
        quality: Option<u8>,
    ) -> Result<(Vec<u8>, String, u32, u32)> {
        let temp_path = format!("/tmp/screenshot_{}.png", std::process::id());

        let mut args = vec!["-o".to_string(), temp_path.clone()];

        if !include_cursor {
            args.push("--hidecursor".to_string());
        }

        if let Some(wid) = window_id {
            if !wid.is_empty() {
                args.push("-u".to_string()); // Use focused window
                args.push("-w".to_string());
                args.push(wid.to_string());
            }
        }

        // Run scrot
        let status = Command::new("scrot")
            .args(&args)
            .env("DISPLAY", ":1")
            .status()?;

        if !status.success() {
            anyhow::bail!("scrot failed with status: {}", status);
        }

        // Read the image
        let image_data = fs::read(&temp_path).await?;

        // Get dimensions
        let img = image::load_from_memory(&image_data)?;
        let width = img.width();
        let height = img.height();

        // Convert to JPEG if quality specified
        let (final_data, format) = if let Some(q) = quality {
            if q > 0 && q <= 100 {
                let mut jpeg_data = Vec::new();
                let mut cursor = std::io::Cursor::new(&mut jpeg_data);
                img.write_to(&mut cursor, image::ImageFormat::Jpeg)?;
                (jpeg_data, "jpeg".to_string())
            } else {
                (image_data, "png".to_string())
            }
        } else {
            (image_data, "png".to_string())
        };

        // Cleanup temp file
        let _ = fs::remove_file(&temp_path).await;

        Ok((final_data, format, width, height))
    }

    /// Capture and return base64 encoded (for vision API)
    pub async fn capture_base64() -> Result<(String, u32, u32)> {
        let (data, _, width, height) = Self::capture(None, false, Some(85)).await?;
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);
        Ok((encoded, width, height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires display
    async fn test_screenshot_capture() {
        let result = ScreenshotCapture::capture(None, false, None).await;
        // This test requires a running X server
        if std::env::var("DISPLAY").is_ok() {
            assert!(result.is_ok());
            let (data, format, width, height) = result.unwrap();
            assert!(!data.is_empty());
            assert_eq!(format, "png");
            assert!(width > 0);
            assert!(height > 0);
        }
    }
}
```

**Step 3: Verify build**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds (with warnings about unused modules)

**Step 4: Commit**

```bash
git add cvm-agent/agent-api/src/services/gui/
git commit -m "feat: implement screenshot capture

- scrot-based screenshot with window selection
- PNG and JPEG output formats
- Base64 encoding for vision API"
```

---

## Task 4: Implement AT-SPI Element Discovery

**Files:**
- Create: `cvm-agent/agent-api/src/services/gui/atspi.rs`

**Step 1: Implement AT-SPI wrapper**

Create `cvm-agent/agent-api/src/services/gui/atspi.rs`:
```rust
use anyhow::Result;
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Clone, Deserialize)]
pub struct ATSPIElement {
    pub id: String,
    pub application: String,
    pub role: String,
    pub name: String,
    pub description: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub states: Vec<String>,
    pub actions: Vec<String>,
}

pub struct ATSPIClient;

impl ATSPIClient {
    /// Query elements matching criteria using python-atspi
    pub fn query_elements(
        application: Option<&str>,
        role: Option<&str>,
        name: Option<&str>,
        max_depth: u32,
    ) -> Result<Vec<ATSPIElement>> {
        let script = Self::build_query_script(application, role, name, max_depth);

        let output = Command::new("python3")
            .arg("-c")
            .arg(&script)
            .env("DISPLAY", ":1")
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("AT-SPI query failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let elements: Vec<ATSPIElement> = serde_json::from_str(&stdout)?;
        Ok(elements)
    }

    /// Get element tree from a root
    pub fn get_element_tree(element_id: Option<&str>, max_depth: u32) -> Result<serde_json::Value> {
        let script = Self::build_tree_script(element_id, max_depth);

        let output = Command::new("python3")
            .arg("-c")
            .arg(&script)
            .env("DISPLAY", ":1")
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("AT-SPI tree query failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let tree: serde_json::Value = serde_json::from_str(&stdout)?;
        Ok(tree)
    }

    fn build_query_script(
        application: Option<&str>,
        role: Option<&str>,
        name: Option<&str>,
        max_depth: u32,
    ) -> String {
        let app_filter = application.map(|a| format!("'{}'", a)).unwrap_or("None".to_string());
        let role_filter = role.map(|r| format!("'{}'", r)).unwrap_or("None".to_string());
        let name_filter = name.map(|n| format!("'{}'", n)).unwrap_or("None".to_string());

        format!(r#"
import gi
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi
import json

def get_element_info(obj, app_name):
    try:
        comp = obj.get_component_iface()
        if comp:
            rect = comp.get_extents(Atspi.CoordType.SCREEN)
            x, y, w, h = rect.x, rect.y, rect.width, rect.height
        else:
            x, y, w, h = 0, 0, 0, 0

        states = []
        state_set = obj.get_state_set()
        for state in [Atspi.StateType.FOCUSED, Atspi.StateType.CHECKED,
                      Atspi.StateType.EXPANDED, Atspi.StateType.SELECTED,
                      Atspi.StateType.VISIBLE, Atspi.StateType.ENABLED]:
            if state_set.contains(state):
                states.append(state.value_nick)

        actions = []
        action_iface = obj.get_action_iface()
        if action_iface:
            for i in range(action_iface.get_n_actions()):
                actions.append(action_iface.get_action_name(i))

        return {{
            'id': str(hash(obj)),
            'application': app_name,
            'role': obj.get_role_name(),
            'name': obj.get_name() or '',
            'description': obj.get_description() or '',
            'x': x, 'y': y, 'width': w, 'height': h,
            'states': states,
            'actions': actions
        }}
    except Exception as e:
        return None

def search_elements(obj, app_name, app_filter, role_filter, name_filter, depth, max_depth, results):
    if depth > max_depth:
        return

    info = get_element_info(obj, app_name)
    if info:
        match = True
        if app_filter and app_filter.lower() not in app_name.lower():
            match = False
        if role_filter and role_filter.lower() != info['role'].lower():
            match = False
        if name_filter and name_filter.lower() not in info['name'].lower():
            match = False
        if match:
            results.append(info)

    for i in range(obj.get_child_count()):
        child = obj.get_child_at_index(i)
        if child:
            search_elements(child, app_name, app_filter, role_filter, name_filter, depth + 1, max_depth, results)

results = []
desktop = Atspi.get_desktop(0)
for i in range(desktop.get_child_count()):
    app = desktop.get_child_at_index(i)
    if app:
        app_name = app.get_name() or 'unknown'
        search_elements(app, app_name, {app_filter}, {role_filter}, {name_filter}, 0, {max_depth}, results)

print(json.dumps(results))
"#, app_filter=app_filter, role_filter=role_filter, name_filter=name_filter, max_depth=max_depth)
    }

    fn build_tree_script(element_id: Option<&str>, max_depth: u32) -> String {
        let root_filter = element_id.map(|id| format!("'{}'", id)).unwrap_or("None".to_string());

        format!(r#"
import gi
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi
import json

def build_tree(obj, app_name, depth, max_depth):
    if depth > max_depth:
        return None

    try:
        comp = obj.get_component_iface()
        if comp:
            rect = comp.get_extents(Atspi.CoordType.SCREEN)
            x, y, w, h = rect.x, rect.y, rect.width, rect.height
        else:
            x, y, w, h = 0, 0, 0, 0

        node = {{
            'element': {{
                'id': str(hash(obj)),
                'application': app_name,
                'role': obj.get_role_name(),
                'name': obj.get_name() or '',
                'description': obj.get_description() or '',
                'bounds': {{'x': x, 'y': y, 'width': w, 'height': h}},
                'states': [],
                'actions': []
            }},
            'children': []
        }}

        for i in range(obj.get_child_count()):
            child = obj.get_child_at_index(i)
            if child:
                child_tree = build_tree(child, app_name, depth + 1, max_depth)
                if child_tree:
                    node['children'].append(child_tree)

        return node
    except:
        return None

desktop = Atspi.get_desktop(0)
root_tree = {{'element': {{'id': 'desktop', 'application': 'desktop', 'role': 'desktop', 'name': 'Desktop'}}, 'children': []}}

for i in range(desktop.get_child_count()):
    app = desktop.get_child_at_index(i)
    if app:
        app_name = app.get_name() or 'unknown'
        app_tree = build_tree(app, app_name, 0, {max_depth})
        if app_tree:
            root_tree['children'].append(app_tree)

print(json.dumps(root_tree))
"#, max_depth=max_depth)
    }
}
```

**Step 2: Verify build**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds

**Step 3: Commit**

```bash
git add cvm-agent/agent-api/src/services/gui/atspi.rs
git commit -m "feat: implement AT-SPI element discovery

- Query elements by application, role, name
- Build element tree from root
- Extract bounds, states, and actions
- Python-atspi based implementation"
```

---

## Task 5: Implement xdotool Actions

**Files:**
- Create: `cvm-agent/agent-api/src/services/gui/xdotool.rs`

**Step 1: Implement xdotool wrapper**

Create `cvm-agent/agent-api/src/services/gui/xdotool.rs`:
```rust
use anyhow::Result;
use std::process::Command;

pub struct XDoTool;

impl XDoTool {
    /// Click at coordinates
    pub fn click_at(x: i32, y: i32, button: &str, clicks: u32) -> Result<()> {
        let button_num = match button {
            "left" | "" => "1",
            "middle" => "2",
            "right" => "3",
            _ => "1",
        };

        // Move mouse to position
        let status = Command::new("xdotool")
            .args(["mousemove", &x.to_string(), &y.to_string()])
            .env("DISPLAY", ":1")
            .status()?;

        if !status.success() {
            anyhow::bail!("xdotool mousemove failed");
        }

        // Perform clicks
        let click_count = if clicks == 0 { 1 } else { clicks };
        for _ in 0..click_count {
            let status = Command::new("xdotool")
                .args(["click", button_num])
                .env("DISPLAY", ":1")
                .status()?;

            if !status.success() {
                anyhow::bail!("xdotool click failed");
            }
        }

        Ok(())
    }

    /// Click on element center (by getting its bounds)
    pub fn click_element(element_id: &str, button: &str, clicks: u32) -> Result<()> {
        // Use AT-SPI to get element bounds
        let elements = super::atspi::ATSPIClient::query_elements(None, None, None, 20)?;

        let element = elements
            .iter()
            .find(|e| e.id == element_id)
            .ok_or_else(|| anyhow::anyhow!("Element not found: {}", element_id))?;

        let center_x = element.x + element.width / 2;
        let center_y = element.y + element.height / 2;

        Self::click_at(center_x, center_y, button, clicks)
    }

    /// Type text with optional delay between keystrokes
    pub fn type_text(text: &str, delay_ms: u32) -> Result<()> {
        let mut args = vec!["type"];

        if delay_ms > 0 {
            args.push("--delay");
            let delay_str = delay_ms.to_string();
            args.push(&delay_str);
        }

        args.push("--");
        args.push(text);

        let status = Command::new("xdotool")
            .args(&args)
            .env("DISPLAY", ":1")
            .status()?;

        if !status.success() {
            anyhow::bail!("xdotool type failed");
        }

        Ok(())
    }

    /// Press key combination
    pub fn key_press(keys: &[String]) -> Result<()> {
        if keys.is_empty() {
            return Ok(());
        }

        // xdotool uses "+" to combine keys, e.g., "ctrl+c"
        let key_combo = keys.join("+");

        let status = Command::new("xdotool")
            .args(["key", &key_combo])
            .env("DISPLAY", ":1")
            .status()?;

        if !status.success() {
            anyhow::bail!("xdotool key failed");
        }

        Ok(())
    }

    /// Move mouse to coordinates
    pub fn move_mouse(x: i32, y: i32, smooth: bool) -> Result<()> {
        let mut args = vec!["mousemove"];

        if !smooth {
            args.push("--sync");
        }

        let x_str = x.to_string();
        let y_str = y.to_string();
        args.push(&x_str);
        args.push(&y_str);

        let status = Command::new("xdotool")
            .args(&args)
            .env("DISPLAY", ":1")
            .status()?;

        if !status.success() {
            anyhow::bail!("xdotool mousemove failed");
        }

        Ok(())
    }

    /// Focus window by name or ID
    pub fn focus_window(window_name: &str) -> Result<()> {
        let status = Command::new("xdotool")
            .args(["search", "--name", window_name, "windowactivate"])
            .env("DISPLAY", ":1")
            .status()?;

        if !status.success() {
            anyhow::bail!("xdotool window focus failed");
        }

        Ok(())
    }

    /// Get current mouse position
    pub fn get_mouse_position() -> Result<(i32, i32)> {
        let output = Command::new("xdotool")
            .args(["getmouselocation", "--shell"])
            .env("DISPLAY", ":1")
            .output()?;

        if !output.status.success() {
            anyhow::bail!("xdotool getmouselocation failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut x = 0;
        let mut y = 0;

        for line in stdout.lines() {
            if line.starts_with("X=") {
                x = line[2..].parse().unwrap_or(0);
            } else if line.starts_with("Y=") {
                y = line[2..].parse().unwrap_or(0);
            }
        }

        Ok((x, y))
    }

    /// Clear text in current field (select all + delete)
    pub fn clear_field() -> Result<()> {
        Self::key_press(&["ctrl".to_string(), "a".to_string()])?;
        Self::key_press(&["Delete".to_string()])?;
        Ok(())
    }
}
```

**Step 2: Verify build**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds

**Step 3: Commit**

```bash
git add cvm-agent/agent-api/src/services/gui/xdotool.rs
git commit -m "feat: implement xdotool action wrapper

- Click at coordinates or element center
- Type text with delay support
- Key combinations (ctrl+c, etc.)
- Mouse movement and positioning
- Window focus and field clearing"
```

---

## Task 6: Implement Vision Fallback

**Files:**
- Create: `cvm-agent/agent-api/src/services/gui/vision.rs`

**Step 1: Implement vision-based element finding**

Create `cvm-agent/agent-api/src/services/gui/vision.rs`:
```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::llm::RedpillClient;

pub struct VisionService {
    llm: std::sync::Arc<RedpillClient>,
}

#[derive(Debug, Serialize)]
struct VisionRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<VisionMessage>,
}

#[derive(Debug, Serialize)]
struct VisionMessage {
    role: String,
    content: Vec<ContentBlock>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
}

#[derive(Debug, Serialize)]
struct ImageSource {
    #[serde(rename = "type")]
    source_type: String,
    media_type: String,
    data: String,
}

#[derive(Debug, Deserialize)]
struct VisionResponseContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IdentifiedElement {
    pub description: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub confidence: f32,
}

impl VisionService {
    pub fn new(llm: std::sync::Arc<RedpillClient>) -> Self {
        Self { llm }
    }

    /// Describe what's visible in a screenshot
    pub async fn describe_screenshot(
        &self,
        screenshot_base64: &str,
        focus_area: Option<&str>,
        question: Option<&str>,
    ) -> Result<String> {
        let prompt = if let Some(q) = question {
            q.to_string()
        } else if let Some(area) = focus_area {
            format!("Describe the UI elements visible in this screenshot, focusing on: {}. List the main interactive elements you can see with their approximate locations.", area)
        } else {
            "Describe the UI elements visible in this screenshot. List the main interactive elements (buttons, text fields, menus) with their approximate screen locations (top-left, center, bottom-right, etc.).".to_string()
        };

        let response = self.send_vision_request(screenshot_base64, &prompt).await?;
        Ok(response)
    }

    /// Find an element by natural language description
    pub async fn find_element(
        &self,
        screenshot_base64: &str,
        description: &str,
        screen_width: u32,
        screen_height: u32,
    ) -> Result<Option<IdentifiedElement>> {
        let prompt = format!(
            r#"I need to find this UI element: "{}"

Look at the screenshot and find this element. Respond with ONLY a JSON object in this exact format:
{{
  "found": true/false,
  "description": "what you identified",
  "x_percent": 0-100,
  "y_percent": 0-100,
  "width_percent": 0-100,
  "height_percent": 0-100,
  "confidence": 0.0-1.0
}}

x_percent and y_percent are the center coordinates as a percentage of screen size.
width_percent and height_percent are the estimated element size as percentages.

If you cannot find the element, set found to false."#,
            description
        );

        let response = self.send_vision_request(screenshot_base64, &prompt).await?;

        // Parse JSON response
        let json_start = response.find('{').unwrap_or(0);
        let json_end = response.rfind('}').map(|i| i + 1).unwrap_or(response.len());
        let json_str = &response[json_start..json_end];

        #[derive(Deserialize)]
        struct FindResult {
            found: bool,
            description: Option<String>,
            x_percent: Option<f32>,
            y_percent: Option<f32>,
            width_percent: Option<f32>,
            height_percent: Option<f32>,
            confidence: Option<f32>,
        }

        let result: FindResult = serde_json::from_str(json_str)?;

        if !result.found {
            return Ok(None);
        }

        let x_pct = result.x_percent.unwrap_or(50.0);
        let y_pct = result.y_percent.unwrap_or(50.0);
        let w_pct = result.width_percent.unwrap_or(10.0);
        let h_pct = result.height_percent.unwrap_or(5.0);

        let x = ((x_pct / 100.0) * screen_width as f32) as i32;
        let y = ((y_pct / 100.0) * screen_height as f32) as i32;
        let width = ((w_pct / 100.0) * screen_width as f32) as i32;
        let height = ((h_pct / 100.0) * screen_height as f32) as i32;

        Ok(Some(IdentifiedElement {
            description: result.description.unwrap_or_default(),
            x: x - width / 2,  // Convert center to top-left
            y: y - height / 2,
            width,
            height,
            confidence: result.confidence.unwrap_or(0.5),
        }))
    }

    async fn send_vision_request(&self, image_base64: &str, prompt: &str) -> Result<String> {
        let client = reqwest::Client::new();

        let api_key = std::env::var("REDPILL_API_KEY")
            .or_else(|_| std::env::var("ANTHROPIC_AUTH_TOKEN"))?;
        let base_url = std::env::var("REDPILL_BASE_URL")
            .unwrap_or_else(|_| "https://api.redpill.ai".to_string());
        let model = std::env::var("REDPILL_MODEL")
            .unwrap_or_else(|_| "claude-3-5-sonnet-20241022".to_string());

        let request = VisionRequest {
            model,
            max_tokens: 1024,
            messages: vec![VisionMessage {
                role: "user".to_string(),
                content: vec![
                    ContentBlock::Image {
                        source: ImageSource {
                            source_type: "base64".to_string(),
                            media_type: "image/jpeg".to_string(),
                            data: image_base64.to_string(),
                        },
                    },
                    ContentBlock::Text {
                        text: prompt.to_string(),
                    },
                ],
            }],
        };

        let response = client
            .post(format!("{}/v1/messages", base_url))
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Vision API error: {}", error_text);
        }

        #[derive(Deserialize)]
        struct APIResponse {
            content: Vec<VisionResponseContent>,
        }

        let api_response: APIResponse = response.json().await?;

        let text = api_response
            .content
            .iter()
            .filter_map(|c| c.text.clone())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(text)
    }
}
```

**Step 2: Verify build**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds

**Step 3: Commit**

```bash
git add cvm-agent/agent-api/src/services/gui/vision.rs
git commit -m "feat: implement vision-based element finding

- Screenshot description via Redpill vision API
- Natural language element finding
- Percentage-based coordinate conversion
- Confidence scoring"
```

---

## Task 7: Implement GUIService

**Files:**
- Create: `cvm-agent/agent-api/src/services/gui/service.rs`
- Modify: `cvm-agent/agent-api/src/services/mod.rs`
- Modify: `cvm-agent/agent-api/src/main.rs`

**Step 1: Implement GUIService**

Create `cvm-agent/agent-api/src/services/gui/service.rs`:
```rust
use std::sync::Arc;
use tonic::{Request, Response, Status};

use super::atspi::ATSPIClient;
use super::screenshot::ScreenshotCapture;
use super::vision::VisionService;
use super::xdotool::XDoTool;

use crate::llm::RedpillClient;
use crate::services::health::proto::gui_service_server::GuiService;
use crate::services::health::proto::{
    ActionResponse, BoundingBox, ClickRequest, Coordinates, DescribeRequest,
    ElementNode, ElementQuery, ElementTreeRequest, ElementTreeResponse,
    ElementsResponse, FindByVisionRequest, FindByVisionResponse, IdentifiedElement,
    KeyPressRequest, MoveMouseRequest, ScreenshotRequest, ScreenshotResponse,
    TypeRequest, UiElement, VisionResponse,
};

pub struct GUIServiceImpl {
    vision: VisionService,
}

impl GUIServiceImpl {
    pub fn new(llm: Arc<RedpillClient>) -> Self {
        Self {
            vision: VisionService::new(llm),
        }
    }
}

#[tonic::async_trait]
impl GuiService for GUIServiceImpl {
    async fn screenshot(
        &self,
        request: Request<ScreenshotRequest>,
    ) -> Result<Response<ScreenshotResponse>, Status> {
        let req = request.into_inner();

        let window_id = if req.window_id.is_empty() {
            None
        } else {
            Some(req.window_id.as_str())
        };

        let quality = if req.quality > 0 {
            Some(req.quality as u8)
        } else {
            None
        };

        let (data, format, width, height) = ScreenshotCapture::capture(
            window_id,
            req.include_cursor,
            quality,
        )
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ScreenshotResponse {
            image_data: data,
            format,
            width,
            height,
        }))
    }

    async fn get_elements(
        &self,
        request: Request<ElementQuery>,
    ) -> Result<Response<ElementsResponse>, Status> {
        let req = request.into_inner();

        let application = if req.application.is_empty() { None } else { Some(req.application.as_str()) };
        let role = if req.role.is_empty() { None } else { Some(req.role.as_str()) };
        let name = if req.name.is_empty() { None } else { Some(req.name.as_str()) };
        let max_depth = if req.max_depth == 0 { 10 } else { req.max_depth as u32 };

        let elements = ATSPIClient::query_elements(application, role, name, max_depth)
            .map_err(|e| Status::internal(e.to_string()))?;

        let ui_elements: Vec<UiElement> = elements
            .into_iter()
            .map(|e| UiElement {
                id: e.id,
                application: e.application,
                role: e.role,
                name: e.name,
                description: e.description,
                bounds: Some(BoundingBox {
                    x: e.x,
                    y: e.y,
                    width: e.width,
                    height: e.height,
                }),
                states: e.states,
                actions: e.actions,
            })
            .collect();

        Ok(Response::new(ElementsResponse { elements: ui_elements }))
    }

    async fn get_element_tree(
        &self,
        request: Request<ElementTreeRequest>,
    ) -> Result<Response<ElementTreeResponse>, Status> {
        let req = request.into_inner();

        let element_id = if req.element_id.is_empty() { None } else { Some(req.element_id.as_str()) };
        let max_depth = if req.max_depth == 0 { 5 } else { req.max_depth as u32 };

        let tree = ATSPIClient::get_element_tree(element_id, max_depth)
            .map_err(|e| Status::internal(e.to_string()))?;

        // Convert JSON tree to proto ElementNode
        let root = json_to_element_node(&tree);

        Ok(Response::new(ElementTreeResponse { root }))
    }

    async fn click(
        &self,
        request: Request<ClickRequest>,
    ) -> Result<Response<ActionResponse>, Status> {
        let req = request.into_inner();
        let button = if req.button.is_empty() { "left" } else { &req.button };
        let clicks = if req.clicks == 0 { 1 } else { req.clicks as u32 };

        let result = match req.target {
            Some(click_request::Target::ElementId(id)) => {
                XDoTool::click_element(&id, button, clicks)
            }
            Some(click_request::Target::Position(coords)) => {
                XDoTool::click_at(coords.x, coords.y, button, clicks)
            }
            None => {
                return Ok(Response::new(ActionResponse {
                    success: false,
                    error: "No target specified".to_string(),
                }));
            }
        };

        match result {
            Ok(()) => Ok(Response::new(ActionResponse {
                success: true,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(ActionResponse {
                success: false,
                error: e.to_string(),
            })),
        }
    }

    async fn r#type(
        &self,
        request: Request<TypeRequest>,
    ) -> Result<Response<ActionResponse>, Status> {
        let req = request.into_inner();

        // Focus element if specified
        if !req.element_id.is_empty() {
            if let Err(e) = XDoTool::click_element(&req.element_id, "left", 1) {
                return Ok(Response::new(ActionResponse {
                    success: false,
                    error: format!("Failed to focus element: {}", e),
                }));
            }
        }

        // Clear field if requested
        if req.clear_first {
            if let Err(e) = XDoTool::clear_field() {
                return Ok(Response::new(ActionResponse {
                    success: false,
                    error: format!("Failed to clear field: {}", e),
                }));
            }
        }

        // Type text
        match XDoTool::type_text(&req.text, req.delay_ms as u32) {
            Ok(()) => Ok(Response::new(ActionResponse {
                success: true,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(ActionResponse {
                success: false,
                error: e.to_string(),
            })),
        }
    }

    async fn key_press(
        &self,
        request: Request<KeyPressRequest>,
    ) -> Result<Response<ActionResponse>, Status> {
        let req = request.into_inner();

        // Focus element if specified
        if !req.element_id.is_empty() {
            if let Err(e) = XDoTool::click_element(&req.element_id, "left", 1) {
                return Ok(Response::new(ActionResponse {
                    success: false,
                    error: format!("Failed to focus element: {}", e),
                }));
            }
        }

        match XDoTool::key_press(&req.keys) {
            Ok(()) => Ok(Response::new(ActionResponse {
                success: true,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(ActionResponse {
                success: false,
                error: e.to_string(),
            })),
        }
    }

    async fn move_mouse(
        &self,
        request: Request<MoveMouseRequest>,
    ) -> Result<Response<ActionResponse>, Status> {
        let req = request.into_inner();

        match XDoTool::move_mouse(req.x, req.y, req.smooth) {
            Ok(()) => Ok(Response::new(ActionResponse {
                success: true,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(ActionResponse {
                success: false,
                error: e.to_string(),
            })),
        }
    }

    async fn describe(
        &self,
        request: Request<DescribeRequest>,
    ) -> Result<Response<VisionResponse>, Status> {
        let req = request.into_inner();

        // Get screenshot if not provided
        let (screenshot_b64, _, _) = if req.screenshot.is_empty() {
            ScreenshotCapture::capture_base64()
                .await
                .map_err(|e| Status::internal(e.to_string()))?
        } else {
            let b64 = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &req.screenshot,
            );
            (b64, 0, 0)
        };

        let focus = if req.focus_area.is_empty() { None } else { Some(req.focus_area.as_str()) };
        let question = if req.question.is_empty() { None } else { Some(req.question.as_str()) };

        let description = self
            .vision
            .describe_screenshot(&screenshot_b64, focus, question)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(VisionResponse {
            description,
            elements: vec![], // Could parse description for elements
        }))
    }

    async fn find_by_vision(
        &self,
        request: Request<FindByVisionRequest>,
    ) -> Result<Response<FindByVisionResponse>, Status> {
        let req = request.into_inner();

        // Get screenshot
        let (screenshot_b64, width, height) = if req.screenshot.is_empty() {
            ScreenshotCapture::capture_base64()
                .await
                .map_err(|e| Status::internal(e.to_string()))?
        } else {
            let b64 = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &req.screenshot,
            );
            // Would need to decode image to get dimensions
            (b64, 1920, 1080) // Default assumption
        };

        let element = self
            .vision
            .find_element(&screenshot_b64, &req.description, width, height)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        match element {
            Some(e) => Ok(Response::new(FindByVisionResponse {
                found: true,
                center: Some(Coordinates {
                    x: e.x + e.width / 2,
                    y: e.y + e.height / 2,
                }),
                bounds: Some(BoundingBox {
                    x: e.x,
                    y: e.y,
                    width: e.width,
                    height: e.height,
                }),
                confidence: e.confidence,
                element_description: e.description,
            })),
            None => Ok(Response::new(FindByVisionResponse {
                found: false,
                center: None,
                bounds: None,
                confidence: 0.0,
                element_description: String::new(),
            })),
        }
    }
}

fn json_to_element_node(json: &serde_json::Value) -> Option<ElementNode> {
    let element_json = json.get("element")?;

    let bounds = element_json.get("bounds").map(|b| BoundingBox {
        x: b["x"].as_i64().unwrap_or(0) as i32,
        y: b["y"].as_i64().unwrap_or(0) as i32,
        width: b["width"].as_i64().unwrap_or(0) as i32,
        height: b["height"].as_i64().unwrap_or(0) as i32,
    });

    let element = UiElement {
        id: element_json["id"].as_str().unwrap_or("").to_string(),
        application: element_json["application"].as_str().unwrap_or("").to_string(),
        role: element_json["role"].as_str().unwrap_or("").to_string(),
        name: element_json["name"].as_str().unwrap_or("").to_string(),
        description: element_json["description"].as_str().unwrap_or("").to_string(),
        bounds,
        states: vec![],
        actions: vec![],
    };

    let children: Vec<ElementNode> = json
        .get("children")
        .and_then(|c| c.as_array())
        .map(|arr| arr.iter().filter_map(json_to_element_node).collect())
        .unwrap_or_default();

    Some(ElementNode {
        element: Some(element),
        children,
    })
}

// Need to import the click_request module
use crate::services::health::proto::click_request;
```

**Step 2: Update services/mod.rs**

Update `cvm-agent/agent-api/src/services/mod.rs`:
```rust
pub mod chat;
pub mod gitops;
pub mod gui;
pub mod health;
pub mod nixops;
pub mod shell;

pub use chat::ChatServiceImpl;
pub use gitops::GitOpsServiceImpl;
pub use gui::GUIServiceImpl;
pub use health::HealthServiceImpl;
pub use nixops::NixOpsServiceImpl;
pub use shell::ShellServiceImpl;
```

**Step 3: Register GUIService in main.rs**

Update `cvm-agent/agent-api/src/main.rs` imports:
```rust
use services::health::proto::gui_service_server::GuiServiceServer;
use services::{ChatServiceImpl, GitOpsServiceImpl, GUIServiceImpl, HealthServiceImpl, NixOpsServiceImpl, ShellServiceImpl};
```

Add service creation and registration:
```rust
let gui_service = GUIServiceImpl::new(llm_client.clone());

Server::builder()
    // ... existing config
    .add_service(GuiServiceServer::new(gui_service))
```

**Step 4: Verify build**

Run:
```bash
cd cvm-agent && cargo build
```
Expected: Build succeeds

**Step 5: Commit**

```bash
git add cvm-agent/agent-api/src/
git commit -m "feat: implement GUIService

- Screenshot capture via scrot
- AT-SPI element query and tree
- xdotool click, type, keypress, mouse
- Vision-based element finding via Redpill
- Full gRPC service implementation"
```

---

## Task 8: Regenerate TypeScript Client

**Files:**
- Modify: `cvm-agent/web-ui/src/gen/` (regenerated)

**Step 1: Regenerate proto client**

Run:
```bash
cd cvm-agent/web-ui && docker run --rm -v "$(pwd):/app" -v "$(pwd)/../proto:/proto" -w /app node:20-alpine sh -c "npm install && npx buf generate /proto"
```

**Step 2: Verify generation**

Run:
```bash
ls cvm-agent/web-ui/src/gen/
```
Expected: Updated `agent_pb.ts` with GUIService types

**Step 3: Commit**

```bash
git add cvm-agent/web-ui/src/gen/
git commit -m "feat: regenerate TypeScript client for GUIService

- Screenshot, element query, action methods
- Vision description and find methods"
```

---

## Task 9: Add GUI Control UI Components

**Files:**
- Create: `cvm-agent/web-ui/src/hooks/useGUI.ts`
- Create: `cvm-agent/web-ui/src/components/GUIInspector.tsx`

**Step 1: Create useGUI hook**

Create `cvm-agent/web-ui/src/hooks/useGUI.ts`:
```typescript
import { createConnectTransport } from "@connectrpc/connect-web";
import { createClient } from "@connectrpc/connect";
import { GUIService } from "../gen/agent_pb";
import { useState, useCallback } from "react";

const transport = createConnectTransport({
  baseUrl: "/api",
});

const guiClient = createClient(GUIService, transport);

export interface UIElement {
  id: string;
  application: string;
  role: string;
  name: string;
  bounds: { x: number; y: number; width: number; height: number } | null;
}

export function useGUI() {
  const [screenshot, setScreenshot] = useState<string | null>(null);
  const [elements, setElements] = useState<UIElement[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const captureScreenshot = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await guiClient.screenshot({ quality: 85 });
      const base64 = btoa(
        String.fromCharCode(...new Uint8Array(response.imageData))
      );
      setScreenshot(`data:image/${response.format};base64,${base64}`);
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  }, []);

  const queryElements = useCallback(async (
    application?: string,
    role?: string,
    name?: string
  ) => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await guiClient.getElements({
        application: application || "",
        role: role || "",
        name: name || "",
        maxDepth: 10,
      });
      setElements(
        response.elements.map((e) => ({
          id: e.id,
          application: e.application,
          role: e.role,
          name: e.name,
          bounds: e.bounds
            ? {
                x: e.bounds.x,
                y: e.bounds.y,
                width: e.bounds.width,
                height: e.bounds.height,
              }
            : null,
        }))
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  }, []);

  const click = useCallback(async (x: number, y: number) => {
    try {
      await guiClient.click({
        target: { case: "position", value: { x, y } },
        button: "left",
        clicks: 1,
      });
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const typeText = useCallback(async (text: string) => {
    try {
      await guiClient.type({ text, delayMs: 0, clearFirst: false });
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const findByVision = useCallback(async (description: string) => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await guiClient.findByVision({ description });
      if (response.found && response.center) {
        return { x: response.center.x, y: response.center.y };
      }
      return null;
    } catch (e) {
      setError(String(e));
      return null;
    } finally {
      setIsLoading(false);
    }
  }, []);

  return {
    screenshot,
    elements,
    isLoading,
    error,
    captureScreenshot,
    queryElements,
    click,
    typeText,
    findByVision,
  };
}
```

**Step 2: Create GUIInspector component**

Create `cvm-agent/web-ui/src/components/GUIInspector.tsx`:
```tsx
import { useEffect, useState } from "react";
import { useGUI } from "../hooks/useGUI";

interface GUIInspectorProps {
  onClose: () => void;
}

export function GUIInspector({ onClose }: GUIInspectorProps) {
  const {
    screenshot,
    elements,
    isLoading,
    error,
    captureScreenshot,
    queryElements,
    click,
    findByVision,
  } = useGUI();

  const [filter, setFilter] = useState({ app: "", role: "", name: "" });
  const [visionQuery, setVisionQuery] = useState("");
  const [selectedElement, setSelectedElement] = useState<string | null>(null);

  useEffect(() => {
    captureScreenshot();
    queryElements();
  }, [captureScreenshot, queryElements]);

  const handleRefresh = () => {
    captureScreenshot();
    queryElements(filter.app, filter.role, filter.name);
  };

  const handleFindByVision = async () => {
    if (visionQuery.trim()) {
      const result = await findByVision(visionQuery);
      if (result) {
        // Highlight found position
        console.log("Found at:", result);
      }
    }
  };

  const handleClickElement = async (e: { bounds: { x: number; y: number; width: number; height: number } | null }) => {
    if (e.bounds) {
      const centerX = e.bounds.x + e.bounds.width / 2;
      const centerY = e.bounds.y + e.bounds.height / 2;
      await click(centerX, centerY);
      handleRefresh();
    }
  };

  return (
    <div className="fixed inset-0 bg-black/90 flex z-50">
      {/* Screenshot Panel */}
      <div className="flex-1 p-4 overflow-hidden">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold text-white">GUI Inspector</h2>
          <div className="flex gap-2">
            <button
              onClick={handleRefresh}
              disabled={isLoading}
              className="px-3 py-1 bg-blue-600 hover:bg-blue-700 rounded text-sm"
            >
              Refresh
            </button>
            <button
              onClick={onClose}
              className="px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded text-sm"
            >
              Close
            </button>
          </div>
        </div>

        {error && (
          <div className="mb-4 p-2 bg-red-500/20 border border-red-500/50 rounded text-red-400 text-sm">
            {error}
          </div>
        )}

        {/* Screenshot with element overlays */}
        <div className="relative bg-gray-900 rounded overflow-hidden" style={{ maxHeight: "70vh" }}>
          {screenshot && (
            <img
              src={screenshot}
              alt="Screenshot"
              className="w-full h-auto"
              onClick={(e) => {
                const rect = e.currentTarget.getBoundingClientRect();
                const scaleX = 1920 / rect.width; // Assuming 1920 width
                const scaleY = 1080 / rect.height;
                const x = Math.round((e.clientX - rect.left) * scaleX);
                const y = Math.round((e.clientY - rect.top) * scaleY);
                click(x, y);
              }}
            />
          )}
        </div>

        {/* Vision Search */}
        <div className="mt-4 flex gap-2">
          <input
            type="text"
            value={visionQuery}
            onChange={(e) => setVisionQuery(e.target.value)}
            placeholder="Find element by description (e.g., 'blue submit button')"
            className="flex-1 bg-gray-800 border border-gray-600 rounded px-3 py-2 text-white text-sm"
          />
          <button
            onClick={handleFindByVision}
            disabled={isLoading}
            className="px-4 py-2 bg-purple-600 hover:bg-purple-700 rounded text-sm"
          >
            Find
          </button>
        </div>
      </div>

      {/* Elements Panel */}
      <div className="w-80 bg-gray-900 border-l border-gray-700 p-4 overflow-y-auto">
        <h3 className="text-sm font-medium text-gray-400 mb-3">UI Elements ({elements.length})</h3>

        {/* Filters */}
        <div className="space-y-2 mb-4">
          <input
            type="text"
            placeholder="Filter by app..."
            value={filter.app}
            onChange={(e) => setFilter({ ...filter, app: e.target.value })}
            className="w-full bg-gray-800 border border-gray-600 rounded px-2 py-1 text-sm text-white"
          />
          <input
            type="text"
            placeholder="Filter by role..."
            value={filter.role}
            onChange={(e) => setFilter({ ...filter, role: e.target.value })}
            className="w-full bg-gray-800 border border-gray-600 rounded px-2 py-1 text-sm text-white"
          />
          <button
            onClick={() => queryElements(filter.app, filter.role, filter.name)}
            className="w-full py-1 bg-gray-700 hover:bg-gray-600 rounded text-sm"
          >
            Apply Filters
          </button>
        </div>

        {/* Element List */}
        <div className="space-y-1">
          {elements.map((el) => (
            <div
              key={el.id}
              className={`p-2 rounded cursor-pointer text-sm ${
                selectedElement === el.id
                  ? "bg-blue-500/30 border border-blue-500/50"
                  : "bg-gray-800 hover:bg-gray-750"
              }`}
              onClick={() => setSelectedElement(el.id)}
              onDoubleClick={() => handleClickElement(el)}
            >
              <div className="flex items-center gap-2">
                <span className="text-blue-400 font-mono text-xs">{el.role}</span>
                <span className="text-gray-200 truncate">{el.name || "(no name)"}</span>
              </div>
              <div className="text-gray-500 text-xs truncate">{el.application}</div>
              {el.bounds && (
                <div className="text-gray-600 text-xs">
                  {el.bounds.x},{el.bounds.y} {el.bounds.width}x{el.bounds.height}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
```

**Step 3: Add GUIInspector to App**

Update `cvm-agent/web-ui/src/App.tsx` to include GUIInspector:

Add import:
```typescript
import { GUIInspector } from "./components/GUIInspector";
```

Add state:
```typescript
const [showGUI, setShowGUI] = useState(false);
```

Add to ChatOverlay props:
```typescript
onShowGUI={() => setShowGUI(true)}
```

Add modal:
```tsx
{showGUI && (
  <GUIInspector onClose={() => setShowGUI(false)} />
)}
```

Update ChatOverlay to accept `onShowGUI` prop and add a "GUI" button.

**Step 4: Verify build**

Run:
```bash
cd cvm-agent/web-ui && docker run --rm -v "$(pwd):/app" -w /app node:20-alpine npm run build
```
Expected: Build succeeds

**Step 5: Commit**

```bash
git add cvm-agent/web-ui/src/
git commit -m "feat: add GUI Inspector UI component

- useGUI hook for GUI operations
- GUIInspector with screenshot view
- Element list with filters
- Vision-based element finding
- Click-to-interact on screenshot"
```

---

## Task 10: Integration Test

**Step 1: Build everything**

Run:
```bash
cd cvm-agent && cargo build --release
cd cvm-agent/web-ui && docker run --rm -v "$(pwd):/app" -w /app node:20-alpine npm run build
```
Expected: Both builds succeed

**Step 2: Run Rust tests**

Run:
```bash
cd cvm-agent && cargo test
```
Expected: All tests pass

**Step 3: Run TypeScript type check**

Run:
```bash
cd cvm-agent/web-ui && docker run --rm -v "$(pwd):/app" -w /app node:20-alpine npx tsc --noEmit
```
Expected: No type errors

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: complete Phase 3 - GUI Automation

Phase 3 complete:
- GUIService with screenshot, AT-SPI, xdotool, vision
- Screenshot capture via scrot
- AT-SPI element discovery and tree traversal
- xdotool click, type, keypress actions
- Vision-based element finding via Redpill
- GUIInspector UI component

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Summary

After completing all tasks, you will have:

1. **GUIService** (`cvm-agent/agent-api/src/services/gui/`)
   - Screenshot capture (scrot-based)
   - AT-SPI element discovery
   - xdotool action execution
   - Vision-based fallback

2. **GUI Modules**
   - `screenshot.rs` - Capture and encoding
   - `atspi.rs` - Accessibility tree queries
   - `xdotool.rs` - Input automation
   - `vision.rs` - LLM-based element finding

3. **Web UI Components**
   - `useGUI` hook - Client operations
   - `GUIInspector` - Visual inspection tool

## Next Phase

Phase 4 will add:
- Whisper server integration
- Voice input UI with live transcription
- Instruction skills loader
- Workflow skills executor
- Skill creation via chat
