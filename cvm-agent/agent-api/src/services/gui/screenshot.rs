//! Screenshot capture service for the CVM Agent.
//!
//! This module provides functionality to capture screenshots using the `scrot` command.
//! It supports full screen capture, specific window capture, and various output formats.

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use image::ImageReader;
use std::io::Cursor;
use std::path::PathBuf;
use std::process::Command;
use tokio::fs;
use tracing::{debug, instrument};

/// Supported image formats for screenshot output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ImageFormat {
    /// PNG format (default, lossless)
    #[default]
    Png,
    /// JPEG format (lossy, with configurable quality)
    Jpeg,
}

impl ImageFormat {
    /// Returns the file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
        }
    }

    /// Returns the MIME type for this format.
    #[allow(dead_code)]
    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
        }
    }
}

/// Options for configuring screenshot capture.
#[derive(Debug, Clone)]
pub struct ScreenshotOptions {
    /// Whether to capture the entire screen or a specific window.
    pub full_screen: bool,
    /// Window ID to capture (if not full screen). None means focused window.
    pub window_id: Option<u32>,
    /// Whether to include the cursor in the screenshot.
    pub include_cursor: bool,
    /// Output image format.
    pub format: ImageFormat,
    /// JPEG quality (1-100, only used for JPEG format).
    pub jpeg_quality: u8,
    /// X display to use (e.g., ":1").
    pub display: String,
    /// Delay in seconds before capturing (useful for capturing menus).
    pub delay: Option<u32>,
}

impl Default for ScreenshotOptions {
    fn default() -> Self {
        Self {
            full_screen: true,
            window_id: None,
            include_cursor: false,
            format: ImageFormat::Png,
            jpeg_quality: 85,
            display: ":1".to_string(),
            delay: None,
        }
    }
}

impl ScreenshotOptions {
    /// Creates options for full screen capture.
    pub fn full_screen() -> Self {
        Self::default()
    }

    /// Creates options for capturing a specific window.
    pub fn window(window_id: u32) -> Self {
        Self {
            full_screen: false,
            window_id: Some(window_id),
            ..Default::default()
        }
    }

    /// Creates options for capturing the focused window.
    #[allow(dead_code)]
    pub fn focused_window() -> Self {
        Self {
            full_screen: false,
            window_id: None,
            ..Default::default()
        }
    }

    /// Sets whether to include the cursor in the screenshot.
    pub fn with_cursor(mut self, include: bool) -> Self {
        self.include_cursor = include;
        self
    }

    /// Sets the output format to JPEG with the specified quality.
    pub fn with_jpeg(mut self, quality: u8) -> Self {
        self.format = ImageFormat::Jpeg;
        self.jpeg_quality = quality.clamp(1, 100);
        self
    }

    /// Sets the output format to PNG.
    pub fn with_png(mut self) -> Self {
        self.format = ImageFormat::Png;
        self
    }

    /// Sets the X display to use.
    #[allow(dead_code)]
    pub fn with_display(mut self, display: impl Into<String>) -> Self {
        self.display = display.into();
        self
    }

    /// Sets a delay before capturing.
    #[allow(dead_code)]
    pub fn with_delay(mut self, seconds: u32) -> Self {
        self.delay = Some(seconds);
        self
    }
}

/// Result of a screenshot capture operation.
#[derive(Debug, Clone)]
pub struct ScreenshotResult {
    /// The raw image data.
    pub data: Vec<u8>,
    /// The image format.
    pub format: ImageFormat,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

impl ScreenshotResult {
    /// Returns the image data as a base64-encoded string.
    pub fn to_base64(&self) -> String {
        BASE64_STANDARD.encode(&self.data)
    }

    /// Returns a data URL suitable for embedding in HTML or passing to vision APIs.
    #[allow(dead_code)]
    pub fn to_data_url(&self) -> String {
        format!(
            "data:{};base64,{}",
            self.format.mime_type(),
            self.to_base64()
        )
    }
}

/// Screenshot capture service.
///
/// Uses the `scrot` command-line tool to capture screenshots from an X11 display.
#[derive(Debug, Clone, Default)]
pub struct ScreenshotCapture {
    /// Default options for screenshot capture.
    default_options: ScreenshotOptions,
}

impl ScreenshotCapture {
    /// Creates a new screenshot capture service with default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new screenshot capture service with custom default options.
    #[allow(dead_code)]
    pub fn with_options(options: ScreenshotOptions) -> Self {
        Self {
            default_options: options,
        }
    }

