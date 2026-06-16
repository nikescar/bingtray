//! XEmbed-based tray backend using x11rb

use anyhow::Result;
use image::RgbaImage;
use x11rb::protocol::xproto::{self, *};
use x11rb::protocol::Event;
use x11rb::protocol::shape;
use x11rb::rust_connection::RustConnection;
use x11rb::connection::Connection;
use x11rb::wrapper::ConnectionExt as _;

use super::{TrayBackend, TrayExitAction};
use super::logic::TrayLogic;

const SYSTEM_TRAY_REQUEST_DOCK: u32 = 0;

use std::sync::{Arc, Mutex};
use std::process::Child;

pub struct XEmbedTrayBackend {
    logic: TrayLogic,
    gui_process: Arc<Mutex<Option<Child>>>,
}

impl TrayBackend for XEmbedTrayBackend {
    fn new(logic: TrayLogic) -> Result<Self> {
        // Verify X11 connection available
        let (conn, screen_num) = RustConnection::connect(None)
            .map_err(|e| anyhow::anyhow!("X11 connection failed: {}", e))?;

        log::info!("XEmbed: Connected to X11, screen number: {}", screen_num);

        // Verify system tray manager exists
        let atoms = Atoms::new(&conn, screen_num)?;
        let selection_name = format!("_NET_SYSTEM_TRAY_S{}", screen_num);
        log::info!("XEmbed: Looking for tray selection: {}", selection_name);

        let tray_manager = conn
            .get_selection_owner(atoms.tray_selection)?
            .reply()?
            .owner;

        log::info!("XEmbed: Selection owner window ID: {} (0 means not found)", tray_manager);

        if tray_manager == x11rb::NONE {
            return Err(anyhow::anyhow!(
                "No system tray manager found.\n\
                 The selection {} has no owner.\n\
                 \n\
                 JWM tray may not support the freedesktop.org System Tray Protocol.\n\
                 Install: i3bar, polybar, tint2, or other tray-enabled panel",
                selection_name
            ));
        }

        Ok(Self {
            logic,
            gui_process: Arc::new(Mutex::new(None)),
        })
    }

    fn run(self) -> Result<TrayExitAction> {
        log::info!("=== Starting XEmbed tray backend ===");

        let (conn, screen_num) = RustConnection::connect(None)?;
        log::info!("Connected to X11, screen number: {}", screen_num);
        let screen = &conn.setup().roots[screen_num];

        // Get atoms
        let atoms = Atoms::new(&conn, screen_num)?;
        log::info!("Interned atoms - looking for tray on _NET_SYSTEM_TRAY_S{}", screen_num);

        // Find tray manager
        let tray_manager = conn.get_selection_owner(atoms.tray_selection)?.reply()?.owner;
        log::info!("Tray selection owner: {} (NONE={})", tray_manager, x11rb::NONE);
        if tray_manager == x11rb::NONE {
            return Err(anyhow::anyhow!(
                "No system tray manager found.\n\
                 Install: i3bar, polybar, tint2, or other tray-enabled panel"
            ));
        }

        // Create icon window with white background (simple, visible)
        let icon_window = conn.generate_id()?;
        conn.create_window(
            x11rb::COPY_FROM_PARENT as u8,
            icon_window,
            screen.root,
            0,
            0,
            24,
            24,
            0,
            WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &CreateWindowAux::new()
                .background_pixel(screen.white_pixel) // White background for now
                .event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS),
        )?;

        // Set _XEMBED_INFO property
        let xembed_info = [0u32, 1u32]; // version=0, flags=XEMBED_MAPPED
        conn.change_property32(
            PropMode::REPLACE,
            icon_window,
            atoms.xembed_info,
            AtomEnum::CARDINAL,
            &xembed_info,
        )?;

        // Map the window to make it visible
        conn.map_window(icon_window)?;
        conn.flush()?;
        log::info!("Window mapped: {}", icon_window);

        // Send dock request
        send_dock_request(&conn, tray_manager, icon_window, &atoms)?;
        conn.flush()?;

        log::info!("XEmbed icon window created and docked: {}", icon_window);

        // Enter event loop
        self.event_loop(conn, icon_window, screen_num)
    }
}

