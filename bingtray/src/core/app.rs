// cli, gui should depend on this module
// Core application state and traits for Bingtray

use anyhow::Result;
use log::{info, warn, error};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use poll_promise::Promise;

use crate::{BingImage, Config, load_market_codes, get_old_market_codes, load_historical_metadata, get_image_metadata};

pub trait WallpaperSetter: Send + Sync {
    fn set_wallpaper_from_bytes(&self, image_bytes: &[u8]) -> std::io::Result<bool>;
}

pub trait ScreenSizeProvider: Send + Sync {
    fn get_screen_size(&self) -> std::io::Result<(i32, i32)>;
}

#[derive(Clone)]
pub struct CarouselImage {
    pub title: String,
    pub copyright: String,
    pub copyright_link: String,
    pub thumbnail_url: String,
    pub full_url: String,
    pub image_bytes: Option<Vec<u8>>,
}

pub struct Gui {
    // UI state
    is_dark_theme: bool,
    window_title: String,
    
    // Material3 components state
    switch_state: bool,
    slider_value: f32,
    checkbox_state: bool,
    
    // Application data
    wallpaper_path: Option<String>,

    // conf, sqlite instance
    conf: Conf,

}

impl App {
    pub fn new() -> Result<Self> {
        Self {
            is_dark_theme: false,
            window_title: "BingTray".to_string(),
            switch_state: false,
            slider_value: 0.5,
            checkbox_state: false,
            wallpaper_path: None,

            conf: Conf::new()?,
        };

        // initialize config
        self.conf = Conf::new()?;

        // initialize database
        


        self

    }
    
    
}

