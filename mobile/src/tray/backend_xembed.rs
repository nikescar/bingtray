//! XEmbed-based tray backend using x11rb

use anyhow::Result;
use image::RgbaImage;
use x11rb::protocol::xproto::Atom;
use x11rb::rust_connection::RustConnection;

use super::{TrayBackend, TrayExitAction};
use super::logic::TrayLogic;

pub struct XEmbedTrayBackend {
    logic: TrayLogic,
}

impl TrayBackend for XEmbedTrayBackend {
    fn new(logic: TrayLogic) -> Result<Self> {
        // Will implement verification later
        Ok(Self { logic })
    }

    fn run(self) -> Result<TrayExitAction> {
        todo!("Implement XEmbed event loop")
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

/// Convert RGBA image to X11 BGRA format
pub fn rgba_to_x11_format(rgba: &RgbaImage) -> Vec<u8> {
    rgba.pixels()
        .flat_map(|p| [p[2], p[1], p[0], p[3]]) // RGBA -> BGRA
        .collect()
}
