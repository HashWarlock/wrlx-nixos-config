//! Main GUI automation service implementation.
//!
//! This module provides the gRPC service implementation for GUI automation,
//! combining screenshot capture, accessibility inspection, and input automation.

use std::sync::Arc;
use tonic::{Request, Response, Status};

use super::atspi::{ATSPIClient, ATSPIQueryFilter};
use super::screenshot::{ImageFormat, ScreenshotCapture, ScreenshotOptions};
use super::vision::{VisionConfig, VisionService};
use super::xdotool::{MouseButton, XDoTool};

// Import proto types from health module where they're generated
use crate::services::health::proto::gui_service_server::GuiService;
use crate::services::health::proto::{
    click_request, ActionResponse, BoundingBox, ClickRequest, Coordinates, DescribeRequest,
    ElementNode, ElementQuery, ElementTreeRequest, ElementTreeResponse, ElementsResponse,
    FindByVisionRequest, FindByVisionResponse, KeyPressRequest, MoveMouseRequest,
    ScreenshotRequest, ScreenshotResponse, TypeRequest, UiElement, VisionResponse,
};

/// GUI automation service implementation.
///
/// Provides unified access to GUI automation capabilities including:
/// - Screenshot capture
/// - Accessibility tree inspection
/// - Input automation (keyboard/mouse)
/// - Vision-based element detection
pub struct GUIServiceImpl {
    screenshot: ScreenshotCapture,
    atspi: ATSPIClient,
    xdotool: XDoTool,
    vision: Option<Arc<VisionService>>,
}

impl GUIServiceImpl {
    /// Creates a new GUI service instance.
    ///
    /// The vision service is optional and will be initialized from environment
    /// variables if available.
    pub fn new() -> Self {
        let vision = VisionService::from_env()
            .ok()
            .map(Arc::new);

        Self {
            screenshot: ScreenshotCapture::new(),
            atspi: ATSPIClient::new(),
            xdotool: XDoTool::new(),
            vision,
        }
    }

    /// Creates a GUI service with a specific display.
    #[allow(dead_code)]
    pub fn with_display(display: impl Into<String>) -> Self {
        let display = display.into();
        let vision = VisionService::from_env()
            .ok()
            .map(Arc::new);

        Self {
            screenshot: ScreenshotCapture::with_options(
                ScreenshotOptions::default().with_display(&display),
            ),
            atspi: ATSPIClient::with_display(&display),
            xdotool: XDoTool::with_display(&display),
            vision,
        }
    }

    /// Creates a GUI service with explicit vision configuration.
    #[allow(dead_code)]
    pub fn with_vision(vision_config: VisionConfig) -> Self {
        let vision = VisionService::new(vision_config)
            .ok()
            .map(Arc::new);

        Self {
            screenshot: ScreenshotCapture::new(),
            atspi: ATSPIClient::new(),
            xdotool: XDoTool::new(),
            vision,
        }
    }

    /// Returns a reference to the screenshot capture service.
    #[allow(dead_code)]
    pub fn screenshot(&self) -> &ScreenshotCapture {
        &self.screenshot
    }

    /// Returns a reference to the AT-SPI client.
    #[allow(dead_code)]
    pub fn atspi(&self) -> &ATSPIClient {
        &self.atspi
    }

    /// Returns a reference to the xdotool wrapper.
    #[allow(dead_code)]
    pub fn xdotool(&self) -> &XDoTool {
        &self.xdotool
    }

    /// Returns whether vision service is available.
    #[allow(dead_code)]
    pub fn has_vision(&self) -> bool {
        self.vision.is_some()
    }

    /// Converts MouseButton proto enum to internal MouseButton type.
    fn proto_button_to_internal(button: i32) -> MouseButton {
        match button {
            1 => MouseButton::Right,
            2 => MouseButton::Middle,
            _ => MouseButton::Left,
        }
    }

