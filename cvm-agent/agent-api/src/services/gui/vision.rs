//! Vision-based element detection for GUI automation.
//!
//! This module provides vision-based element detection capabilities
//! by integrating with vision AI models (via Redpill API) to identify UI elements.
//! It serves as a fallback when accessibility tree inspection is unavailable
//! or when elements cannot be found through AT-SPI.

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, instrument, warn};

/// Configuration for the vision API.
#[derive(Debug, Clone)]
pub struct VisionConfig {
    /// API key for authentication.
    pub api_key: String,
    /// Base URL for the API (e.g., "https://api.redpill.ai").
    pub base_url: String,
    /// Model to use for vision tasks (e.g., "claude-3-5-sonnet-20241022").
    pub model: String,
}

impl VisionConfig {
    /// Creates a new vision configuration from environment variables.
    ///
    /// Reads from:
    /// - REDPILL_API_KEY (required)
    /// - REDPILL_BASE_URL (defaults to "https://api.redpill.ai")
    /// - REDPILL_MODEL (defaults to "claude-3-5-sonnet-20241022")
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("REDPILL_API_KEY")
            .or_else(|_| std::env::var("ANTHROPIC_AUTH_TOKEN"))
            .map_err(|_| anyhow::anyhow!("REDPILL_API_KEY or ANTHROPIC_AUTH_TOKEN required"))?;

        let base_url = std::env::var("REDPILL_BASE_URL")
            .or_else(|_| std::env::var("ANTHROPIC_BASE_URL"))
            .unwrap_or_else(|_| "https://api.redpill.ai".to_string());

        let model = std::env::var("REDPILL_MODEL")
            .or_else(|_| std::env::var("ANTHROPIC_MODEL"))
            .unwrap_or_else(|_| "claude-3-5-sonnet-20241022".to_string());

        Ok(Self {
            api_key,
            base_url,
            model,
        })
    }

    /// Creates a configuration with explicit values.
    pub fn new(api_key: impl Into<String>, base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            model: model.into(),
        }
    }
}

/// An element identified by vision-based analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentifiedElement {
    /// Human-readable description of the element.
    pub description: String,
    /// X coordinate of the element's top-left corner in pixels.
    pub x: i32,
    /// Y coordinate of the element's top-left corner in pixels.
    pub y: i32,
    /// Width of the element in pixels.
    pub width: i32,
    /// Height of the element in pixels.
    pub height: i32,
    /// Confidence score (0.0 to 1.0) of the element identification.
    pub confidence: f32,
}

impl IdentifiedElement {
    /// Returns the center point of the element.
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    /// Returns the bounding box as (x, y, width, height).
    pub fn bounds(&self) -> (i32, i32, i32, i32) {
        (self.x, self.y, self.width, self.height)
    }
}

/// Response format expected from the vision API for element location.
#[derive(Debug, Deserialize)]
struct ElementLocationResponse {
    /// X position as percentage of screen width (0.0 to 1.0).
    x_percent: f32,
    /// Y position as percentage of screen height (0.0 to 1.0).
    y_percent: f32,
    /// Width as percentage of screen width (0.0 to 1.0).
    width_percent: f32,
    /// Height as percentage of screen height (0.0 to 1.0).
    height_percent: f32,
    /// Confidence score (0.0 to 1.0).
    confidence: f32,
    /// Optional description of the found element.
    description: Option<String>,
}

/// Request structure for Anthropic-compatible vision API.
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
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Serialize)]
struct ImageSource {
    #[serde(rename = "type")]
    source_type: String,
    media_type: String,
    data: String,
}

/// Response structure from Anthropic-compatible API.
#[derive(Debug, Deserialize)]
struct VisionResponse {
    content: Vec<ResponseContent>,
}

#[derive(Debug, Deserialize)]
struct ResponseContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

/// Vision-based element detection service.
///
/// Uses vision AI models to identify and locate UI elements in screenshots.
/// This is useful as a fallback when accessibility tree inspection is unavailable.
#[derive(Clone)]
pub struct VisionService {
    client: Client,
    config: VisionConfig,
}

