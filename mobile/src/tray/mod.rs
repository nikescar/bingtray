//! System tray backend abstraction and public API

use anyhow::Result;
use std::sync::{Arc, OnceLock};
use crossbeam_queue::SegQueue;
use tray_icon::{TrayIconEvent, menu::MenuEvent};

pub mod logic;

#[cfg(target_os = "linux")]
pub mod backend_gtk;

#[cfg(target_os = "linux")]
pub mod backend_xembed;

#[cfg(target_os = "linux")]
pub mod menu_popup;

#[cfg(target_os = "linux")]
use backend_gtk::GtkTrayBackend;

/// Global queue for tray icon events
pub(crate) static TRAY_ICON_EVENTS: OnceLock<Arc<SegQueue<TrayIconEvent>>> = OnceLock::new();

/// Global queue for menu events
pub(crate) static MENU_EVENTS: OnceLock<Arc<SegQueue<MenuEvent>>> = OnceLock::new();

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

/// Initialize global event handlers (call once at startup)
pub fn init_tray_event_handlers() {
    log::info!("Initializing global tray event handlers");

    TRAY_ICON_EVENTS.get_or_init(|| Arc::new(SegQueue::new()));
    MENU_EVENTS.get_or_init(|| Arc::new(SegQueue::new()));

    TrayIconEvent::set_event_handler(Some(|event: TrayIconEvent| {
        if let Some(queue) = TRAY_ICON_EVENTS.get() {
            queue.push(event);
        }
    }));

    MenuEvent::set_event_handler(Some(|event: MenuEvent| {
        if let Some(queue) = MENU_EVENTS.get() {
            queue.push(event);
        }
    }));
}

/// Run the system tray mode
pub fn run_tray_mode() -> Result<TrayExitAction> {
    let logic = logic::TrayLogic::new()?;

    #[cfg(target_os = "linux")]
    {
        match GtkTrayBackend::new(logic.clone()) {
            Ok(backend) => {
                log::info!("Using GTK tray backend");
                backend.run()
            }
            Err(e) => {
                log::warn!("GTK tray unavailable: {}, falling back to XEmbed", e);
                // XEmbed fallback will be added later
                Err(anyhow::anyhow!("XEmbed not yet implemented"))
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        Err(anyhow::anyhow!("Tray not implemented for this platform"))
    }
}
