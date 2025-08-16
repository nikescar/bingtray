use bingtray_core::gui::bingtray_app::{WallpaperSetter, ScreenSizeProvider};
use crate::android_wallpaper::set_wallpaper_from_bytes;
use crate::android_screensize::get_screen_size;

pub struct AndroidBingtrayService;

// AndroidBingtrayService is stateless, so it's safe to implement Send + Sync
unsafe impl Send for AndroidBingtrayService {}
unsafe impl Sync for AndroidBingtrayService {}

impl WallpaperSetter for AndroidBingtrayService {
    fn set_wallpaper_from_bytes(&self, image_bytes: &[u8]) -> std::io::Result<bool> {
        set_wallpaper_from_bytes(image_bytes)
    }
}

impl ScreenSizeProvider for AndroidBingtrayService {
    fn get_screen_size(&self) -> std::io::Result<(i32, i32)> {
        get_screen_size()
    }
}