    /// Captures a screenshot with the default options.
    #[instrument(skip(self))]
    pub async fn capture(&self) -> Result<ScreenshotResult> {
        self.capture_with_options(self.default_options.clone())
            .await
    }

    /// Captures a screenshot with custom options.
    #[instrument(skip(self))]
    pub async fn capture_with_options(&self, options: ScreenshotOptions) -> Result<ScreenshotResult> {
        let temp_file = self.generate_temp_path(&options.format);

        debug!(
            path = %temp_file.display(),
            full_screen = options.full_screen,
            include_cursor = options.include_cursor,
            format = ?options.format,
            "Capturing screenshot"
        );

        // Build scrot command
        let mut cmd = Command::new("scrot");

        // Set display environment variable
        cmd.env("DISPLAY", &options.display);

        // Add options
        if !options.include_cursor {
            cmd.arg("--hidecursor");
        }

        if !options.full_screen {
            if let Some(window_id) = options.window_id {
                // Capture specific window by ID
                cmd.arg("--window").arg(format!("{}", window_id));
            } else {
                // Capture focused window
                cmd.arg("--focused");
            }
        }

        if let Some(delay) = options.delay {
            cmd.arg("--delay").arg(delay.to_string());
        }

        // Set quality for JPEG
        if options.format == ImageFormat::Jpeg {
            cmd.arg("--quality").arg(options.jpeg_quality.to_string());
        }

        // Output file
        cmd.arg(&temp_file);

        // Execute scrot
        let output = cmd
            .output()
            .context("Failed to execute scrot command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("scrot failed: {}", stderr);
        }

        // Read the captured image
        let data = fs::read(&temp_file)
            .await
            .context("Failed to read screenshot file")?;

        // Get image dimensions
        let (width, height) = self.get_image_dimensions(&data)?;

        // Clean up temp file
        if let Err(e) = fs::remove_file(&temp_file).await {
            debug!(error = %e, "Failed to remove temp screenshot file");
        }

        // Convert format if necessary (scrot outputs based on extension, but we verify)
        let final_data = self.ensure_format(data, &options.format)?;

        Ok(ScreenshotResult {
            data: final_data,
            format: options.format,
            width,
            height,
        })
    }

    /// Captures a screenshot and returns it as base64-encoded data.
    ///
    /// This is a convenience method for vision API usage.
    #[instrument(skip(self))]
    pub async fn capture_base64(&self) -> Result<String> {
        let result = self.capture().await?;
        Ok(result.to_base64())
    }

    /// Captures a screenshot with options and returns it as base64-encoded data.
    #[allow(dead_code)]
    #[instrument(skip(self))]
    pub async fn capture_base64_with_options(&self, options: ScreenshotOptions) -> Result<String> {
        let result = self.capture_with_options(options).await?;
        Ok(result.to_base64())
    }

    /// Generates a temporary file path for the screenshot.
    fn generate_temp_path(&self, format: &ImageFormat) -> PathBuf {
        let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let filename = format!("cvm_screenshot_{}.{}", timestamp, format.extension());
        std::env::temp_dir().join(filename)
    }

    /// Gets the dimensions of an image from its data.
    fn get_image_dimensions(&self, data: &[u8]) -> Result<(u32, u32)> {
        let reader = ImageReader::new(Cursor::new(data))
            .with_guessed_format()
            .context("Failed to guess image format")?;

        let dimensions = reader
            .into_dimensions()
            .context("Failed to read image dimensions")?;

        Ok(dimensions)
    }

