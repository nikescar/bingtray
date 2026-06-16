# XEmbed Tray Icon Fallback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement XEmbed protocol fallback for tray icon when GTK/libayatana unavailable on Linux/BSD systems

**Architecture:** Trait-based abstraction with `TrayBackend` trait, shared `TrayLogic` between GTK and XEmbed backends, runtime detection tries GTK first then falls back to XEmbed

**Tech Stack:** Rust, x11rb, tray-icon (GTK), diesel, egui-i18n

---

## File Structure Overview

**New files:**
- `mobile/src/tray/mod.rs` - TrayBackend trait and backend selection
- `mobile/src/tray/logic.rs` - Extracted TrayLogic (shared)
- `mobile/src/tray/backend_gtk.rs` - GTK backend wrapper
- `mobile/src/tray/backend_xembed.rs` - XEmbed implementation
- `mobile/src/tray/menu_popup.rs` - X11 popup menu
- `mobile/tests/tray/logic_tests.rs` - TrayLogic unit tests
- `mobile/tests/tray/backend_xembed_tests.rs` - XEmbed tests
- `mobile/tests/tray/menu_popup_tests.rs` - Menu tests

**Modified files:**
- `mobile/src/tray.rs` - Becomes thin public API wrapper
- `mobile/Cargo.toml` - Add x11rb dependency

---

## Phase 1: Refactoring & Structure

### Task 1: Add x11rb Dependency

**Files:**
- Modify: `mobile/Cargo.toml`

- [ ] **Step 1: Add x11rb to Linux dependencies**

Add to `[target.'cfg(target_os = "linux")'.dependencies]` section:

```toml
x11rb = { version = "0.13", features = ["allow-unsafe-code"] }
```

- [ ] **Step 2: Verify dependency resolves**

Run: `cargo check --manifest-path mobile/Cargo.toml`
Expected: Success, x11rb downloads

- [ ] **Step 3: Commit**

```bash
git add mobile/Cargo.toml
git commit -m "build: add x11rb dependency for XEmbed tray fallback"
```

---

### Task 2: Create Tray Module Structure

**Files:**
- Create: `mobile/src/tray/mod.rs`
- Create: `mobile/tests/tray/mod.rs`

- [ ] **Step 1: Create tray module directory**

```bash
mkdir -p mobile/src/tray
mkdir -p mobile/tests/tray
```

- [ ] **Step 2: Create tray/mod.rs with basic structure**

Create `mobile/src/tray/mod.rs`:

```rust
//! System tray backend abstraction

use anyhow::Result;

pub mod logic;

#[cfg(target_os = "linux")]
pub mod backend_gtk;

#[cfg(target_os = "linux")]
pub mod backend_xembed;

#[cfg(target_os = "linux")]
pub mod menu_popup;

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
```

- [ ] **Step 3: Create tests/tray/mod.rs**

Create `mobile/tests/tray/mod.rs`:

```rust
//! Tray module tests

pub mod logic_tests;
pub mod backend_xembed_tests;
pub mod menu_popup_tests;
```

- [ ] **Step 4: Verify module compiles**

Run: `cargo check --manifest-path mobile/Cargo.toml`
Expected: Errors about missing modules (expected, will fix next)

- [ ] **Step 5: Commit**

```bash
git add mobile/src/tray/mod.rs mobile/tests/tray/mod.rs
git commit -m "feat: add tray module structure with TrayBackend trait"
```

---

### Task 3: Extract TrayLogic to Separate Module

**Files:**
- Create: `mobile/src/tray/logic.rs`
- Modify: `mobile/src/tray.rs`

- [ ] **Step 1: Create logic.rs with TrayLogic struct**

Create `mobile/src/tray/logic.rs`:

