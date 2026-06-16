//! XEmbed-based tray backend using x11rb

use anyhow::Result;
use image::RgbaImage;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;
use x11rb::connection::Connection;

use super::{TrayBackend, TrayExitAction};
use super::logic::TrayLogic;

const SYSTEM_TRAY_REQUEST_DOCK: u32 = 0;

pub struct XEmbedTrayBackend {
    logic: TrayLogic,
}

impl TrayBackend for XEmbedTrayBackend {
    fn new(logic: TrayLogic) -> Result<Self> {
        // Verify X11 connection available
        let (conn, screen_num) = RustConnection::connect(None)
            .map_err(|e| anyhow::anyhow!("X11 connection failed: {}", e))?;

        // Verify system tray manager exists
        let atoms = Atoms::new(&conn, screen_num)?;
        let tray_manager = conn
            .get_selection_owner(atoms.tray_selection)?
            .reply()?
            .owner;

        if tray_manager == x11rb::NONE {
            return Err(anyhow::anyhow!(
                "No system tray manager found.\n\
                 Install: i3bar, polybar, tint2, or other tray-enabled panel"
            ));
        }

        Ok(Self { logic })
    }

    fn run(mut self) -> Result<TrayExitAction> {
        log::info!("=== Starting XEmbed tray backend ===");

        let (conn, screen_num) = RustConnection::connect(None)?;
        let screen = &conn.setup().roots[screen_num];

        // Get atoms
        let atoms = Atoms::new(&conn, screen_num)?;

        // Find tray manager
        let tray_manager = conn.get_selection_owner(atoms.tray_selection)?.reply()?.owner;
        if tray_manager == x11rb::NONE {
            return Err(anyhow::anyhow!("No system tray manager found"));
        }

        // Create icon window
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
                .background_pixel(screen.black_pixel)
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

        // Send dock request
        send_dock_request(&conn, tray_manager, icon_window, &atoms)?;
        conn.flush()?;

        log::info!("XEmbed icon window created: {}", icon_window);

        // Enter event loop
        self.event_loop(conn, icon_window, screen)
    }
}

impl XEmbedTrayBackend {
    fn event_loop(
        mut self,
        conn: RustConnection,
        icon_window: Window,
        screen: &Screen,
    ) -> Result<TrayExitAction> {
        use super::menu_popup::MenuPopup;

        let mut menu_popup: Option<MenuPopup> = None;

        loop {
            let event = conn.wait_for_event()
                .map_err(|e| {
                    log::error!("X11 connection error: {}", e);
                    anyhow::anyhow!("X11 connection lost")
                })?;

            match event {
                Event::Expose(e) if e.window == icon_window => {
                    render_icon(&conn, icon_window, screen)?;
                }
                Event::Expose(e) if menu_popup.as_ref().map(|m| m.window == e.window).unwrap_or(false) => {
                    if let Some(ref mut menu) = menu_popup {
                        menu.render(&conn)?;
                    }
                }
                Event::ButtonPress(e) if e.window == icon_window => {
                    match e.detail {
                        1 => {
                            // Left click - close menu
                            menu_popup = None;
                        }
                        3 => {
                            // Right click - show menu
                            log::info!("Right-click at ({}, {})", e.root_x, e.root_y);
                            menu_popup = Some(MenuPopup::new(
                                &conn,
                                screen,
                                e.root_x,
                                e.root_y,
                                &mut self.logic,
                            )?);
                        }
                        _ => {}
                    }
                }
                Event::ButtonPress(e) if menu_popup.as_ref().map(|m| m.window == e.window).unwrap_or(false) => {
                    // Menu clicked
                    if let Some(ref mut menu) = menu_popup {
                        if let Some(action) = menu.handle_click(e.event_x, e.event_y, &mut self.logic)? {
                            return Ok(action);
                        }
                    }
                    menu_popup = None;
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
    // Load embedded icon
    let icon_bytes = include_bytes!("../../resources/logo.png");
    let image = image::load_from_memory(icon_bytes)?;
    let rgba = image.to_rgba8();

    // Create pixmap
    let pixmap = conn.generate_id()?;
    conn.create_pixmap(screen.root_depth, pixmap, window, 24, 24)?;

    // Create GC
    let gc = conn.generate_id()?;
    conn.create_gc(gc, pixmap, &CreateGCAux::new())?;

    // Convert and draw
    let image_data = rgba_to_x11_format(&rgba);
    conn.put_image(
        ImageFormat::Z_PIXMAP,
        pixmap,
        gc,
        24,
        24,
        0,
        0,
        0,
        screen.root_depth,
        &image_data,
    )?;

    // Copy to window
    conn.copy_area(pixmap, window, gc, 0, 0, 0, 0, 24, 24)?;
    conn.flush()?;

    Ok(())
}

/// Convert RGBA image to X11 BGRA format
pub fn rgba_to_x11_format(rgba: &RgbaImage) -> Vec<u8> {
    rgba.pixels()
        .flat_map(|p| [p[2], p[1], p[0], p[3]]) // RGBA -> BGRA
        .collect()
}
