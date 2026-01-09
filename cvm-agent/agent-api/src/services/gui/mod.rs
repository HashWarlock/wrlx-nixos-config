//! GUI automation services for the CVM Agent.
//!
//! This module provides services for GUI automation including:
//! - Screenshot capture
//! - Accessibility tree inspection (AT-SPI)
//! - Input automation (xdotool)
//! - Vision-based element detection

pub mod screenshot;
pub mod atspi;
pub mod xdotool;
pub mod vision;

mod service;

pub use screenshot::ScreenshotCapture;
pub use atspi::{ATSPIClient, ATSPIElement, ATSPIQueryFilter, ATSPITreeNode};
pub use service::GUIServiceImpl;