```rust
//! Tray business logic shared between backends

use anyhow::Result;
use diesel::prelude::*;

pub struct TrayLogic {
    conn: diesel::SqliteConnection,
}

impl TrayLogic {
    pub fn new() -> Result<Self> {
        use diesel::Connection;
        let db_path = crate::db::get_database_path()?;
        let mut conn = diesel::SqliteConnection::establish(&db_path.to_string_lossy())?;

        // Run migrations
        use diesel_migrations::MigrationHarness;
        conn.run_pending_migrations(crate::db::MIGRATIONS)
            .map_err(|e| anyhow::anyhow!("Migration failed: {}", e))?;

        Ok(Self { conn })
    }

    pub fn get_wallpaper_page_status(&mut self) -> String {
        match crate::db::operations::count_by_status(&mut self.conn, crate::db::ImageStatus::Unprocessed) {
            Ok(count) => format!("({} available)", count),
            Err(_) => String::new(),
        }
    }

    pub fn has_next_available(&mut self) -> bool {
        true
    }

    pub fn get_current_image_title(&mut self) -> String {
        use crate::viewmodel::commands::get_current_desktop_wallpaper_url_sync;

        if let Ok(Some(url)) = get_current_desktop_wallpaper_url_sync(&mut self.conn) {
            if let Ok(Some(image)) = crate::db::operations::get_image(&mut self.conn, &url) {
                let title = &image.title;
                if title.len() > 40 {
                    format!("{}...", &title[..40])
                } else {
                    title.clone()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    }

    pub fn can_keep(&mut self) -> bool {
        use crate::viewmodel::commands::get_current_desktop_wallpaper_url_sync;

        if let Ok(Some(url)) = get_current_desktop_wallpaper_url_sync(&mut self.conn) {
            if let Ok(Some(image)) = crate::db::operations::get_image(&mut self.conn, &url) {
                image.status != crate::db::ImageStatus::KeepFavorite.as_str()
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn can_blacklist(&mut self) -> bool {
        use crate::viewmodel::commands::get_current_desktop_wallpaper_url_sync;
        get_current_desktop_wallpaper_url_sync(&mut self.conn).ok().flatten().is_some()
    }

    pub fn has_kept_wallpapers(&mut self) -> bool {
        crate::db::operations::count_by_status(&mut self.conn, crate::db::ImageStatus::KeepFavorite)
            .map(|count| count > 0)
            .unwrap_or(false)
    }

    pub fn open_cache_directory(&self) -> Result<()> {
        let config = crate::Config::new()?;
        let path = &config.cached_dir;

        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(path)
                .spawn()?;
        }

        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(path)
                .spawn()?;
        }

        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(path)
                .spawn()?;
        }

        log::info!("Opened cache directory: {:?}", path);
        Ok(())
    }

    pub fn set_next_market_wallpaper(&mut self) -> Result<bool> {
        use crate::viewmodel::commands::download_and_set_next_wallpaper_sync;

        match download_and_set_next_wallpaper_sync(&mut self.conn) {
            Ok(_result) => Ok(true),
            Err(e) => {
                log::error!("Failed to set next wallpaper: {}", e);
                Err(e)
            }
        }
    }

    pub fn keep_current_image(&mut self) -> Result<()> {
        use crate::viewmodel::commands::keep_current_wallpaper_sync;

        if let Some(_title) = keep_current_wallpaper_sync(&mut self.conn)? {
            log::info!("Kept current image");
            Ok(())
        } else {
            anyhow::bail!("No current wallpaper to keep")
        }
    }

    pub fn blacklist_current_image(&mut self) -> Result<()> {
        use crate::viewmodel::commands::blacklist_current_wallpaper_sync;

        if let Some(_title) = blacklist_current_wallpaper_sync(&mut self.conn)? {
            log::info!("Blacklisted current image");
            Ok(())
        } else {
            anyhow::bail!("No current wallpaper to blacklist")
        }
    }

    pub fn set_kept_wallpaper(&mut self) -> Result<bool> {
        use crate::viewmodel::commands::set_random_favorite_wallpaper_sync;

        match set_random_favorite_wallpaper_sync(&mut self.conn) {
            Ok(Some(_title)) => Ok(true),
            Ok(None) => {
                log::warn!("No favorite wallpapers available");
                Ok(false)
            }
            Err(e) => {
                log::error!("Failed to set random favorite: {}", e);
                Err(e)
            }
        }
    }
}

impl Clone for TrayLogic {
    fn clone(&self) -> Self {
        Self::new().expect("Failed to clone TrayLogic")
    }
}
```

- [ ] **Step 2: Verify logic.rs compiles**

Run: `cargo check --manifest-path mobile/Cargo.toml`
Expected: Success

- [ ] **Step 3: Commit**

```bash
git add mobile/src/tray/logic.rs
git commit -m "feat: extract TrayLogic to separate module"
```

---

### Task 4: Create GTK Backend Wrapper

**Files:**
- Create: `mobile/src/tray/backend_gtk.rs`
- Modify: `mobile/src/tray.rs`

- [ ] **Step 1: Create backend_gtk.rs stub**

Create `mobile/src/tray/backend_gtk.rs`:

```rust
//! GTK-based tray backend using tray-icon crate

use anyhow::Result;
use std::sync::{Arc, Mutex};
use crossbeam_queue::SegQueue;
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, MenuId},
    Icon, TrayIconBuilder, TrayIcon, TrayIconEvent,
};

use super::{TrayBackend, TrayExitAction};
use super::logic::TrayLogic;

pub struct GtkTrayBackend {
    logic: TrayLogic,
}

impl TrayBackend for GtkTrayBackend {
    fn new(logic: TrayLogic) -> Result<Self> {
        // Early GTK availability check
        if !is_gtk_available() {
            return Err(anyhow::anyhow!("GTK tray manager not available"));
        }
        Ok(Self { logic })
    }

    fn run(mut self) -> Result<TrayExitAction> {
        // Will implement in next step
        todo!("Implement GTK backend run()")
    }
}

fn is_gtk_available() -> bool {
    // Quick check: can we create event loop?
    EventLoopBuilder::<UserEvent>::with_user_event()
        .build()
        .is_ok()
}

#[derive(Debug)]
enum UserEvent {
    TrayIconEvent(TrayIconEvent),
    MenuEvent(MenuEvent),
}
```