impl VisionService {
    /// Creates a new vision service with the given configuration.
    pub fn new(config: VisionConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client, config })
    }

    /// Creates a new vision service from environment variables.
    pub fn from_env() -> Result<Self> {
        let config = VisionConfig::from_env()?;
        Self::new(config)
    }

    /// Describes the contents of a screenshot.
    ///
    /// # Arguments
    /// * `screenshot_base64` - Base64-encoded screenshot image data
    /// * `focus_area` - Optional area to focus description on (e.g., "top toolbar", "center of screen")
    /// * `question` - Optional specific question about the screenshot
    ///
    /// # Returns
    /// A description of the UI elements visible in the screenshot.
    #[instrument(skip(self, screenshot_base64))]
    pub async fn describe_screenshot(
        &self,
        screenshot_base64: &str,
        focus_area: Option<&str>,
        question: Option<&str>,
    ) -> Result<String> {
        let prompt = self.build_description_prompt(focus_area, question);

        debug!(prompt_length = prompt.len(), "Sending screenshot for description");

        let response = self.send_vision_request(screenshot_base64, &prompt).await?;

        Ok(response)
    }

    /// Finds an element in a screenshot based on a description.
    ///
    /// # Arguments
    /// * `screenshot_base64` - Base64-encoded screenshot image data
    /// * `description` - Description of the element to find (e.g., "blue submit button", "search input field")
    /// * `screen_width` - Width of the screen in pixels
    /// * `screen_height` - Height of the screen in pixels
    ///
    /// # Returns
    /// The identified element with pixel coordinates, or None if not found.
    #[instrument(skip(self, screenshot_base64))]
    pub async fn find_element(
        &self,
        screenshot_base64: &str,
        description: &str,
        screen_width: u32,
        screen_height: u32,
    ) -> Result<Option<IdentifiedElement>> {
        let prompt = self.build_element_location_prompt(description);

        debug!(
            description = description,
            screen_width = screen_width,
            screen_height = screen_height,
            "Searching for element in screenshot"
        );

        let response = self.send_vision_request(screenshot_base64, &prompt).await?;

        // Parse JSON from response
        let element = self.parse_element_response(&response, description, screen_width, screen_height)?;

        Ok(element)
    }

    /// Sends a vision request to the API.
    async fn send_vision_request(&self, screenshot_base64: &str, prompt: &str) -> Result<String> {
        let request = VisionRequest {
            model: self.config.model.clone(),
            max_tokens: 1024,
            messages: vec![VisionMessage {
                role: "user".to_string(),
                content: vec![
                    ContentBlock::Image {
                        source: ImageSource {
                            source_type: "base64".to_string(),
                            media_type: "image/jpeg".to_string(),
                            data: screenshot_base64.to_string(),
                        },
                    },
                    ContentBlock::Text {
                        text: prompt.to_string(),
                    },
                ],
            }],
        };

        let response = self
            .client
            .post(format!("{}/v1/messages", self.config.base_url))
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send vision API request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Vision API request failed ({}): {}", status, error_text);
        }

        let api_response: VisionResponse = response
            .json()
            .await
            .context("Failed to parse vision API response")?;

        // Extract text from response
        let text = api_response
            .content
            .into_iter()
            .filter_map(|c| {
                if c.content_type == "text" {
                    c.text
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        if text.is_empty() {
            anyhow::bail!("Vision API returned empty response");
        }

        Ok(text)
    }

    /// Builds a prompt for describing screenshot contents.
    fn build_description_prompt(&self, focus_area: Option<&str>, question: Option<&str>) -> String {
        let mut prompt = String::from(
            "Analyze this screenshot and describe the UI elements you see. \
            For each visible element, provide:\n\
            - Type (button, text field, label, menu, icon, etc.)\n\
            - Approximate location (top-left, center, bottom-right, etc.)\n\
            - Any visible text or labels\n\
            - Visual characteristics (color, size, state)\n\n"
        );

        if let Some(area) = focus_area {
            prompt.push_str(&format!("Focus particularly on the {} area.\n\n", area));
        }

        if let Some(q) = question {
            prompt.push_str(&format!("Specifically, please answer: {}\n\n", q));
        }

        prompt.push_str("Provide a clear, structured description.");

        prompt
    }

    /// Builds a prompt for locating a specific element.
    fn build_element_location_prompt(&self, description: &str) -> String {
        format!(
            r#"Find the UI element matching this description: "{}"

Analyze the screenshot and locate the element. Respond with ONLY a JSON object in this exact format:
{{
    "x_percent": <float 0.0-1.0>,
    "y_percent": <float 0.0-1.0>,
    "width_percent": <float 0.0-1.0>,
    "height_percent": <float 0.0-1.0>,
    "confidence": <float 0.0-1.0>,
    "description": "<brief description of what you found>"
}}

Where:
- x_percent, y_percent: Position of top-left corner as percentage of screen dimensions
- width_percent, height_percent: Size as percentage of screen dimensions
- confidence: How confident you are this is the correct element (1.0 = certain)
- description: Brief description of the found element

If the element cannot be found, respond with:
{{"confidence": 0.0, "description": "Element not found"}}

Respond with ONLY the JSON object, no other text."#,
            description
        )
    }

    /// Parses the element location response from the API.
    fn parse_element_response(
        &self,
        response: &str,
        original_description: &str,
        screen_width: u32,
        screen_height: u32,
    ) -> Result<Option<IdentifiedElement>> {
        // Try to extract JSON from the response
        let json_str = self.extract_json(response);

        let location: ElementLocationResponse = match serde_json::from_str(&json_str) {
            Ok(loc) => loc,
            Err(e) => {
                warn!(
                    error = %e,
                    response = response,
                    "Failed to parse element location response"
                );
                return Ok(None);
            }
        };

        // If confidence is too low, element wasn't found
        if location.confidence < 0.1 {
            debug!(
                confidence = location.confidence,
                "Element not found (low confidence)"
            );
            return Ok(None);
        }

        // Convert percentages to pixel coordinates
        let x = (location.x_percent * screen_width as f32).round() as i32;
        let y = (location.y_percent * screen_height as f32).round() as i32;
        let width = (location.width_percent * screen_width as f32).round() as i32;
        let height = (location.height_percent * screen_height as f32).round() as i32;

        let element = IdentifiedElement {
            description: location.description.unwrap_or_else(|| original_description.to_string()),
            x,
            y,
            width: width.max(1),
            height: height.max(1),
            confidence: location.confidence,
        };

        debug!(
            description = %element.description,
            x = element.x,
            y = element.y,
            width = element.width,
            height = element.height,
            confidence = element.confidence,
            "Found element"
        );

        Ok(Some(element))
    }

    /// Extracts JSON from a response that might contain additional text.
    fn extract_json(&self, response: &str) -> String {
        // Try to find JSON object in the response
        let trimmed = response.trim();

        // If it already looks like valid JSON, return as-is
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            return trimmed.to_string();
        }

        // Try to find JSON object within the text
        if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                if end > start {
                    return trimmed[start..=end].to_string();
                }
            }
        }

        // Return original if no JSON found
        trimmed.to_string()
    }
}