    /// Converts internal ATSPIElement to proto UIElement.
    fn atspi_element_to_proto(element: &super::atspi::ATSPIElement) -> UiElement {
        UiElement {
            id: element.id.clone(),
            application: element.application.clone(),
            role: element.role.clone(),
            name: element.name.clone(),
            description: element.description.clone(),
            bounds: Some(BoundingBox {
                x: element.x,
                y: element.y,
                width: element.width,
                height: element.height,
            }),
            states: element.states.clone(),
            actions: element.actions.clone(),
        }
    }

    /// Recursively converts JSON tree node to proto ElementNode.
    fn json_to_element_node(value: &serde_json::Value) -> Option<ElementNode> {
        let element_json = value.get("element")?;

        let element = UiElement {
            id: element_json.get("id")?.as_str()?.to_string(),
            application: element_json.get("application").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            role: element_json.get("role").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            name: element_json.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            description: element_json.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            bounds: Some(BoundingBox {
                x: element_json.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                y: element_json.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                width: element_json.get("width").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                height: element_json.get("height").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            }),
            states: element_json
                .get("states")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            actions: element_json
                .get("actions")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
        };

        let children = value
            .get("children")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(Self::json_to_element_node)
                    .collect()
            })
            .unwrap_or_default();

        Some(ElementNode {
            element: Some(element),
            children,
        })
    }
}

impl Default for GUIServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for GUIServiceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GUIServiceImpl")
            .field("screenshot", &self.screenshot)
            .field("atspi", &self.atspi)
            .field("xdotool", &self.xdotool)
            .field("vision", &self.vision.is_some())
            .finish()
    }
}

#[tonic::async_trait]
impl GuiService for GUIServiceImpl {
    /// Captures a screenshot of the screen or a specific window.
    async fn screenshot(
        &self,
        request: Request<ScreenshotRequest>,
    ) -> Result<Response<ScreenshotResponse>, Status> {
        let req = request.into_inner();

        // Build screenshot options from request
        let mut options = if let Some(window_id) = req.window_id {
            if let Ok(id) = window_id.parse::<u32>() {
                ScreenshotOptions::window(id)
            } else {
                ScreenshotOptions::full_screen()
            }
        } else {
            ScreenshotOptions::full_screen()
        };

        options = options.with_cursor(req.include_cursor);

        // Handle format: quality 0 = PNG, 1-100 = JPEG with that quality
        if req.quality > 0 && req.quality <= 100 {
            options = options.with_jpeg(req.quality as u8);
        } else {
            options = options.with_png();
        }

        let result = self
            .screenshot
            .capture_with_options(options)
            .await
            .map_err(|e| Status::internal(format!("Screenshot capture failed: {}", e)))?;

        let format = match result.format {
            ImageFormat::Png => "png".to_string(),
            ImageFormat::Jpeg => "jpeg".to_string(),
        };

        Ok(Response::new(ScreenshotResponse {
            image_data: result.data,
            format,
            width: result.width as i32,
            height: result.height as i32,
        }))
    }

    /// Queries UI elements by application, role, or name.
    async fn get_elements(
        &self,
        request: Request<ElementQuery>,
    ) -> Result<Response<ElementsResponse>, Status> {
        let req = request.into_inner();

        let mut filter = ATSPIQueryFilter::new();

        if let Some(app) = req.application {
            if !app.is_empty() {
                filter = filter.with_application(app);
            }
        }

        if let Some(role) = req.role {
            if !role.is_empty() {
                filter = filter.with_role(role);
            }
        }

        if let Some(name) = req.name {
            if !name.is_empty() {
                filter = filter.with_name(name);
            }
        }

        if req.max_depth > 0 {
            filter = filter.with_max_depth(req.max_depth as u32);
        }

        let elements = self
            .atspi
            .query_elements(&filter)
            .map_err(|e| Status::internal(format!("AT-SPI query failed: {}", e)))?;

        let proto_elements: Vec<UiElement> = elements
            .iter()
            .map(Self::atspi_element_to_proto)
            .collect();

        Ok(Response::new(ElementsResponse {
            elements: proto_elements,
        }))
    }