- [ ] **Step 2: Move GTK implementation from tray.rs to backend_gtk.rs**

Complete the `run()` method in `backend_gtk.rs` by copying the event loop logic from current `tray.rs::run_tray_mode()`:

```rust
impl TrayBackend for GtkTrayBackend {
    fn new(logic: TrayLogic) -> Result<Self> {
        if !is_gtk_available() {
            return Err(anyhow::anyhow!("GTK tray manager not available"));
        }
        Ok(Self { logic })
    }

    fn run(mut self) -> Result<TrayExitAction> {
        log::info!("=== Starting GTK tray backend ===");

        // Create event loop
        let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

        // Get global event queues (from tray.rs init_tray_event_handlers)
        let tray_queue = super::super::TRAY_ICON_EVENTS.get()
            .expect("Tray event handlers not initialized");
        let menu_queue = super::super::MENU_EVENTS.get()
            .expect("Menu event handlers not initialized");

        let exit_action = Arc::new(Mutex::new(TrayExitAction::Quit));
        let exit_action_for_return = exit_action.clone();

        let mut tray_icon: Option<TrayIcon> = None;
        let mut menu_items: Option<MenuItems> = None;

        let tray_queue = tray_queue.clone();
        let menu_queue = menu_queue.clone();

        event_loop.run_return(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            while let Some(_tray_event) = tray_queue.pop() {
                // Handle tray icon events
            }

            while let Some(menu_event) = menu_queue.pop() {
                if let Some(ref items) = menu_items {
                    if menu_event.id == items.show_app {
                        *exit_action_for_return.lock().unwrap() = TrayExitAction::OpenGui;
                        *control_flow = ControlFlow::Exit;
                        continue;
                    } else if menu_event.id == items.cache_dir {
                        let _ = self.logic.open_cache_directory();
                    } else if menu_event.id == items.next_market {
                        if let Ok(true) = self.logic.set_next_market_wallpaper() {
                            if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &mut self.logic, &mut menu_items.as_mut().unwrap());
                            }
                        }
                    } else if menu_event.id == items.keep_current {
                        if self.logic.can_keep() {
                            let _ = self.logic.keep_current_image();
                            if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &mut self.logic, &mut menu_items.as_mut().unwrap());
                            }
                        }
                    } else if menu_event.id == items.blacklist_current {
                        if self.logic.can_blacklist() {
                            let _ = self.logic.blacklist_current_image();
                            if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &mut self.logic, &mut menu_items.as_mut().unwrap());
                            }
                        }
                    } else if menu_event.id == items.random_favorite {
                        if let Ok(true) = self.logic.set_kept_wallpaper() {
                            if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &mut self.logic, &mut menu_items.as_mut().unwrap());
                            }
                        }
                    } else if menu_event.id == items.quit {
                        *control_flow = ControlFlow::Exit;
                    }
                }
            }

            match event {
                Event::NewEvents(_) => {
                    if tray_icon.is_none() {
                        let icon = load_tray_icon();
                        let (menu, items) = create_tray_menu(&mut self.logic);

                        let new_tray_icon = TrayIconBuilder::new()
                            .with_menu(Box::new(menu))
                            .with_tooltip("BingTray")
                            .with_icon(icon)
                            .build()
                            .expect("Failed to build tray icon");

                        tray_icon = Some(new_tray_icon);
                        menu_items = Some(items);
                    }

                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                _ => {}
            }
        });

        Ok(*exit_action.lock().unwrap())
    }
}

struct MenuItems {
    show_app: MenuId,
    cache_dir: MenuId,
    next_market: MenuId,
    current_title: MenuId,
    keep_current: MenuId,
    blacklist_current: MenuId,
    random_favorite: MenuId,
    quit: MenuId,
}

fn load_tray_icon() -> Icon {
    let icon_bytes = include_bytes!("../../resources/logo.png");
    let image = image::load_from_memory(icon_bytes).expect("Failed to load icon");
    let rgba = image.to_rgba8();
    Icon::from_rgba(rgba.to_vec(), image.width(), image.height())
        .expect("Failed to create icon")
}

fn create_tray_menu(logic: &mut TrayLogic) -> (Menu, MenuItems) {
    use egui_i18n::tr;

    let menu = Menu::new();

    let show_app = MenuItem::new(format!("{}", tr!("tray-show-app")), true, None);
    let cache_dir = MenuItem::new(format!("{}", tr!("tray-cache-dir")), true, None);

    let wallpaper_status = logic.get_wallpaper_page_status();
    let has_next = logic.has_next_available();
    let next_market = MenuItem::new(
        format!("{}\n{}", tr!("tray-next-market"), wallpaper_status),
        has_next,
        None
    );

    let current_title_text = logic.get_current_image_title();
    let current_title_display = if !current_title_text.is_empty() {
        format!("📷 {}", current_title_text)
    } else {
        format!("📷 {}", tr!("tray-no-wallpaper"))
    };
    let current_title_item = MenuItem::new(current_title_display, false, None);

    let can_keep = logic.can_keep();
    let keep_text = if can_keep {
        format!("{}", tr!("tray-keep-with-title", { title: current_title_text.clone() }))
    } else {
        format!("{}", tr!("tray-keep-current"))
    };
    let keep_current = MenuItem::new(keep_text, can_keep, None);

    let can_blacklist = logic.can_blacklist();
    let blacklist_text = if can_blacklist {
        format!("{}", tr!("tray-blacklist-with-title", { title: current_title_text.clone() }))
    } else {
        format!("{}", tr!("tray-blacklist-current"))
    };
    let blacklist_current = MenuItem::new(blacklist_text, can_blacklist, None);

    let has_kept = logic.has_kept_wallpapers();
    let random_favorite = MenuItem::new(
        format!("{}", tr!("tray-random-favorite")),
        has_kept,
        None,
    );

    let quit = MenuItem::new(format!("{}", tr!("tray-quit")), true, None);

    let menu_items = MenuItems {
        show_app: show_app.id().clone(),
        cache_dir: cache_dir.id().clone(),
        next_market: next_market.id().clone(),
        current_title: current_title_item.id().clone(),
        keep_current: keep_current.id().clone(),
        blacklist_current: blacklist_current.id().clone(),
        random_favorite: random_favorite.id().clone(),
        quit: quit.id().clone(),
    };

    menu.append(&show_app).ok();
    menu.append(&MenuItem::new("", false, None)).ok();
    menu.append(&cache_dir).ok();
    menu.append(&next_market).ok();
    menu.append(&current_title_item).ok();
    menu.append(&keep_current).ok();
    menu.append(&blacklist_current).ok();
    menu.append(&random_favorite).ok();
    menu.append(&MenuItem::new("", false, None)).ok();
    menu.append(&quit).ok();

    (menu, menu_items)
}

fn update_tray_menu(
    tray_icon: &TrayIcon,
    logic: &mut TrayLogic,
    menu_items: &mut MenuItems,
) {
    let (new_menu, new_menu_items) = create_tray_menu(logic);
    *menu_items = new_menu_items;
    tray_icon.set_menu(Some(Box::new(new_menu)));
}
```