impl XEmbedTrayBackend {
    fn event_loop(
        self,
        conn: RustConnection,
        icon_window: Window,
        screen_num: usize,
    ) -> Result<TrayExitAction> {
        let screen = &conn.setup().roots[screen_num];
        let gui_process = self.gui_process.clone();

        loop {
            // Check for quit signal file
            let quit_signal = std::env::temp_dir().join("bingtray_quit_signal");
            if quit_signal.exists() {
                log::info!("Quit signal detected, exiting tray");
                let _ = std::fs::remove_file(&quit_signal);
                return Ok(TrayExitAction::Quit);
            }

            // Poll for events (non-blocking)
            let event = match conn.poll_for_event()? {
                Some(event) => event,
                None => {
                    // No event, sleep briefly and continue
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
            };

            match event {
                Event::Expose(e) if e.window == icon_window => {
                    render_icon(&conn, icon_window, screen)?;
                }
                Event::ButtonPress(e) if e.event == icon_window => {
                    match e.detail {
                        1 | 3 => {
                            // Left or right click - open/focus GUI
                            log::info!("Click on tray icon");

                            let mut process_guard = gui_process.lock().unwrap();

                            // Check if GUI is already running
                            let is_running = if let Some(ref mut child) = *process_guard {
                                match child.try_wait() {
                                    Ok(Some(_)) => {
                                        // Process exited
                                        log::info!("GUI process exited");
                                        false
                                    }
                                    Ok(None) => {
                                        // Process still running
                                        log::info!("GUI already running, attempting to focus");
                                        true
                                    }
                                    Err(e) => {
                                        log::error!("Error checking GUI process: {}", e);
                                        false
                                    }
                                }
                            } else {
                                false
                            };

                            if is_running {
                                // Try to focus the existing window using wmctrl or xdotool
                                // For now, just log - window should auto-focus when activated
                                let _ = std::process::Command::new("wmctrl")
                                    .args(&["-a", "BingTray"])
                                    .spawn();
                            } else {
                                // Spawn new GUI process
                                match std::process::Command::new(std::env::current_exe()?)
                                    .arg("--gui")
                                    .spawn()
                                {
                                    Ok(child) => {
                                        log::info!("Spawned GUI process");
                                        *process_guard = Some(child);
                                    }
                                    Err(e) => {
                                        log::error!("Failed to spawn GUI: {}", e);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

pub struct Atoms {
    pub tray_selection: Atom,
    pub tray_opcode: Atom,
    pub xembed_info: Atom,
}

impl Atoms {
    pub fn new(conn: &RustConnection, screen_num: usize) -> Result<Self> {
        let tray_selection = conn
            .intern_atom(false, format!("_NET_SYSTEM_TRAY_S{}", screen_num).as_bytes())?
            .reply()?
            .atom;

        let tray_opcode = conn
            .intern_atom(false, b"_NET_SYSTEM_TRAY_OPCODE")?
            .reply()?
            .atom;

        let xembed_info = conn
            .intern_atom(false, b"_XEMBED_INFO")?
            .reply()?
            .atom;

        Ok(Self {
            tray_selection,
            tray_opcode,
            xembed_info,
        })
    }
}

fn send_dock_request(
    conn: &RustConnection,
    tray_manager: Window,
    icon_window: Window,
    atoms: &Atoms,
) -> Result<()> {
    let event = ClientMessageEvent {
        response_type: CLIENT_MESSAGE_EVENT,
        format: 32,
        sequence: 0,
        window: tray_manager,
        type_: atoms.tray_opcode,
        data: ClientMessageData::from([
            x11rb::CURRENT_TIME,
            SYSTEM_TRAY_REQUEST_DOCK,
            icon_window,
            0,
            0,
        ]),
    };

    conn.send_event(false, tray_manager, EventMask::NO_EVENT, event)?;
    Ok(())
}

fn render_icon(
    conn: &RustConnection,
    window: Window,
    screen: &Screen,
) -> Result<()> {
    log::info!("render_icon called for window {}", window);

    // Load embedded PNG icon
    let icon_bytes = include_bytes!("../../resources/logo.png");
    let image = image::load_from_memory(icon_bytes)?;
    let rgba = image.to_rgba8();

    // Resize to 24x24
    let rgba = image::imageops::resize(&rgba, 24, 24, image::imageops::FilterType::Lanczos3);
    let (width, height) = (rgba.width() as u16, rgba.height() as u16);

    // Convert RGBA to BGRA format for X11
    let bgra_pixels: Vec<u8> = rgba.pixels()
        .flat_map(|p| {
            let [r, g, b, a] = p.0;
            [b, g, r, a] // RGBA -> BGRA
        })
        .collect();

    log::debug!("Rendering PNG icon: {}x{}, {} bytes", width, height, bgra_pixels.len());

    // Create GC for drawing
    let gc = conn.generate_id()?;
    conn.create_gc(gc, window, &CreateGCAux::new())?;

    // Draw image directly to window
    conn.put_image(
        ImageFormat::Z_PIXMAP,
        window,
        gc,
        width,
        height,
        0, 0, 0,
        screen.root_depth,
        &bgra_pixels,
    )?;

    conn.free_gc(gc)?;
    conn.flush()?;

    log::info!("PNG icon rendered successfully");
    Ok(())
}

/// Convert RGBA image to X11 BGR format (24-bit, no alpha)
pub fn rgba_to_x11_format(rgba: &RgbaImage) -> Vec<u8> {
    rgba.pixels()
        .flat_map(|p| [p[2], p[1], p[0]]) // RGBA -> BGR (drop alpha)
        .collect()
}