impl std::fmt::Debug for VisionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VisionService")
            .field("base_url", &self.config.base_url)
            .field("model", &self.config.model)
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vision_config_new() {
        let config = VisionConfig::new("test-key", "https://api.test.com", "test-model");
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.base_url, "https://api.test.com");
        assert_eq!(config.model, "test-model");
    }

    #[test]
    fn test_identified_element_center() {
        let element = IdentifiedElement {
            description: "test button".to_string(),
            x: 100,
            y: 200,
            width: 50,
            height: 30,
            confidence: 0.95,
        };

        let (cx, cy) = element.center();
        assert_eq!(cx, 125); // 100 + 50/2
        assert_eq!(cy, 215); // 200 + 30/2
    }

    #[test]
    fn test_identified_element_bounds() {
        let element = IdentifiedElement {
            description: "test".to_string(),
            x: 10,
            y: 20,
            width: 100,
            height: 50,
            confidence: 0.9,
        };

        assert_eq!(element.bounds(), (10, 20, 100, 50));
    }

    #[test]
    fn test_extract_json_clean() {
        let config = VisionConfig::new("key", "url", "model");
        let service = VisionService {
            client: Client::new(),
            config,
        };

        let json = r#"{"x_percent": 0.5, "y_percent": 0.3}"#;
        assert_eq!(service.extract_json(json), json);
    }

    #[test]
    fn test_extract_json_with_prefix() {
        let config = VisionConfig::new("key", "url", "model");
        let service = VisionService {
            client: Client::new(),
            config,
        };

        let response = r#"Here is the JSON:
{"x_percent": 0.5, "y_percent": 0.3}"#;
        let extracted = service.extract_json(response);
        assert!(extracted.starts_with('{'));
        assert!(extracted.ends_with('}'));
    }

    #[test]
    fn test_extract_json_with_surrounding_text() {
        let config = VisionConfig::new("key", "url", "model");
        let service = VisionService {
            client: Client::new(),
            config,
        };

        let response = r#"I found the element. {"x_percent": 0.5} Let me explain."#;
        let extracted = service.extract_json(response);
        assert_eq!(extracted, r#"{"x_percent": 0.5}"#);
    }

    #[test]
    fn test_parse_element_response() {
        let config = VisionConfig::new("key", "url", "model");
        let service = VisionService {
            client: Client::new(),
            config,
        };

        let response = r#"{
            "x_percent": 0.25,
            "y_percent": 0.5,
            "width_percent": 0.1,
            "height_percent": 0.05,
            "confidence": 0.95,
            "description": "Submit button"
        }"#;

        let result = service.parse_element_response(response, "button", 1920, 1080);
        assert!(result.is_ok());

        let element = result.unwrap();
        assert!(element.is_some());

        let element = element.unwrap();
        assert_eq!(element.x, 480);   // 0.25 * 1920
        assert_eq!(element.y, 540);   // 0.5 * 1080
        assert_eq!(element.width, 192);  // 0.1 * 1920
        assert_eq!(element.height, 54);  // 0.05 * 1080
        assert_eq!(element.confidence, 0.95);
        assert_eq!(element.description, "Submit button");
    }

    #[test]
    fn test_parse_element_response_not_found() {
        let config = VisionConfig::new("key", "url", "model");
        let service = VisionService {
            client: Client::new(),
            config,
        };

        let response = r#"{"confidence": 0.0, "description": "Element not found"}"#;

        let result = service.parse_element_response(response, "button", 1920, 1080);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_element_response_invalid_json() {
        let config = VisionConfig::new("key", "url", "model");
        let service = VisionService {
            client: Client::new(),
            config,
        };

        let response = "This is not valid JSON";

        let result = service.parse_element_response(response, "button", 1920, 1080);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_build_description_prompt_basic() {
        let config = VisionConfig::new("key", "url", "model");
        let service = VisionService {
            client: Client::new(),
            config,
        };

        let prompt = service.build_description_prompt(None, None);
        assert!(prompt.contains("UI elements"));
        assert!(prompt.contains("Type"));
        assert!(prompt.contains("location"));
    }

    #[test]
    fn test_build_description_prompt_with_focus() {
        let config = VisionConfig::new("key", "url", "model");
        let service = VisionService {
            client: Client::new(),
            config,
        };

        let prompt = service.build_description_prompt(Some("toolbar"), None);
        assert!(prompt.contains("toolbar"));
    }

    #[test]
    fn test_build_description_prompt_with_question() {
        let config = VisionConfig::new("key", "url", "model");
        let service = VisionService {
            client: Client::new(),
            config,
        };

        let prompt = service.build_description_prompt(None, Some("Is there a save button?"));
        assert!(prompt.contains("Is there a save button?"));
    }

    #[test]
    fn test_build_element_location_prompt() {
        let config = VisionConfig::new("key", "url", "model");
        let service = VisionService {
            client: Client::new(),
            config,
        };

        let prompt = service.build_element_location_prompt("blue submit button");
        assert!(prompt.contains("blue submit button"));
        assert!(prompt.contains("x_percent"));
        assert!(prompt.contains("y_percent"));
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn test_vision_service_debug() {
        let config = VisionConfig::new("secret-key", "https://api.test.com", "model");
        let service = VisionService {
            client: Client::new(),
            config,
        };

        let debug_str = format!("{:?}", service);
        assert!(debug_str.contains("VisionService"));
        assert!(debug_str.contains("[REDACTED]"));
        assert!(!debug_str.contains("secret-key"));
    }
}
