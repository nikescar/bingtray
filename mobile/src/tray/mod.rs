//! System tray backend abstraction

use anyhow::Result;

pub mod logic;

#[cfg(target_os = "linux")]
pub mod backend_gtk;

#[cfg(target_os = "linux")]
pub mod backend_xembed;

#[cfg(target_os = "linux")]
pub mod menu_popup;

/// Action to take after tray mode exits
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayExitAction {
    Quit,
    OpenGui,
}

/// Trait for tray backend implementations
pub trait TrayBackend {
    fn new(logic: logic::TrayLogic) -> Result<Self> where Self: Sized;
    fn run(self) -> Result<TrayExitAction>;
}