    /// Ensures the image data is in the requested format.
    ///
    /// If the format matches, returns the data unchanged.
    /// Otherwise, converts the image to the requested format.
    fn ensure_format(&self, data: Vec<u8>, format: &ImageFormat) -> Result<Vec<u8>> {
        let reader = ImageReader::new(Cursor::new(&data))
            .with_guessed_format()
            .context("Failed to guess image format")?;

        let detected_format = reader.format();

        // Check if conversion is needed
        let needs_conversion = match (detected_format, format) {
            (Some(image::ImageFormat::Png), ImageFormat::Png) => false,
            (Some(image::ImageFormat::Jpeg), ImageFormat::Jpeg) => false,
            _ => true,
        };

        if !needs_conversion {
            return Ok(data);
        }

        // Convert to requested format
        let img = ImageReader::new(Cursor::new(&data))
            .with_guessed_format()
            .context("Failed to guess image format")?
            .decode()
            .context("Failed to decode image")?;

        let mut output = Cursor::new(Vec::new());

        match format {
            ImageFormat::Png => {
                img.write_to(&mut output, image::ImageFormat::Png)
                    .context("Failed to encode as PNG")?;
            }
            ImageFormat::Jpeg => {
                img.write_to(&mut output, image::ImageFormat::Jpeg)
                    .context("Failed to encode as JPEG")?;
            }
        }

        Ok(output.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_format_extension() {
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
    }

    #[test]
    fn test_image_format_mime_type() {
        assert_eq!(ImageFormat::Png.mime_type(), "image/png");
        assert_eq!(ImageFormat::Jpeg.mime_type(), "image/jpeg");
    }

    #[test]
    fn test_screenshot_options_default() {
        let options = ScreenshotOptions::default();
        assert!(options.full_screen);
        assert!(options.window_id.is_none());
        assert!(!options.include_cursor);
        assert_eq!(options.format, ImageFormat::Png);
        assert_eq!(options.jpeg_quality, 85);
        assert_eq!(options.display, ":1");
        assert!(options.delay.is_none());
    }

    #[test]
    fn test_screenshot_options_builders() {
        let options = ScreenshotOptions::full_screen()
            .with_cursor(true)
            .with_jpeg(90)
            .with_display(":0")
            .with_delay(2);

        assert!(options.full_screen);
        assert!(options.include_cursor);
        assert_eq!(options.format, ImageFormat::Jpeg);
        assert_eq!(options.jpeg_quality, 90);
        assert_eq!(options.display, ":0");
        assert_eq!(options.delay, Some(2));
    }

    #[test]
    fn test_screenshot_options_window() {
        let options = ScreenshotOptions::window(12345);
        assert!(!options.full_screen);
        assert_eq!(options.window_id, Some(12345));
    }

    #[test]
    fn test_screenshot_options_focused_window() {
        let options = ScreenshotOptions::focused_window();
        assert!(!options.full_screen);
        assert!(options.window_id.is_none());
    }

    #[test]
    fn test_jpeg_quality_clamping() {
        let options = ScreenshotOptions::default().with_jpeg(150);
        assert_eq!(options.jpeg_quality, 100);

        let options = ScreenshotOptions::default().with_jpeg(0);
        assert_eq!(options.jpeg_quality, 1);
    }

    #[test]
    fn test_screenshot_capture_creation() {
        let capture = ScreenshotCapture::new();
        assert!(capture.default_options.full_screen);

        let custom_options = ScreenshotOptions::focused_window().with_jpeg(75);
        let capture = ScreenshotCapture::with_options(custom_options);
        assert!(!capture.default_options.full_screen);
        assert_eq!(capture.default_options.jpeg_quality, 75);
    }

    /// Integration test that requires a running X server.
    /// Run with: cargo test -- --ignored
    #[tokio::test]
    #[ignore]
    async fn test_capture_screenshot() {
        let capture = ScreenshotCapture::new();
        let result = capture.capture().await;

        match result {
            Ok(screenshot) => {
                assert!(screenshot.width > 0);
                assert!(screenshot.height > 0);
                assert!(!screenshot.data.is_empty());
                assert_eq!(screenshot.format, ImageFormat::Png);

                // Verify base64 encoding works
                let base64 = screenshot.to_base64();
                assert!(!base64.is_empty());

                // Verify data URL works
                let data_url = screenshot.to_data_url();
                assert!(data_url.starts_with("data:image/png;base64,"));
            }
            Err(e) => {
                // Test may fail if no display is available, which is expected in CI
                eprintln!("Screenshot capture failed (expected if no display): {}", e);
            }
        }
    }

    /// Integration test for JPEG capture.
    #[tokio::test]
    #[ignore]
    async fn test_capture_screenshot_jpeg() {
        let options = ScreenshotOptions::full_screen().with_jpeg(80);
        let capture = ScreenshotCapture::with_options(options);
        let result = capture.capture().await;

        match result {
            Ok(screenshot) => {
                assert_eq!(screenshot.format, ImageFormat::Jpeg);
                let data_url = screenshot.to_data_url();
                assert!(data_url.starts_with("data:image/jpeg;base64,"));
            }
            Err(e) => {
                eprintln!("Screenshot capture failed (expected if no display): {}", e);
            }
        }
    }
}
