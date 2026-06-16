//! System tray backend abstraction and public API

use anyhow::Result;

pub mod logic;

#[cfg(target_os = "linux")]
pub mod backend_gtk;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod backend_xembed;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod menu_popup;

#[cfg(target_os = "linux")]
use std::sync::{Arc, OnceLock};

#[cfg(target_os = "linux")]
use crossbeam_queue::SegQueue;

#[cfg(target_os = "linux")]
use tray_icon::{TrayIconEvent, menu::MenuEvent};

#[cfg(target_os = "linux")]
use backend_gtk::GtkTrayBackend;

/// Global queue for tray icon events (Linux only - used by GTK backend)
#[cfg(target_os = "linux")]
pub(crate) static TRAY_ICON_EVENTS: OnceLock<Arc<SegQueue<TrayIconEvent>>> = OnceLock::new();

/// Global queue for menu events (Linux only - used by GTK backend)
#[cfg(target_os = "linux")]
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
/// Linux only - used by GTK backend
#[cfg(target_os = "linux")]
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
        use backend_xembed::XEmbedTrayBackend;

        // On Linux: Try GTK first, fall back to XEmbed
        match GtkTrayBackend::new(logic.clone()) {
            Ok(backend) => {
                log::info!("Using GTK tray backend");
                backend.run()
            }
            Err(e) => {
                log::warn!("GTK tray unavailable: {}, falling back to XEmbed", e);
                match XEmbedTrayBackend::new(logic) {
                    Ok(backend) => {
                        log::info!("Using XEmbed tray backend");
                        backend.run()
                    }
                    Err(xembed_err) => {
                        Err(anyhow::anyhow!(
                            "No tray backend available:\n\
                             - GTK: {}\n\
                             - XEmbed: {}\n\
                             \n\
                             Try: Install libayatana-appindicator or ensure X11 is running",
                            e,
                            xembed_err
                        ))
                    }
                }
            }
        }
    }

    #[cfg(all(unix, not(target_os = "linux"), not(target_os = "macos")))]
    {
        use backend_xembed::XEmbedTrayBackend;

        // On non-Linux Unix (BSDs, etc.): Use XEmbed only
        match XEmbedTrayBackend::new(logic) {
            Ok(backend) => {
                log::info!("Using XEmbed tray backend");
                backend.run()
            }
            Err(e) => {
                Err(anyhow::anyhow!(
                    "XEmbed tray backend failed: {}\n\
                     \n\
                     Try: Ensure X11 is running",
                    e
                ))
            }
        }
    }

    #[cfg(any(not(unix), target_os = "macos"))]
    {
        Err(anyhow::anyhow!("Tray not implemented for this platform"))
    }
}
