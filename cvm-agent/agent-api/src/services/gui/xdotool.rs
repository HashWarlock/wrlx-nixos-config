//! xdotool integration for input automation.
//!
//! This module provides keyboard and mouse input automation using the xdotool
//! command-line tool. It supports clicking, typing, key presses, mouse movement,
//! and window management.

use anyhow::{Context, Result};
use std::process::Command;
use tracing::{debug, instrument};

use super::atspi::ATSPIClient;

/// Mouse button types for click operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    /// Returns the xdotool button number (1=left, 2=middle, 3=right).
    fn as_xdotool_button(&self) -> &str {
        match self {
            MouseButton::Left => "1",
            MouseButton::Right => "3",
            MouseButton::Middle => "2",
        }
    }

    /// Parses a button string to MouseButton.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "left" | "1" => Some(MouseButton::Left),
            "right" | "3" => Some(MouseButton::Right),
            "middle" | "2" => Some(MouseButton::Middle),
            _ => None,
        }
    }
}

impl Default for MouseButton {
    fn default() -> Self {
        MouseButton::Left
    }
}

/// Mouse position on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MousePosition {
    pub x: i32,
    pub y: i32,
}

/// xdotool wrapper for input automation.
///
/// Provides methods for mouse clicks, keyboard input, and window management
/// using the xdotool command-line utility.
#[derive(Debug, Clone)]
pub struct XDoTool {
    /// X display to target (e.g., ":1").
    display: String,
}

impl Default for XDoTool {
    fn default() -> Self {
        Self {
            display: ":1".to_string(),
        }
    }
}

impl XDoTool {
    /// Creates a new XDoTool instance with default display (:1).
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new XDoTool instance for a specific display.
    pub fn with_display(display: impl Into<String>) -> Self {
        Self {
            display: display.into(),
        }
    }

