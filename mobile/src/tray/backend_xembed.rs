//! XEmbed-based tray backend using x11rb

use anyhow::Result;
use image::RgbaImage;

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

/// Convert RGBA image to X11 BGRA format
pub fn rgba_to_x11_format(rgba: &RgbaImage) -> Vec<u8> {
    rgba.pixels()
        .flat_map(|p| [p[2], p[1], p[0], p[3]]) // RGBA -> BGRA
        .collect()
}
