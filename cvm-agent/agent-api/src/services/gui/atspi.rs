//! AT-SPI (Accessibility) integration for GUI automation.
//!
//! This module provides accessibility tree inspection capabilities using the AT-SPI
//! D-Bus protocol via Python bindings. It enables discovering UI elements, their
//! properties, and available actions for GUI automation.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;
use tracing::{debug, instrument};

/// Represents a single accessible UI element discovered via AT-SPI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ATSPIElement {
    /// Unique identifier for this element (path in accessibility tree).
    pub id: String,
    /// Name of the application this element belongs to.
    pub application: String,
    /// Accessibility role (e.g., "push button", "text", "frame").
    pub role: String,
    /// Human-readable name of the element.
    pub name: String,
    /// Description of the element's purpose.
    pub description: String,
    /// X coordinate of the element on screen.
    pub x: i32,
    /// Y coordinate of the element on screen.
    pub y: i32,
    /// Width of the element in pixels.
    pub width: i32,
    /// Height of the element in pixels.
    pub height: i32,
    /// Current states (e.g., "focused", "enabled", "visible").
    pub states: Vec<String>,
    /// Available actions (e.g., "click", "activate", "press").
    pub actions: Vec<String>,
}

/// Represents a tree node of AT-SPI elements.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ATSPITreeNode {
    /// The element at this node.
    pub element: ATSPIElement,
    /// Child elements.
    pub children: Vec<ATSPITreeNode>,
}

/// Query filters for AT-SPI element discovery.
#[derive(Debug, Clone, Default)]
pub struct ATSPIQueryFilter {
    /// Filter by application name (case-insensitive substring match).
    pub application: Option<String>,
    /// Filter by element role (case-insensitive exact match).
    pub role: Option<String>,
    /// Filter by element name (case-insensitive substring match).
    pub name: Option<String>,
    /// Maximum depth to search in the accessibility tree.
    pub max_depth: Option<u32>,
}

impl ATSPIQueryFilter {
    /// Creates a new empty filter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the application filter.
    pub fn with_application(mut self, application: impl Into<String>) -> Self {
        self.application = Some(application.into());
        self
    }

    /// Sets the role filter.
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }

    /// Sets the name filter.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the maximum search depth.
    pub fn with_max_depth(mut self, depth: u32) -> Self {
        self.max_depth = Some(depth);
        self
    }
}

/// AT-SPI client for accessibility tree inspection.
///
/// Uses Python with PyGObject bindings to access the AT-SPI D-Bus interface.
#[derive(Debug, Clone)]
pub struct ATSPIClient {
    /// X display to connect to.
    display: String,
}

impl Default for ATSPIClient {
    fn default() -> Self {
        Self {
            display: ":1".to_string(),
        }
    }
}

impl ATSPIClient {
    /// Creates a new AT-SPI client with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new AT-SPI client for a specific display.
    #[allow(dead_code)]
    pub fn with_display(display: impl Into<String>) -> Self {
        Self {
            display: display.into(),
        }
    }

