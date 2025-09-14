// export all modules
pub mod gui;
pub mod conf;
pub mod app;
pub mod sqlite;

use crate::core::gui::Gui;
use crate::core::conf::Conf;
use crate::core::app::{App, WallpaperSetter, ScreenSizeProvider};
use crate::core::sqlite::Sqlite;

