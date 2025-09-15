// export all modules
pub mod gui;
pub mod conf;
pub mod app;
pub mod sqlite;
pub mod httpclient;
pub mod request;
pub mod bingwpclient;

use crate::core::gui::Gui;
use crate::core::conf::Conf;
use crate::core::app::{App, WallpaperSetter, ScreenSizeProvider};
use crate::core::sqlite::Sqlite;