    /// Queries accessible elements matching the given filter.
    ///
    /// Searches the accessibility tree and returns all elements matching
    /// the specified criteria.
    #[instrument(skip(self))]
    pub fn query_elements(&self, filter: &ATSPIQueryFilter) -> Result<Vec<ATSPIElement>> {
        let script = self.build_query_script(filter);

        debug!(
            display = %self.display,
            application = ?filter.application,
            role = ?filter.role,
            name = ?filter.name,
            max_depth = ?filter.max_depth,
            "Querying AT-SPI elements"
        );

        let output = Command::new("python3")
            .arg("-c")
            .arg(&script)
            .env("DISPLAY", &self.display)
            .output()
            .context("Failed to execute AT-SPI query script")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("AT-SPI query failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let elements: Vec<ATSPIElement> =
            serde_json::from_str(&stdout).context("Failed to parse AT-SPI query output")?;

        debug!(count = elements.len(), "Found AT-SPI elements");

        Ok(elements)
    }

    /// Gets the accessibility tree starting from the desktop or a specific element.
    ///
    /// Returns a hierarchical view of the accessibility tree structure.
    #[instrument(skip(self))]
    pub fn get_element_tree(
        &self,
        element_id: Option<&str>,
        max_depth: Option<u32>,
    ) -> Result<serde_json::Value> {
        let script = self.build_tree_script(element_id, max_depth);

        debug!(
            display = %self.display,
            element_id = ?element_id,
            max_depth = ?max_depth,
            "Getting AT-SPI element tree"
        );

        let output = Command::new("python3")
            .arg("-c")
            .arg(&script)
            .env("DISPLAY", &self.display)
            .output()
            .context("Failed to execute AT-SPI tree script")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("AT-SPI tree query failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let tree: serde_json::Value =
            serde_json::from_str(&stdout).context("Failed to parse AT-SPI tree output")?;

        Ok(tree)
    }

    /// Builds the Python script for querying elements.
    fn build_query_script(&self, filter: &ATSPIQueryFilter) -> String {
        let app_filter = filter
            .application
            .as_ref()
            .map(|a| format!("'{}'", a.replace('\'', "\\'")))
            .unwrap_or_else(|| "None".to_string());

        let role_filter = filter
            .role
            .as_ref()
            .map(|r| format!("'{}'", r.replace('\'', "\\'")))
            .unwrap_or_else(|| "None".to_string());

        let name_filter = filter
            .name
            .as_ref()
            .map(|n| format!("'{}'", n.replace('\'', "\\'")))
            .unwrap_or_else(|| "None".to_string());

        let max_depth = filter.max_depth.unwrap_or(10);

        format!(
            r#"
import json
import sys

try:
    import gi
    gi.require_version('Atspi', '2.0')
    from gi.repository import Atspi
except Exception as e:
    print(json.dumps([]))
    sys.exit(0)

app_filter = {app_filter}
role_filter = {role_filter}
name_filter = {name_filter}
max_depth = {max_depth}

def get_states(accessible):
    states = []
    try:
        state_set = accessible.get_state_set()
        for state in dir(Atspi.StateType):
            if not state.startswith('_'):
                try:
                    state_type = getattr(Atspi.StateType, state)
                    if state_set.contains(state_type):
                        states.append(state.lower().replace('_', ' '))
                except:
                    pass
    except:
        pass
    return states

def get_actions(accessible):
    actions = []
    try:
        action = accessible.get_action_iface()
        if action:
            n_actions = action.get_n_actions()
            for i in range(n_actions):
                name = action.get_action_name(i)
                if name:
                    actions.append(name)
    except:
        pass
    return actions

def get_element_info(accessible, app_name, path):
    try:
        role = accessible.get_role_name() or ''
        name = accessible.get_name() or ''
        description = accessible.get_description() or ''

        x, y, width, height = 0, 0, 0, 0
        try:
            component = accessible.get_component_iface()
            if component:
                rect = component.get_extents(Atspi.CoordType.SCREEN)
                x, y, width, height = rect.x, rect.y, rect.width, rect.height
        except:
            pass

        return {{
            'id': path,
            'application': app_name,
            'role': role,
            'name': name,
            'description': description,
            'x': x,
            'y': y,
            'width': width,
            'height': height,
            'states': get_states(accessible),
            'actions': get_actions(accessible)
        }}
    except:
        return None

def matches_filter(element):
    if app_filter and app_filter.lower() not in element['application'].lower():
        return False
    if role_filter and role_filter.lower() != element['role'].lower():
        return False
    if name_filter and name_filter.lower() not in element['name'].lower():
        return False
    return True

def search_elements(accessible, app_name, path, depth, results):
    if depth > max_depth:
        return

    element = get_element_info(accessible, app_name, path)
    if element and matches_filter(element):
        results.append(element)

    try:
        n_children = accessible.get_child_count()
        for i in range(n_children):
            child = accessible.get_child_at_index(i)
            if child:
                child_path = f"{{path}}/{{i}}"
                search_elements(child, app_name, child_path, depth + 1, results)
    except:
        pass

results = []

try:
    desktop = Atspi.get_desktop(0)
    if desktop:
        n_apps = desktop.get_child_count()
        for app_idx in range(n_apps):
            try:
                app = desktop.get_child_at_index(app_idx)
                if app:
                    app_name = app.get_name() or f'app_{{app_idx}}'
                    if app_filter and app_filter.lower() not in app_name.lower():
                        continue
                    path = f"/{{app_idx}}"
                    search_elements(app, app_name, path, 0, results)
            except:
                pass
except Exception as e:
    pass

print(json.dumps(results))
"#,
            app_filter = app_filter,
            role_filter = role_filter,
            name_filter = name_filter,
            max_depth = max_depth
        )
    }

    /// Builds the Python script for getting the element tree.
    fn build_tree_script(&self, element_id: Option<&str>, max_depth: Option<u32>) -> String {
        let element_id_str = element_id
            .map(|e| format!("'{}'", e.replace('\'', "\\'")))
            .unwrap_or_else(|| "None".to_string());

        let max_depth = max_depth.unwrap_or(5);

        format!(
            r#"
import json
import sys

try:
    import gi
    gi.require_version('Atspi', '2.0')
    from gi.repository import Atspi
except Exception as e:
    print(json.dumps({{"error": "AT-SPI not available"}}))
    sys.exit(0)

element_id = {element_id}
max_depth = {max_depth}

def get_states(accessible):
    states = []
    try:
        state_set = accessible.get_state_set()
        for state in dir(Atspi.StateType):
            if not state.startswith('_'):
                try:
                    state_type = getattr(Atspi.StateType, state)
                    if state_set.contains(state_type):
                        states.append(state.lower().replace('_', ' '))
                except:
                    pass
    except:
        pass
    return states

def get_actions(accessible):
    actions = []
    try:
        action = accessible.get_action_iface()
        if action:
            n_actions = action.get_n_actions()
            for i in range(n_actions):
                name = action.get_action_name(i)
                if name:
                    actions.append(name)
    except:
        pass
    return actions

def build_element_node(accessible, app_name, path, depth):
    if depth > max_depth:
        return None

    try:
        role = accessible.get_role_name() or ''
        name = accessible.get_name() or ''
        description = accessible.get_description() or ''

        x, y, width, height = 0, 0, 0, 0
        try:
            component = accessible.get_component_iface()
            if component:
                rect = component.get_extents(Atspi.CoordType.SCREEN)
                x, y, width, height = rect.x, rect.y, rect.width, rect.height
        except:
            pass

        element = {{
            'id': path,
            'application': app_name,
            'role': role,
            'name': name,
            'description': description,
            'x': x,
            'y': y,
            'width': width,
            'height': height,
            'states': get_states(accessible),
            'actions': get_actions(accessible)
        }}

        children = []
        try:
            n_children = accessible.get_child_count()
            for i in range(n_children):
                child = accessible.get_child_at_index(i)
                if child:
                    child_path = f"{{path}}/{{i}}"
                    child_node = build_element_node(child, app_name, child_path, depth + 1)
                    if child_node:
                        children.append(child_node)
        except:
            pass

        return {{
            'element': element,
            'children': children
        }}
    except:
        return None

def navigate_to_element(element_id):
    if not element_id:
        return None, None, None

    parts = element_id.strip('/').split('/')
    if not parts:
        return None, None, None

    try:
        desktop = Atspi.get_desktop(0)
        app_idx = int(parts[0])
        current = desktop.get_child_at_index(app_idx)
        app_name = current.get_name() or f'app_{{app_idx}}'

        for part in parts[1:]:
            idx = int(part)
            current = current.get_child_at_index(idx)
            if not current:
                return None, None, None

        return current, app_name, element_id
    except:
        return None, None, None

try:
    if element_id:
        accessible, app_name, path = navigate_to_element(element_id)
        if accessible:
            tree = build_element_node(accessible, app_name, path, 0)
            print(json.dumps(tree if tree else {{"error": "Failed to build tree"}}))
        else:
            print(json.dumps({{"error": f"Element not found: {{element_id}}"}}))
    else:
        desktop = Atspi.get_desktop(0)
        if desktop:
            apps = []
            n_apps = desktop.get_child_count()
            for app_idx in range(n_apps):
                try:
                    app = desktop.get_child_at_index(app_idx)
                    if app:
                        app_name = app.get_name() or f'app_{{app_idx}}'
                        path = f"/{{app_idx}}"
                        app_tree = build_element_node(app, app_name, path, 0)
                        if app_tree:
                            apps.append(app_tree)
                except:
                    pass
            print(json.dumps({{'applications': apps}}))
        else:
            print(json.dumps({{"error": "Desktop not available"}}))
except Exception as e:
    print(json.dumps({{"error": str(e)}}))
"#,
            element_id = element_id_str,
            max_depth = max_depth
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atspi_element_serialization() {
        let element = ATSPIElement {
            id: "/0/1/2".to_string(),
            application: "TestApp".to_string(),
            role: "push button".to_string(),
            name: "OK".to_string(),
            description: "Confirm action".to_string(),
            x: 100,
            y: 200,
            width: 80,
            height: 30,
            states: vec!["enabled".to_string(), "visible".to_string()],
            actions: vec!["click".to_string(), "press".to_string()],
        };

        let json = serde_json::to_string(&element).unwrap();
        assert!(json.contains("push button"));
        assert!(json.contains("TestApp"));

        let parsed: ATSPIElement = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "OK");
        assert_eq!(parsed.role, "push button");
    }

    #[test]
    fn test_query_filter_builder() {
        let filter = ATSPIQueryFilter::new()
            .with_application("Firefox")
            .with_role("push button")
            .with_name("Submit")
            .with_max_depth(5);

        assert_eq!(filter.application, Some("Firefox".to_string()));
        assert_eq!(filter.role, Some("push button".to_string()));
        assert_eq!(filter.name, Some("Submit".to_string()));
        assert_eq!(filter.max_depth, Some(5));
    }

    #[test]
    fn test_query_filter_default() {
        let filter = ATSPIQueryFilter::default();
        assert!(filter.application.is_none());
        assert!(filter.role.is_none());
        assert!(filter.name.is_none());
        assert!(filter.max_depth.is_none());
    }

    #[test]
    fn test_atspi_client_default() {
        let client = ATSPIClient::new();
        assert_eq!(client.display, ":1");
    }

    #[test]
    fn test_atspi_client_with_display() {
        let client = ATSPIClient::with_display(":0");
        assert_eq!(client.display, ":0");
    }

    #[test]
    fn test_query_script_generation() {
        let client = ATSPIClient::new();
        let filter = ATSPIQueryFilter::new()
            .with_application("Firefox")
            .with_role("button");

        let script = client.build_query_script(&filter);

        assert!(script.contains("gi.require_version('Atspi', '2.0')"));
        assert!(script.contains("from gi.repository import Atspi"));
        assert!(script.contains("app_filter = 'Firefox'"));
        assert!(script.contains("role_filter = 'button'"));
        assert!(script.contains("Atspi.get_desktop(0)"));
    }

    #[test]
    fn test_tree_script_generation() {
        let client = ATSPIClient::new();

        let script = client.build_tree_script(Some("/0/1"), Some(3));
        assert!(script.contains("element_id = '/0/1'"));
        assert!(script.contains("max_depth = 3"));

        let script_no_element = client.build_tree_script(None, None);
        assert!(script_no_element.contains("element_id = None"));
        assert!(script_no_element.contains("max_depth = 5"));
    }

    #[test]
    fn test_atspi_tree_node_serialization() {
        let node = ATSPITreeNode {
            element: ATSPIElement {
                id: "/0".to_string(),
                application: "App".to_string(),
                role: "frame".to_string(),
                name: "Window".to_string(),
                description: "".to_string(),
                x: 0,
                y: 0,
                width: 800,
                height: 600,
                states: vec!["visible".to_string()],
                actions: vec![],
            },
            children: vec![],
        };

        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("frame"));
        assert!(json.contains("children"));
    }

    /// Integration test that requires a running AT-SPI registry.
    /// Run with: cargo test -- --ignored
    #[test]
    #[ignore]
    fn test_query_elements_integration() {
        let client = ATSPIClient::new();
        let filter = ATSPIQueryFilter::new().with_max_depth(3);

        match client.query_elements(&filter) {
            Ok(elements) => {
                println!("Found {} elements", elements.len());
                for element in elements.iter().take(5) {
                    println!(
                        "  {} ({}) - {}",
                        element.name, element.role, element.application
                    );
                }
            }
            Err(e) => {
                eprintln!("Query failed (expected if no AT-SPI): {}", e);
            }
        }
    }

    /// Integration test for getting element tree.
    #[test]
    #[ignore]
    fn test_get_element_tree_integration() {
        let client = ATSPIClient::new();

        match client.get_element_tree(None, Some(2)) {
            Ok(tree) => {
                let pretty = serde_json::to_string_pretty(&tree).unwrap();
                println!("Element tree:\n{}", pretty);
            }
            Err(e) => {
                eprintln!("Tree query failed (expected if no AT-SPI): {}", e);
            }
        }
    }
}
