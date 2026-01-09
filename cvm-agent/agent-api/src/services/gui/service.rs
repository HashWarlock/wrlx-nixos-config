//! Main GUI automation service implementation.
//!
//! This module provides the gRPC service implementation for GUI automation,
//! combining screenshot capture, accessibility inspection, and input automation.

use super::screenshot::ScreenshotCapture;

/// GUI automation service implementation.
///
/// Provides unified access to GUI automation capabilities including:
/// - Screenshot capture
/// - Accessibility tree inspection
/// - Input automation (keyboard/mouse)
/// - Vision-based element detection
#[derive(Debug, Default)]
pub struct GUIServiceImpl {
    screenshot: ScreenshotCapture,
}

impl GUIServiceImpl {
    /// Creates a new GUI service instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a reference to the screenshot capture service.
    pub fn screenshot(&self) -> &ScreenshotCapture {
        &self.screenshot
    }
}

// TODO: Implement gRPC service trait in Phase 3, Task 7