    /// Gets the full UI element tree.
    async fn get_element_tree(
        &self,
        request: Request<ElementTreeRequest>,
    ) -> Result<Response<ElementTreeResponse>, Status> {
        let req = request.into_inner();

        let max_depth = if req.max_depth > 0 {
            Some(req.max_depth as u32)
        } else {
            None
        };

        let tree = self
            .atspi
            .get_element_tree(req.application.as_deref(), max_depth)
            .map_err(|e| Status::internal(format!("AT-SPI tree query failed: {}", e)))?;

        // Parse the JSON tree structure into proto ElementNode
        let roots = if let Some(apps) = tree.get("applications").and_then(|v| v.as_array()) {
            apps.iter()
                .filter_map(Self::json_to_element_node)
                .collect()
        } else if tree.get("element").is_some() {
            // Single element tree
            Self::json_to_element_node(&tree)
                .map(|n| vec![n])
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(Response::new(ElementTreeResponse { roots }))
    }

    /// Clicks on an element or at specific coordinates.
    async fn click(&self, request: Request<ClickRequest>) -> Result<Response<ActionResponse>, Status> {
        let req = request.into_inner();

        let button = Self::proto_button_to_internal(req.button);
        let clicks = if req.clicks > 0 { req.clicks as u32 } else { 1 };

        let result = match req.target {
            Some(click_request::Target::ElementId(id)) => {
                self.xdotool.click_element(&id, button, clicks)
            }
            Some(click_request::Target::Position(coords)) => {
                self.xdotool.click_at(coords.x, coords.y, button, clicks)
            }
            None => {
                return Ok(Response::new(ActionResponse {
                    success: false,
                    error: "No click target specified".to_string(),
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
                error: format!("Click failed: {}", e),
            })),
        }
    }

    /// Types text into an element or at cursor position.
    async fn r#type(&self, request: Request<TypeRequest>) -> Result<Response<ActionResponse>, Status> {
        let req = request.into_inner();

        // If element_id is specified, click on it first to focus
        if let Some(ref element_id) = req.element_id {
            if !element_id.is_empty() {
                if let Err(e) = self.xdotool.click_element(element_id, MouseButton::Left, 1) {
                    return Ok(Response::new(ActionResponse {
                        success: false,
                        error: format!("Failed to focus element: {}", e),
                    }));
                }
                // Small delay to ensure focus
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }

        // Clear field if requested
        if req.clear_first {
            if let Err(e) = self.xdotool.clear_field() {
                return Ok(Response::new(ActionResponse {
                    success: false,
                    error: format!("Failed to clear field: {}", e),
                }));
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        // Type the text
        let delay = req.delay_ms as u32;
        match self.xdotool.type_text(&req.text, delay) {
            Ok(()) => Ok(Response::new(ActionResponse {
                success: true,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(ActionResponse {
                success: false,
                error: format!("Type failed: {}", e),
            })),
        }
    }

    /// Presses keyboard keys.
    async fn key_press(
        &self,
        request: Request<KeyPressRequest>,
    ) -> Result<Response<ActionResponse>, Status> {
        let req = request.into_inner();

        // If element_id is specified, click on it first to focus
        if let Some(ref element_id) = req.element_id {
            if !element_id.is_empty() {
                if let Err(e) = self.xdotool.click_element(element_id, MouseButton::Left, 1) {
                    return Ok(Response::new(ActionResponse {
                        success: false,
                        error: format!("Failed to focus element: {}", e),
                    }));
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }

        // Convert keys to &str slice
        let keys: Vec<&str> = req.keys.iter().map(|s| s.as_str()).collect();

        match self.xdotool.key_press(&keys) {
            Ok(()) => Ok(Response::new(ActionResponse {
                success: true,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(ActionResponse {
                success: false,
                error: format!("Key press failed: {}", e),
            })),
        }
    }

    /// Moves the mouse cursor to specified coordinates.
    async fn move_mouse(
        &self,
        request: Request<MoveMouseRequest>,
    ) -> Result<Response<ActionResponse>, Status> {
        let req = request.into_inner();

        match self.xdotool.move_mouse(req.x, req.y, req.smooth) {
            Ok(()) => Ok(Response::new(ActionResponse {
                success: true,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(ActionResponse {
                success: false,
                error: format!("Mouse move failed: {}", e),
            })),
        }
    }

    /// Describes screen content using vision model.
    async fn describe(
        &self,
        request: Request<DescribeRequest>,
    ) -> Result<Response<VisionResponse>, Status> {
        let req = request.into_inner();

        let vision = self.vision.as_ref().ok_or_else(|| {
            Status::unavailable("Vision service not configured. Set REDPILL_API_KEY environment variable.")
        })?;

        // Use provided screenshot or capture a new one
        let screenshot_base64 = if !req.screenshot.is_empty() {
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &req.screenshot)
        } else {
            self.screenshot
                .capture_base64()
                .await
                .map_err(|e| Status::internal(format!("Failed to capture screenshot: {}", e)))?
        };

        // Build focus area description from bounds if provided
        let focus_area = req.focus_area.as_ref().map(|bounds| {
            format!(
                "area at ({}, {}) with size {}x{}",
                bounds.x, bounds.y, bounds.width, bounds.height
            )
        });

        let description = vision
            .describe_screenshot(
                &screenshot_base64,
                focus_area.as_deref(),
                req.question.as_deref(),
            )
            .await
            .map_err(|e| Status::internal(format!("Vision describe failed: {}", e)))?;

        Ok(Response::new(VisionResponse {
            description,
            elements: Vec::new(), // Elements are not identified in describe, use find_by_vision
        }))
    }

    /// Finds an element by visual description.
    async fn find_by_vision(
        &self,
        request: Request<FindByVisionRequest>,
    ) -> Result<Response<FindByVisionResponse>, Status> {
        let req = request.into_inner();

        let vision = self.vision.as_ref().ok_or_else(|| {
            Status::unavailable("Vision service not configured. Set REDPILL_API_KEY environment variable.")
        })?;

        // Use provided screenshot or capture a new one
        let (screenshot_base64, width, height) = if let Some(ref screenshot_bytes) = req.screenshot {
            if !screenshot_bytes.is_empty() {
                let base64 = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    screenshot_bytes,
                );
                // Try to determine dimensions from image data
                // For simplicity, assume standard dimensions or use defaults
                (base64, 1920, 1080)
            } else {
                let result = self
                    .screenshot
                    .capture()
                    .await
                    .map_err(|e| Status::internal(format!("Failed to capture screenshot: {}", e)))?;
                (result.to_base64(), result.width, result.height)
            }
        } else {
            let result = self
                .screenshot
                .capture()
                .await
                .map_err(|e| Status::internal(format!("Failed to capture screenshot: {}", e)))?;
            (result.to_base64(), result.width, result.height)
        };

        let element = vision
            .find_element(&screenshot_base64, &req.description, width, height)
            .await
            .map_err(|e| Status::internal(format!("Vision find failed: {}", e)))?;

        match element {
            Some(elem) => {
                let (cx, cy) = elem.center();
                Ok(Response::new(FindByVisionResponse {
                    found: true,
                    center: Some(Coordinates { x: cx, y: cy }),
                    bounds: Some(BoundingBox {
                        x: elem.x,
                        y: elem.y,
                        width: elem.width,
                        height: elem.height,
                    }),
                    confidence: elem.confidence,
                    element_description: elem.description,
                }))
            }
            None => Ok(Response::new(FindByVisionResponse {
                found: false,
                center: None,
                bounds: None,
                confidence: 0.0,
                element_description: "Element not found".to_string(),
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gui_service_creation() {
        let service = GUIServiceImpl::new();
        // Verify service was created successfully
        assert!(format!("{:?}", service).contains("GUIServiceImpl"));
    }

    #[test]
    fn test_gui_service_with_display() {
        let service = GUIServiceImpl::with_display(":0");
        // Verify it was created (internal state is private)
        assert!(format!("{:?}", service).contains("GUIServiceImpl"));
    }

    #[test]
    fn test_proto_button_conversion() {
        assert_eq!(
            GUIServiceImpl::proto_button_to_internal(0),
            MouseButton::Left
        );
        assert_eq!(
            GUIServiceImpl::proto_button_to_internal(1),
            MouseButton::Right
        );
        assert_eq!(
            GUIServiceImpl::proto_button_to_internal(2),
            MouseButton::Middle
        );
        assert_eq!(
            GUIServiceImpl::proto_button_to_internal(99),
            MouseButton::Left
        );
    }

    #[test]
    fn test_atspi_element_to_proto() {
        let atspi_element = super::super::atspi::ATSPIElement {
            id: "/0/1".to_string(),
            application: "TestApp".to_string(),
            role: "button".to_string(),
            name: "Submit".to_string(),
            description: "Submit form".to_string(),
            x: 100,
            y: 200,
            width: 80,
            height: 30,
            states: vec!["enabled".to_string()],
            actions: vec!["click".to_string()],
        };

        let proto = GUIServiceImpl::atspi_element_to_proto(&atspi_element);

        assert_eq!(proto.id, "/0/1");
        assert_eq!(proto.application, "TestApp");
        assert_eq!(proto.role, "button");
        assert_eq!(proto.name, "Submit");
        assert_eq!(proto.description, "Submit form");
        assert!(proto.bounds.is_some());

        let bounds = proto.bounds.unwrap();
        assert_eq!(bounds.x, 100);
        assert_eq!(bounds.y, 200);
        assert_eq!(bounds.width, 80);
        assert_eq!(bounds.height, 30);

        assert_eq!(proto.states, vec!["enabled"]);
        assert_eq!(proto.actions, vec!["click"]);
    }

    #[test]
    fn test_json_to_element_node() {
        let json = serde_json::json!({
            "element": {
                "id": "/0",
                "application": "App",
                "role": "frame",
                "name": "Window",
                "description": "",
                "x": 0,
                "y": 0,
                "width": 800,
                "height": 600,
                "states": ["visible"],
                "actions": []
            },
            "children": []
        });

        let node = GUIServiceImpl::json_to_element_node(&json);
        assert!(node.is_some());

        let node = node.unwrap();
        assert!(node.element.is_some());
        assert!(node.children.is_empty());

        let elem = node.element.unwrap();
        assert_eq!(elem.id, "/0");
        assert_eq!(elem.role, "frame");
    }

    #[test]
    fn test_json_to_element_node_with_children() {
        let json = serde_json::json!({
            "element": {
                "id": "/0",
                "application": "App",
                "role": "frame",
                "name": "Window",
                "x": 0,
                "y": 0,
                "width": 800,
                "height": 600,
                "states": [],
                "actions": []
            },
            "children": [
                {
                    "element": {
                        "id": "/0/0",
                        "application": "App",
                        "role": "button",
                        "name": "OK",
                        "x": 100,
                        "y": 100,
                        "width": 80,
                        "height": 30,
                        "states": ["enabled"],
                        "actions": ["click"]
                    },
                    "children": []
                }
            ]
        });

        let node = GUIServiceImpl::json_to_element_node(&json);
        assert!(node.is_some());

        let node = node.unwrap();
        assert_eq!(node.children.len(), 1);

        let child = &node.children[0];
        assert!(child.element.is_some());
        assert_eq!(child.element.as_ref().unwrap().id, "/0/0");
        assert_eq!(child.element.as_ref().unwrap().role, "button");
    }
}
