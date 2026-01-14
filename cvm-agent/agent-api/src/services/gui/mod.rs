//! GUI automation services for the CVM Agent.
//!
//! This module provides services for GUI automation including:
//! - Screenshot capture
//! - Accessibility tree inspection (AT-SPI)
//! - Input automation (xdotool)
//! - Vision-based element detection

mod screenshot;
mod atspi;
mod xdotool;
mod vision;
mod service;

// Only export what's actually used externally
pub use service::GUIServiceImpl;