- [ ] **Step 3: Update tray.rs to use backend**

Modify `mobile/src/tray.rs` to delegate to backend:

```rust
//! System tray interface for Bingtray (Desktop only)

use anyhow::Result;
use std::sync::{Arc, OnceLock};
use crossbeam_queue::SegQueue;
use tray_icon::{TrayIconEvent, menu::MenuEvent};

mod tray;
pub use tray::{TrayBackend, TrayExitAction};

#[cfg(target_os = "linux")]
use tray::backend_gtk::GtkTrayBackend;

/// Global queue for tray icon events
pub(crate) static TRAY_ICON_EVENTS: OnceLock<Arc<SegQueue<TrayIconEvent>>> = OnceLock::new();

/// Global queue for menu events
pub(crate) static MENU_EVENTS: OnceLock<Arc<SegQueue<MenuEvent>>> = OnceLock::new();

/// Initialize global event handlers (call once at startup)
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
    let logic = tray::logic::TrayLogic::new()?;

    #[cfg(target_os = "linux")]
    {
        match GtkTrayBackend::new(logic.clone()) {
            Ok(backend) => {
                log::info!("Using GTK tray backend");
                backend.run()
            }
            Err(e) => {
                log::warn!("GTK tray unavailable: {}, falling back to XEmbed", e);
                // XEmbed fallback will be added later
                Err(anyhow::anyhow!("XEmbed not yet implemented"))
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        GtkTrayBackend::new(logic)?.run()
    }
}
```

- [ ] **Step 4: Verify GTK backend compiles**

Run: `cargo check --manifest-path mobile/Cargo.toml`
Expected: Success

- [ ] **Step 5: Commit**

```bash
git add mobile/src/tray/backend_gtk.rs mobile/src/tray.rs
git commit -m "refactor: extract GTK tray to backend_gtk module"
```

---

## Phase 2: XEmbed Core Implementation (TDD)

### Task 5: Test and Implement RGBA to X11 Format Conversion

**Files:**
- Create: `mobile/tests/tray/backend_xembed_tests.rs`
- Create: `mobile/src/tray/backend_xembed.rs`

- [ ] **Step 1: Write failing test for RGBA conversion**

Create `mobile/tests/tray/backend_xembed_tests.rs`:

