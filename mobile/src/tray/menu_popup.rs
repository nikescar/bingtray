//! X11 popup menu for XEmbed tray

use anyhow::Result;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;
use x11rb::connection::Connection;
use egui_i18n::tr;
use super::logic::TrayLogic;
use super::TrayExitAction;

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub fn contains(&self, x: i16, y: i16) -> bool {
        x >= self.x
            && x < self.x + self.width as i16
            && y >= self.y
            && y < self.y + self.height as i16
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuAction {
    ShowApp,
    CacheDir,
    NextMarket,
    KeepCurrent,
    BlacklistCurrent,
    RandomFavorite,
    Quit,
    Separator,
}

#[derive(Debug)]
pub struct MenuItem {
    pub id: MenuAction,
    pub label: String,
    pub enabled: bool,
    pub bounds: Rect,
}

impl MenuItem {
    pub fn new(id: MenuAction, label: impl Into<String>, enabled: bool) -> Self {
        Self {
            id,
            label: label.into(),
            enabled,
            bounds: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        }
    }

    pub fn separator() -> Self {
        Self {
            id: MenuAction::Separator,
            label: String::new(),
            enabled: false,
            bounds: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        }
    }

    pub fn is_separator(&self) -> bool {
        self.id == MenuAction::Separator
    }
}

pub struct MenuPopup {
    pub window: Window,
    items: Vec<MenuItem>,
}

impl MenuPopup {
    pub fn new(
        conn: &RustConnection,
        screen: &Screen,
        x: i16,
        y: i16,
        logic: &mut TrayLogic,
    ) -> Result<Self> {
        // Build menu items
        let items = build_menu_items(logic);

        // Calculate size
        let (width, height) = calculate_menu_size(&items);

        // Create popup window
        let window = conn.generate_id()?;
        conn.create_window(
            x11rb::COPY_FROM_PARENT as u8,
            window,
            screen.root,
            x,
            y,
            width,
            height,
            1, // 1px border
            WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &CreateWindowAux::new()
                .background_pixel(0xFFFFFF) // White
                .border_pixel(0x000000)     // Black
                .override_redirect(1)        // No WM decoration
                .event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS),
        )?;

        conn.map_window(window)?;
        conn.flush()?;

        Ok(Self { window, items })
    }

    pub fn render(&mut self, conn: &RustConnection) -> Result<()> {
        let gc = conn.generate_id()?;
        conn.create_gc(gc, self.window, &CreateGCAux::new())?;

        let mut y: i16 = 5;

        for item in &mut self.items {
            if item.is_separator() {
                // Draw horizontal line
                conn.poly_line(
                    CoordMode::ORIGIN,
                    self.window,
                    gc,
                    &[
                        Point { x: 5, y },
                        Point { x: 195, y },
                    ],
                )?;
                y += 10;
            } else {
                // Set text color
                let color = if item.enabled { 0x000000 } else { 0x808080 };
                conn.change_gc(gc, &ChangeGCAux::new().foreground(color))?;

                // Draw text
                conn.image_text8(self.window, gc, 10, y + 15, item.label.as_bytes())?;

                // Update bounds for click detection
                item.bounds = Rect {
                    x: 0,
                    y,
                    width: 200,
                    height: 25,
                };

                y += 25;
            }
        }

        conn.flush()?;
        Ok(())
    }

    pub fn handle_click(
        &mut self,
        x: i16,
        y: i16,
        logic: &mut TrayLogic,
    ) -> Result<Option<TrayExitAction>> {
        for item in &self.items {
            if !item.enabled || item.is_separator() {
                continue;
            }

            if item.bounds.contains(x, y) {
                return self.execute_action(item.id, logic);
            }
        }

        Ok(None)
    }

    fn execute_action(
        &mut self,
        action: MenuAction,
        logic: &mut TrayLogic,
    ) -> Result<Option<TrayExitAction>> {
        match action {
            MenuAction::ShowApp => {
                log::info!("Show App clicked");
                Ok(Some(TrayExitAction::OpenGui))
            }
            MenuAction::Quit => {
                log::info!("Quit clicked");
                Ok(Some(TrayExitAction::Quit))
            }
            MenuAction::CacheDir => {
                log::info!("Cache directory clicked");
                logic.open_cache_directory()?;
                Ok(None)
            }
            MenuAction::NextMarket => {
                log::info!("Next market clicked");
                logic.set_next_market_wallpaper()?;
                Ok(None)
            }
            MenuAction::KeepCurrent => {
                log::info!("Keep current clicked");
                logic.keep_current_image()?;
                Ok(None)
            }
            MenuAction::BlacklistCurrent => {
                log::info!("Blacklist current clicked");
                logic.blacklist_current_image()?;
                Ok(None)
            }
            MenuAction::RandomFavorite => {
                log::info!("Random favorite clicked");
                logic.set_kept_wallpaper()?;
                Ok(None)
            }
            MenuAction::Separator => Ok(None),
        }
    }
}

pub fn calculate_menu_size(items: &[MenuItem]) -> (u16, u16) {
    const ITEM_HEIGHT: u16 = 25;
    const SEPARATOR_HEIGHT: u16 = 10;
    const MIN_WIDTH: u16 = 200;
    const PADDING: u16 = 10;

    let mut height = 5; // Top padding

    for item in items {
        if item.is_separator() {
            height += SEPARATOR_HEIGHT;
        } else {
            height += ITEM_HEIGHT;
        }
    }

    height += 5; // Bottom padding

    // Calculate width based on longest label
    let max_label_width = items
        .iter()
        .filter(|item| !item.is_separator())
        .map(|item| item.label.len() as u16 * 7) // ~7 pixels per char
        .max()
        .unwrap_or(MIN_WIDTH);

    let width = max_label_width.max(MIN_WIDTH) + PADDING * 2;

    (width, height)
}

fn build_menu_items(logic: &mut TrayLogic) -> Vec<MenuItem> {
    let current_title = logic.get_current_image_title();

    vec![
        MenuItem::new(MenuAction::ShowApp, tr!("tray-show-app").to_string(), true),
        MenuItem::separator(),
        MenuItem::new(MenuAction::CacheDir, tr!("tray-cache-dir").to_string(), true),
        MenuItem::new(
            MenuAction::NextMarket,
            format!("{}\n{}", tr!("tray-next-market"), logic.get_wallpaper_page_status()),
            logic.has_next_available(),
        ),
        MenuItem::new(
            MenuAction::KeepCurrent,
            if logic.can_keep() {
                format!("{}", tr!("tray-keep-with-title", { title: current_title.clone() }))
            } else {
                format!("{}", tr!("tray-keep-current"))
            },
            logic.can_keep(),
        ),
        MenuItem::new(
            MenuAction::BlacklistCurrent,
            if logic.can_blacklist() {
                format!("{}", tr!("tray-blacklist-with-title", { title: current_title.clone() }))
            } else {
                format!("{}", tr!("tray-blacklist-current"))
            },
            logic.can_blacklist(),
        ),
        MenuItem::new(
            MenuAction::RandomFavorite,
            tr!("tray-random-favorite").to_string(),
            logic.has_kept_wallpapers(),
        ),
        MenuItem::separator(),
        MenuItem::new(MenuAction::Quit, tr!("tray-quit").to_string(), true),
    ]
}