    /// Executes an xdotool command with the configured display.
    fn run_xdotool(&self, args: &[&str]) -> Result<String> {
        debug!(display = %self.display, args = ?args, "Running xdotool command");

        let output = Command::new("xdotool")
            .args(args)
            .env("DISPLAY", &self.display)
            .output()
            .context("Failed to execute xdotool command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("xdotool command failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    }

    /// Clicks at specific screen coordinates.
    ///
    /// # Arguments
    ///
    /// * `x` - X coordinate on screen
    /// * `y` - Y coordinate on screen
    /// * `button` - Mouse button to click (left, right, middle)
    /// * `clicks` - Number of clicks (1 for single, 2 for double, etc.)
    #[instrument(skip(self))]
    pub fn click_at(&self, x: i32, y: i32, button: MouseButton, clicks: u32) -> Result<()> {
        debug!(x, y, ?button, clicks, "Clicking at coordinates");

        // Move mouse to position
        self.run_xdotool(&["mousemove", "--sync", &x.to_string(), &y.to_string()])?;

        // Perform clicks
        let button_str = button.as_xdotool_button();
        let repeat_str = clicks.to_string();

        self.run_xdotool(&["click", "--repeat", &repeat_str, button_str])?;

        Ok(())
    }

    /// Clicks an element by its AT-SPI accessibility ID.
    ///
    /// Looks up the element's bounds via AT-SPI, calculates the center point,
    /// and performs a click at that location.
    ///
    /// # Arguments
    ///
    /// * `element_id` - The AT-SPI element path (e.g., "/0/1/2")
    /// * `button` - Mouse button to click
    /// * `clicks` - Number of clicks
    #[instrument(skip(self))]
    pub fn click_element(&self, element_id: &str, button: MouseButton, clicks: u32) -> Result<()> {
        debug!(element_id, ?button, clicks, "Clicking element by ID");

        // Create AT-SPI client with same display
        let atspi = ATSPIClient::with_display(&self.display);

        // Query for the specific element to get its bounds
        // We need to get the element tree and find our element
        let tree = atspi
            .get_element_tree(Some(element_id), Some(1))
            .context("Failed to get element from AT-SPI")?;

        // Extract the element's bounds from the tree
        let element = tree
            .get("element")
            .context("Element not found in AT-SPI tree")?;

        let x = element
            .get("x")
            .and_then(|v| v.as_i64())
            .context("Failed to get element X coordinate")? as i32;
        let y = element
            .get("y")
            .and_then(|v| v.as_i64())
            .context("Failed to get element Y coordinate")? as i32;
        let width = element
            .get("width")
            .and_then(|v| v.as_i64())
            .context("Failed to get element width")? as i32;
        let height = element
            .get("height")
            .and_then(|v| v.as_i64())
            .context("Failed to get element height")? as i32;

        // Calculate center point
        let center_x = x + width / 2;
        let center_y = y + height / 2;

        debug!(
            element_id,
            x, y, width, height, center_x, center_y, "Element bounds resolved"
        );

        // Click at center
        self.click_at(center_x, center_y, button, clicks)
    }

    /// Types text using keyboard simulation.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to type
    /// * `delay_ms` - Delay between keystrokes in milliseconds (0 for no delay)
    #[instrument(skip(self))]
    pub fn type_text(&self, text: &str, delay_ms: u32) -> Result<()> {
        debug!(text_len = text.len(), delay_ms, "Typing text");

        let delay_str = delay_ms.to_string();

        // Use -- to prevent xdotool from interpreting text as options
        self.run_xdotool(&["type", "--delay", &delay_str, "--", text])?;

        Ok(())
    }

    /// Presses a key combination.
    ///
    /// # Arguments
    ///
    /// * `keys` - Array of key names to press together (e.g., ["ctrl", "c"])
    ///
    /// Keys are joined with '+' for xdotool (e.g., "ctrl+c").
    #[instrument(skip(self))]
    pub fn key_press(&self, keys: &[&str]) -> Result<()> {
        if keys.is_empty() {
            anyhow::bail!("No keys specified for key_press");
        }

        // Join keys with + for xdotool format
        let combo = keys.join("+");
        debug!(combo = %combo, "Pressing key combination");

        self.run_xdotool(&["key", &combo])?;

        Ok(())
    }

    /// Moves the mouse cursor to a position.
    ///
    /// # Arguments
    ///
    /// * `x` - Target X coordinate
    /// * `y` - Target Y coordinate
    /// * `smooth` - If true, waits for the motion to complete (--sync)
    #[instrument(skip(self))]
    pub fn move_mouse(&self, x: i32, y: i32, smooth: bool) -> Result<()> {
        debug!(x, y, smooth, "Moving mouse");

        let x_str = x.to_string();
        let y_str = y.to_string();

        if smooth {
            self.run_xdotool(&["mousemove", "--sync", &x_str, &y_str])?;
        } else {
            self.run_xdotool(&["mousemove", &x_str, &y_str])?;
        }

        Ok(())
    }

    /// Focuses a window by name.
    ///
    /// Searches for windows matching the given name and activates the first match.
    ///
    /// # Arguments
    ///
    /// * `window_name` - Name (or partial name) of the window to focus
    #[instrument(skip(self))]
    pub fn focus_window(&self, window_name: &str) -> Result<()> {
        debug!(window_name, "Focusing window");

        // Search for window and activate it
        self.run_xdotool(&["search", "--name", window_name, "windowactivate"])?;

        Ok(())
    }

    /// Gets the current mouse cursor position.
    ///
    /// Returns the X and Y coordinates of the mouse cursor.
    #[instrument(skip(self))]
    pub fn get_mouse_position(&self) -> Result<MousePosition> {
        debug!("Getting mouse position");

        let output = self.run_xdotool(&["getmouselocation", "--shell"])?;

        // Parse output format: X=123\nY=456\n...
        let mut x: Option<i32> = None;
        let mut y: Option<i32> = None;

        for line in output.lines() {
            if let Some(value) = line.strip_prefix("X=") {
                x = Some(value.parse().context("Failed to parse X coordinate")?);
            } else if let Some(value) = line.strip_prefix("Y=") {
                y = Some(value.parse().context("Failed to parse Y coordinate")?);
            }
        }

        let x = x.context("X coordinate not found in xdotool output")?;
        let y = y.context("Y coordinate not found in xdotool output")?;

        debug!(x, y, "Mouse position retrieved");

        Ok(MousePosition { x, y })
    }

    /// Clears the current text field by selecting all and deleting.
    ///
    /// Sends Ctrl+A to select all, then Delete to remove the selection.
    #[instrument(skip(self))]
    pub fn clear_field(&self) -> Result<()> {
        debug!("Clearing text field");

        // Select all
        self.key_press(&["ctrl", "a"])?;

        // Small delay to ensure selection completes
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Delete selection
        self.key_press(&["Delete"])?;

        Ok(())
    }

    /// Performs a drag operation from one point to another.
    ///
    /// # Arguments
    ///
    /// * `from_x` - Starting X coordinate
    /// * `from_y` - Starting Y coordinate
    /// * `to_x` - Ending X coordinate
    /// * `to_y` - Ending Y coordinate
    /// * `button` - Mouse button to hold during drag
    #[instrument(skip(self))]
    pub fn drag(&self, from_x: i32, from_y: i32, to_x: i32, to_y: i32, button: MouseButton) -> Result<()> {
        debug!(from_x, from_y, to_x, to_y, ?button, "Performing drag");

        // Move to start position
        self.run_xdotool(&["mousemove", "--sync", &from_x.to_string(), &from_y.to_string()])?;

        // Press button down
        self.run_xdotool(&["mousedown", button.as_xdotool_button()])?;

        // Move to end position
        self.run_xdotool(&["mousemove", "--sync", &to_x.to_string(), &to_y.to_string()])?;

        // Release button
        self.run_xdotool(&["mouseup", button.as_xdotool_button()])?;

        Ok(())
    }

    /// Scrolls the mouse wheel.
    ///
    /// # Arguments
    ///
    /// * `direction` - "up" or "down"
    /// * `clicks` - Number of scroll clicks
    #[instrument(skip(self))]
    pub fn scroll(&self, direction: &str, clicks: u32) -> Result<()> {
        debug!(direction, clicks, "Scrolling");

        let button = match direction.to_lowercase().as_str() {
            "up" => "4",
            "down" => "5",
            _ => anyhow::bail!("Invalid scroll direction: {}. Use 'up' or 'down'", direction),
        };

        let repeat_str = clicks.to_string();
        self.run_xdotool(&["click", "--repeat", &repeat_str, button])?;

        Ok(())
    }

    /// Gets the currently active window ID.
    #[instrument(skip(self))]
    pub fn get_active_window(&self) -> Result<String> {
        debug!("Getting active window");

        let output = self.run_xdotool(&["getactivewindow"])?;
        Ok(output.trim().to_string())
    }

    /// Gets the title of a window by its ID.
    #[instrument(skip(self))]
    pub fn get_window_name(&self, window_id: &str) -> Result<String> {
        debug!(window_id, "Getting window name");

        let output = self.run_xdotool(&["getwindowname", window_id])?;
        Ok(output.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_button_conversion() {
        assert_eq!(MouseButton::Left.as_xdotool_button(), "1");
        assert_eq!(MouseButton::Middle.as_xdotool_button(), "2");
        assert_eq!(MouseButton::Right.as_xdotool_button(), "3");
    }

    #[test]
    fn test_mouse_button_from_str() {
        assert_eq!(MouseButton::from_str("left"), Some(MouseButton::Left));
        assert_eq!(MouseButton::from_str("LEFT"), Some(MouseButton::Left));
        assert_eq!(MouseButton::from_str("1"), Some(MouseButton::Left));
        assert_eq!(MouseButton::from_str("right"), Some(MouseButton::Right));
        assert_eq!(MouseButton::from_str("3"), Some(MouseButton::Right));
        assert_eq!(MouseButton::from_str("middle"), Some(MouseButton::Middle));
        assert_eq!(MouseButton::from_str("2"), Some(MouseButton::Middle));
        assert_eq!(MouseButton::from_str("invalid"), None);
    }

    #[test]
    fn test_mouse_button_default() {
        assert_eq!(MouseButton::default(), MouseButton::Left);
    }

    #[test]
    fn test_xdotool_default() {
        let tool = XDoTool::new();
        assert_eq!(tool.display, ":1");
    }

    #[test]
    fn test_xdotool_with_display() {
        let tool = XDoTool::with_display(":0");
        assert_eq!(tool.display, ":0");
    }

    #[test]
    fn test_mouse_position() {
        let pos = MousePosition { x: 100, y: 200 };
        assert_eq!(pos.x, 100);
        assert_eq!(pos.y, 200);
    }

    /// Integration test that requires xdotool and X display.
    /// Run with: cargo test -- --ignored
    #[test]
    #[ignore]
    fn test_get_mouse_position_integration() {
        let tool = XDoTool::new();
        match tool.get_mouse_position() {
            Ok(pos) => {
                println!("Mouse position: ({}, {})", pos.x, pos.y);
                assert!(pos.x >= 0);
                assert!(pos.y >= 0);
            }
            Err(e) => {
                eprintln!("Failed to get mouse position (expected if no X display): {}", e);
            }
        }
    }

    /// Integration test for typing.
    #[test]
    #[ignore]
    fn test_type_text_integration() {
        let tool = XDoTool::new();
        match tool.type_text("Hello, World!", 10) {
            Ok(()) => println!("Text typed successfully"),
            Err(e) => eprintln!("Failed to type text: {}", e),
        }
    }

    /// Integration test for key press.
    #[test]
    #[ignore]
    fn test_key_press_integration() {
        let tool = XDoTool::new();
        match tool.key_press(&["ctrl", "a"]) {
            Ok(()) => println!("Key press successful"),
            Err(e) => eprintln!("Failed to press keys: {}", e),
        }
    }

    /// Integration test for mouse movement.
    #[test]
    #[ignore]
    fn test_move_mouse_integration() {
        let tool = XDoTool::new();
        match tool.move_mouse(500, 500, true) {
            Ok(()) => println!("Mouse moved successfully"),
            Err(e) => eprintln!("Failed to move mouse: {}", e),
        }
    }

    /// Integration test for clicking.
    #[test]
    #[ignore]
    fn test_click_at_integration() {
        let tool = XDoTool::new();
        match tool.click_at(100, 100, MouseButton::Left, 1) {
            Ok(()) => println!("Click successful"),
            Err(e) => eprintln!("Failed to click: {}", e),
        }
    }

    /// Integration test for getting active window.
    #[test]
    #[ignore]
    fn test_get_active_window_integration() {
        let tool = XDoTool::new();
        match tool.get_active_window() {
            Ok(window_id) => {
                println!("Active window ID: {}", window_id);
                // Try to get the window name
                if let Ok(name) = tool.get_window_name(&window_id) {
                    println!("Window name: {}", name);
                }
            }
            Err(e) => eprintln!("Failed to get active window: {}", e),
        }
    }
}