```rust
//! XEmbed backend tests

#[cfg(target_os = "linux")]
mod xembed_tests {
    use image::{Rgba, RgbaImage};

    #[test]
    fn test_rgba_to_x11_format_converts_bgra() {
        let rgba = RgbaImage::from_pixel(1, 1, Rgba([255, 128, 64, 32]));
        let x11_data = bingtray::tray::backend_xembed::rgba_to_x11_format(&rgba);

        // X11 expects BGRA format
        assert_eq!(x11_data.len(), 4);
        assert_eq!(x11_data[0], 64);  // B
        assert_eq!(x11_data[1], 128); // G
        assert_eq!(x11_data[2], 255); // R
        assert_eq!(x11_data[3], 32);  // A
    }

    #[test]
    fn test_rgba_to_x11_format_multiple_pixels() {
        let mut rgba = RgbaImage::new(2, 1);
        rgba.put_pixel(0, 0, Rgba([255, 0, 0, 255])); // Red
        rgba.put_pixel(1, 0, Rgba([0, 255, 0, 255])); // Green

        let x11_data = bingtray::tray::backend_xembed::rgba_to_x11_format(&rgba);

        assert_eq!(x11_data.len(), 8);
        // First pixel: red -> BGRA
        assert_eq!(x11_data[0], 0);   // B
        assert_eq!(x11_data[1], 0);   // G
        assert_eq!(x11_data[2], 255); // R
        assert_eq!(x11_data[3], 255); // A
        // Second pixel: green -> BGRA
        assert_eq!(x11_data[4], 0);   // B
        assert_eq!(x11_data[5], 255); // G
        assert_eq!(x11_data[6], 0);   // R
        assert_eq!(x11_data[7], 255); // A
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path mobile/Cargo.toml rgba_to_x11`
Expected: FAIL - module `backend_xembed` not found

- [ ] **Step 3: Create backend_xembed.rs stub**

Create `mobile/src/tray/backend_xembed.rs`:

```rust
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
```

- [ ] **Step 4: Make function public in mod.rs**

Update `mobile/src/tray/mod.rs`:

```rust
#[cfg(target_os = "linux")]
pub mod backend_xembed;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --manifest-path mobile/Cargo.toml rgba_to_x11`
Expected: PASS (both tests)

- [ ] **Step 6: Commit**

```bash
git add mobile/tests/tray/backend_xembed_tests.rs mobile/src/tray/backend_xembed.rs mobile/src/tray/mod.rs
git commit -m "test: add RGBA to X11 format conversion with tests"
```

---

### Task 6: Test and Implement X11 Atoms Structure

**Files:**
- Modify: `mobile/tests/tray/backend_xembed_tests.rs`
- Modify: `mobile/src/tray/backend_xembed.rs`

- [ ] **Step 1: Write failing test for Atoms initialization**

Add to `mobile/tests/tray/backend_xembed_tests.rs`:

```rust
#[test]
#[ignore] // Requires X11 display
fn test_atoms_new_interns_required_atoms() {
    use x11rb::rust_connection::RustConnection;

    let (conn, screen_num) = RustConnection::connect(None)
        .expect("X11 not available - run with DISPLAY set or xvfb-run");

    let atoms = bingtray::tray::backend_xembed::Atoms::new(&conn, screen_num)
        .expect("Failed to intern atoms");

    // Verify atoms are non-zero (successfully interned)
    assert_ne!(atoms.tray_selection, 0);
    assert_ne!(atoms.tray_opcode, 0);
    assert_ne!(atoms.xembed_info, 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path mobile/Cargo.toml atoms_new -- --ignored`
Expected: FAIL - `Atoms` struct not found

- [ ] **Step 3: Implement Atoms struct**

Add to `mobile/src/tray/backend_xembed.rs`:

```rust
use x11rb::protocol::xproto::Atom;
use x11rb::rust_connection::RustConnection;

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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `xvfb-run cargo test --manifest-path mobile/Cargo.toml atoms_new -- --ignored`
Expected: PASS (with Xvfb) or skip if no X11

- [ ] **Step 5: Commit**

```bash
git add mobile/tests/tray/backend_xembed_tests.rs mobile/src/tray/backend_xembed.rs
git commit -m "test: add Atoms struct for XEmbed protocol"
```

---

### Task 7: Implement XEmbed Backend Initialization

**Files:**
- Modify: `mobile/src/tray/backend_xembed.rs`

- [ ] **Step 1: Implement new() with X11 validation**

Update `XEmbedTrayBackend::new()`:

```rust
impl TrayBackend for XEmbedTrayBackend {
    fn new(logic: TrayLogic) -> Result<Self> {
        use x11rb::connection::Connection;

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

    fn run(self) -> Result<TrayExitAction> {
        todo!("Implement XEmbed event loop")
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --manifest-path mobile/Cargo.toml`
Expected: Success

- [ ] **Step 3: Commit**

```bash
git add mobile/src/tray/backend_xembed.rs
git commit -m "feat: add X11 validation to XEmbed backend init"
```

---

### Task 8: Implement XEmbed Protocol Handshake

**Files:**
- Modify: `mobile/src/tray/backend_xembed.rs`

- [ ] **Step 1: Add constants and helper functions**

Add to `mobile/src/tray/backend_xembed.rs`:

```rust
use x11rb::protocol::xproto::*;
use x11rb::connection::Connection;

const SYSTEM_TRAY_REQUEST_DOCK: u32 = 0;

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
```

- [ ] **Step 2: Implement run() with protocol handshake**

Update `run()`:

```rust
impl TrayBackend for XEmbedTrayBackend {
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
```

- [ ] **Step 3: Add event loop stub**

Add method:

```rust
impl XEmbedTrayBackend {
    fn event_loop(
        mut self,
        conn: RustConnection,
        icon_window: Window,
        screen: &Screen,
    ) -> Result<TrayExitAction> {
        loop {
            let event = conn.wait_for_event()?;

            match event {
                Event::Expose(e) if e.window == icon_window => {
                    render_icon(&conn, icon_window, screen)?;
                }
                Event::ButtonPress(e) if e.window == icon_window => {
                    log::info!("Icon clicked: button {}", e.detail);
                    if e.detail == 3 {
                        // Right-click - will add menu later
                        log::info!("Right-click detected");
                    }
                }
                _ => {}
            }
        }
    }
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check --manifest-path mobile/Cargo.toml`
Expected: Success

- [ ] **Step 5: Commit**

```bash
git add mobile/src/tray/backend_xembed.rs
git commit -m "feat: implement XEmbed protocol handshake and event loop"
```

---

## Phase 3: Menu System Implementation (TDD)

### Task 9: Test and Implement Menu Bounds Detection

**Files:**
- Create: `mobile/tests/tray/menu_popup_tests.rs`
- Create: `mobile/src/tray/menu_popup.rs`

- [ ] **Step 1: Write failing test for Rect bounds**

Create `mobile/tests/tray/menu_popup_tests.rs`:

```rust
//! Menu popup tests

#[cfg(target_os = "linux")]
mod menu_tests {
    use bingtray::tray::menu_popup::Rect;

    #[test]
    fn test_rect_contains_point_inside() {
        let rect = Rect {
            x: 10,
            y: 20,
            width: 100,
            height: 50,
        };

        assert!(rect.contains(50, 30));  // Center
        assert!(rect.contains(10, 20));  // Top-left corner
        assert!(rect.contains(109, 69)); // Bottom-right (just inside)
    }

    #[test]
    fn test_rect_contains_point_outside() {
        let rect = Rect {
            x: 10,
            y: 20,
            width: 100,
            height: 50,
        };

        assert!(!rect.contains(5, 30));   // Left of rect
        assert!(!rect.contains(150, 30)); // Right of rect
        assert!(!rect.contains(50, 10));  // Above rect
        assert!(!rect.contains(50, 100)); // Below rect
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path mobile/Cargo.toml rect_contains`
Expected: FAIL - module `menu_popup` not found

- [ ] **Step 3: Create menu_popup.rs stub**

Create `mobile/src/tray/menu_popup.rs`:

```rust
//! X11 popup menu for XEmbed tray

use anyhow::Result;

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
```

- [ ] **Step 4: Make module public**

Update `mobile/src/tray/mod.rs`:

```rust
#[cfg(target_os = "linux")]
pub mod menu_popup;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --manifest-path mobile/Cargo.toml rect_contains`
Expected: PASS (all tests)

- [ ] **Step 6: Commit**

```bash
git add mobile/tests/tray/menu_popup_tests.rs mobile/src/tray/menu_popup.rs mobile/src/tray/mod.rs
git commit -m "test: add Rect bounds detection with tests"
```

---

### Task 10: Implement Menu Action Types and MenuItem

**Files:**
- Modify: `mobile/src/tray/menu_popup.rs`

- [ ] **Step 1: Add MenuAction enum and MenuItem struct**

Add to `mobile/src/tray/menu_popup.rs`:

```rust
use super::TrayExitAction;

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
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --manifest-path mobile/Cargo.toml`
Expected: Success

- [ ] **Step 3: Commit**

```bash
git add mobile/src/tray/menu_popup.rs
git commit -m "feat: add MenuAction and MenuItem types"
```

---

### Task 11: Test and Implement Menu Size Calculation

**Files:**
- Modify: `mobile/tests/tray/menu_popup_tests.rs`
- Modify: `mobile/src/tray/menu_popup.rs`

- [ ] **Step 1: Write failing test for menu size**

Add to `mobile/tests/tray/menu_popup_tests.rs`:

```rust
use bingtray::tray::menu_popup::{MenuItem, MenuAction, calculate_menu_size};

#[test]
fn test_calculate_menu_size_single_item() {
    let items = vec![MenuItem::new(MenuAction::Quit, "Quit", true)];

    let (width, height) = calculate_menu_size(&items);

    assert!(width >= 100); // Minimum width
    assert_eq!(height, 30); // 5px top + 25px item
}

#[test]
fn test_calculate_menu_size_with_separator() {
    let items = vec![
        MenuItem::new(MenuAction::ShowApp, "Show App", true),
        MenuItem::separator(),
        MenuItem::new(MenuAction::Quit, "Quit", true),
    ];

    let (width, height) = calculate_menu_size(&items);

    assert_eq!(height, 30 + 10 + 30); // item + separator + item
}

#[test]
fn test_calculate_menu_size_long_label() {
    let long_label = "This is a very long menu item label that should increase width";
    let items = vec![MenuItem::new(MenuAction::ShowApp, long_label, true)];

    let (width, _) = calculate_menu_size(&items);

    assert!(width > 200); // Should be wider than minimum
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path mobile/Cargo.toml calculate_menu_size`
Expected: FAIL - function not found

- [ ] **Step 3: Implement calculate_menu_size**

Add to `mobile/src/tray/menu_popup.rs`:

```rust
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path mobile/Cargo.toml calculate_menu_size`
Expected: PASS (all tests)

- [ ] **Step 5: Commit**

```bash
git add mobile/tests/tray/menu_popup_tests.rs mobile/src/tray/menu_popup.rs
git commit -m "test: add menu size calculation with tests"
```

---

### Task 12: Implement MenuPopup Window Creation

**Files:**
- Modify: `mobile/src/tray/menu_popup.rs`

- [ ] **Step 1: Add MenuPopup struct and new() method**

Add to `mobile/src/tray/menu_popup.rs`:

```rust
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;
use x11rb::connection::Connection;
use egui_i18n::tr;
use super::logic::TrayLogic;

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
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --manifest-path mobile/Cargo.toml`
Expected: Success

- [ ] **Step 3: Commit**

```bash
git add mobile/src/tray/menu_popup.rs
git commit -m "feat: implement MenuPopup window creation"
```

---

### Task 13: Implement Menu Rendering

**Files:**
- Modify: `mobile/src/tray/menu_popup.rs`

- [ ] **Step 1: Add render method**

Add to `MenuPopup` impl:

```rust
impl MenuPopup {
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
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --manifest-path mobile/Cargo.toml`
Expected: Success

- [ ] **Step 3: Commit**

```bash
git add mobile/src/tray/menu_popup.rs
git commit -m "feat: implement menu rendering with X11 text"
```

---

### Task 14: Implement Menu Click Handling

**Files:**
- Modify: `mobile/src/tray/menu_popup.rs`

- [ ] **Step 1: Add handle_click method**

Add to `MenuPopup` impl:

```rust
impl MenuPopup {
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
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --manifest-path mobile/Cargo.toml`
Expected: Success

- [ ] **Step 3: Commit**

```bash
git add mobile/src/tray/menu_popup.rs
git commit -m "feat: implement menu click handling and actions"
```

---

## Phase 4: Integration

### Task 15: Integrate MenuPopup with XEmbed Backend

**Files:**
- Modify: `mobile/src/tray/backend_xembed.rs`

- [ ] **Step 1: Update event loop to use MenuPopup**

Update `event_loop()` method:

```rust
use super::menu_popup::MenuPopup;

impl XEmbedTrayBackend {
    fn event_loop(
        mut self,
        conn: RustConnection,
        icon_window: Window,
        screen: &Screen,
    ) -> Result<TrayExitAction> {
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
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --manifest-path mobile/Cargo.toml`
Expected: Success

- [ ] **Step 3: Commit**

```bash
git add mobile/src/tray/backend_xembed.rs
git commit -m "feat: integrate MenuPopup with XEmbed event loop"
```

---

### Task 16: Enable XEmbed Fallback in run_tray_mode

**Files:**
- Modify: `mobile/src/tray.rs`

- [ ] **Step 1: Update fallback logic to use XEmbed**

Update `run_tray_mode()`:

```rust
#[cfg(target_os = "linux")]
use tray::backend_xembed::XEmbedTrayBackend;

pub fn run_tray_mode() -> Result<TrayExitAction> {
    let logic = tray::logic::TrayLogic::new()?;

    #[cfg(target_os = "linux")]
    {
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

    #[cfg(not(target_os = "linux"))]
    {
        GtkTrayBackend::new(logic)?.run()
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --manifest-path mobile/Cargo.toml`
Expected: Success

- [ ] **Step 3: Test fallback logic compiles**

Run: `cargo build --manifest-path mobile/Cargo.toml`
Expected: Success

- [ ] **Step 4: Commit**

```bash
git add mobile/src/tray.rs
git commit -m "feat: enable XEmbed fallback in run_tray_mode"
```

---

## Phase 5: Testing & Documentation

### Task 17: Add Integration Test

**Files:**
- Create: `mobile/tests/tray_integration_test.rs`

- [ ] **Step 1: Create integration test**

Create `mobile/tests/tray_integration_test.rs`:

```rust
//! Integration test for tray backend fallback

#[cfg(target_os = "linux")]
#[test]
#[ignore] // Run with: cargo test --ignored
fn test_xembed_backend_initializes_with_x11() {
    use bingtray::tray::{TrayBackend, backend_xembed::XEmbedTrayBackend, logic::TrayLogic};

    // This test requires X11 display (run with xvfb-run)
    let logic = TrayLogic::new().expect("Failed to create TrayLogic");

    let result = XEmbedTrayBackend::new(logic);

    // Should succeed if X11 available and tray manager running
    // Or fail with helpful error if not
    match result {
        Ok(_) => {
            println!("XEmbed backend initialized successfully");
            // Don't actually run - would block forever
        }
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("X11") || msg.contains("tray manager"),
                "Error should mention X11 or tray manager: {}",
                msg
            );
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn test_fallback_error_message_quality() {
    use bingtray::tray::run_tray_mode;

    // Remove DISPLAY to force both backends to fail
    std::env::remove_var("DISPLAY");

    let result = run_tray_mode();

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();

    // Should mention both backends and give helpful hint
    assert!(error_msg.contains("GTK") || error_msg.contains("XEmbed"));
    assert!(error_msg.contains("Try:") || error_msg.contains("Install"));
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --manifest-path mobile/Cargo.toml`
Expected: PASS (non-ignored tests)

- [ ] **Step 3: Run ignored test with Xvfb (if available)**

Run: `xvfb-run cargo test --manifest-path mobile/Cargo.toml --ignored` (or skip if no X11)
Expected: PASS or informative error

- [ ] **Step 4: Commit**

```bash
git add mobile/tests/tray_integration_test.rs
git commit -m "test: add integration tests for tray backend fallback"
```

---

### Task 18: Update CLAUDE.md Documentation

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Document XEmbed fallback**

Add to section 11 "Dependencies" in `CLAUDE.md`:

```markdown
### Linux Tray Icon

**Primary**: GTK backend via `tray-icon` crate
- Requires: `libayatana-appindicator3`

**Fallback**: XEmbed protocol via `x11rb`
- Activates when GTK unavailable
- Requires: X11 display + system tray manager (i3bar, polybar, etc.)
```

Add to section 12 "Recent Changes":

```markdown
### XEmbed Tray Fallback (2026-06-16)

**Completed**:
- ✅ Trait-based tray backend abstraction
- ✅ XEmbed protocol implementation using x11rb
- ✅ Runtime fallback (GTK → XEmbed)
- ✅ X11 popup menu system
- ✅ Full feature parity between backends

**Impact**:
- Tray icon works on minimal X11 systems without GTK
- Clear error messages when both backends fail
- ~1.5MB binary size increase (x11rb dependency)

**Documentation**:
- Design spec: `docs/superpowers/specs/2026-06-16-xembed-tray-fallback-design.md`
- Implementation plan: `docs/superpowers/plans/2026-06-16-xembed-tray-fallback.md`
```

- [ ] **Step 2: Verify markdown renders correctly**

Run: `cat CLAUDE.md | grep -A 5 "XEmbed"`
Expected: Shows new section

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: document XEmbed tray fallback in CLAUDE.md"
```

---

### Task 19: Final Verification and Summary

**Files:**
- None (verification only)

- [ ] **Step 1: Run full test suite**

Run: `cargo test --manifest-path mobile/Cargo.toml`
Expected: All tests pass

- [ ] **Step 2: Build release binary**

Run: `cargo build --release --manifest-path mobile/Cargo.toml`
Expected: Success

- [ ] **Step 3: Check binary size increase**

Run: `ls -lh target/release/bingtray`
Expected: ~1-2MB larger than before (x11rb added)

- [ ] **Step 4: Manual test (if X11 available)**

Test scenarios:
1. **With GTK**: Run normally, should use GTK backend
2. **Without GTK**: Remove libayatana, should fall back to XEmbed
3. **No X11**: Remove DISPLAY, should show helpful error

- [ ] **Step 5: Create summary commit**

```bash
git add -A
git commit -m "feat: complete XEmbed tray icon fallback implementation

Implements runtime fallback from GTK to XEmbed protocol for system tray
on Linux/BSD systems without libayatana-appindicator.

Key components:
- TrayBackend trait abstraction
- GtkTrayBackend (refactored existing code)
- XEmbedTrayBackend (new x11rb implementation)
- MenuPopup (X11 popup menu window)
- Shared TrayLogic between backends

Features:
- Runtime detection (try GTK, fall back to XEmbed)
- Full feature parity (all menu items work)
- Clear error messages on failure
- ~1.5MB binary size increase

Tests:
- RGBA conversion tests
- Atoms initialization tests
- Menu bounds detection tests
- Menu size calculation tests
- Integration tests

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Success Criteria

- [ ] All unit tests pass
- [ ] Integration tests pass (or skip gracefully without X11)
- [ ] GTK backend works identically to before refactoring
- [ ] XEmbed backend initializes on minimal X11 systems
- [ ] Menu shows on right-click with all items
- [ ] All menu operations work (next, keep, blacklist, favorite)
- [ ] Fallback happens automatically when GTK unavailable
- [ ] Clear error message when both backends fail
- [ ] Documentation updated
- [ ] Binary compiles and runs

## Estimated Time

- Phase 1 (Refactoring): 1-2 hours
- Phase 2 (XEmbed Core): 3-4 hours
- Phase 3 (Menu System): 2-3 hours
- Phase 4 (Integration): 1 hour
- Phase 5 (Testing): 1-2 hours

**Total**: 8-12 hours
