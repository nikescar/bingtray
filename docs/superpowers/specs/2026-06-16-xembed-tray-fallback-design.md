# XEmbed Tray Icon Fallback Design

**Date:** 2026-06-16  
**Author:** Claude Sonnet 4.5  
**Status:** Approved

## Overview

Implement XEmbed protocol-based tray icon as runtime fallback when libayatana-appindicator is not available on Linux/BSD/Unix systems.

**Current state:** BingTray uses `tray-icon` crate with GTK backend, which depends on libayatana-appindicator.

**Problem:** On minimal X11 systems without GTK/ayatana, tray functionality is unavailable.

**Solution:** Implement XEmbed (freedesktop.org System Tray Protocol) fallback using raw X11 protocol via `x11rb`.

## Requirements

1. **Runtime fallback:** Try GTK first, fall back to XEmbed if GTK initialization fails
2. **Platform support:** Linux, BSDs, and Unix variants with X11
3. **Feature parity:** All current tray features (menu, wallpaper operations) work in XEmbed mode
4. **Static icon:** Display logo.png (same as GTK backend)
5. **X11 popup menu:** Right-click shows menu via undecorated X11 window
6. **Graceful degradation:** Clear error messages when both backends fail

## Architecture

### File Structure

```
mobile/src/
├── tray.rs                    # Public API (run_tray_mode, init_tray_event_handlers)
├── tray/
│   ├── mod.rs                 # TrayBackend trait, backend selection logic
│   ├── backend_gtk.rs         # GtkTrayBackend (refactored from current tray.rs)
│   ├── backend_xembed.rs      # XEmbedTrayBackend (new)
│   ├── logic.rs               # TrayLogic (extracted, shared between backends)
│   └── menu_popup.rs          # X11 popup menu window for XEmbed
```

### Core Abstraction

**TrayBackend trait:**
```rust
pub trait TrayBackend {
    fn new(logic: TrayLogic) -> Result<Self> where Self: Sized;
    fn run(self) -> Result<TrayExitAction>;
}
```

**Fallback logic:**
```rust
pub fn run_tray_mode() -> Result<TrayExitAction> {
    let logic = TrayLogic::new()?;
    
    match GtkTrayBackend::new(logic.clone()) {
        Ok(backend) => {
            log::info!("Using GTK tray backend");
            backend.run()
        }
        Err(e) => {
            log::warn!("GTK tray unavailable: {}, falling back to XEmbed", e);
            XEmbedTrayBackend::new(logic)?.run()
        }
    }
}
```

**Design decisions:**
- Trait abstraction enables clean separation and testability
- `TrayLogic` shared between backends (business logic reuse)
- Backends own their event loops
- Clone `TrayLogic` to support fallback (GTK fails, XEmbed reuses)

## Component Design

### 1. TrayLogic (Shared Business Logic)

**Location:** `mobile/src/tray/logic.rs`

**Extraction:** Move existing `TrayLogic` struct from `tray.rs` to separate module.

**Interface:**
```rust
pub struct TrayLogic {
    conn: diesel::SqliteConnection,
}

impl TrayLogic {
    pub fn new() -> Result<Self>;
    
    // State queries
    pub fn get_wallpaper_page_status(&mut self) -> String;
    pub fn has_next_available(&mut self) -> bool;
    pub fn get_current_image_title(&mut self) -> String;
    pub fn can_keep(&mut self) -> bool;
    pub fn can_blacklist(&mut self) -> bool;
    pub fn has_kept_wallpapers(&mut self) -> bool;
    
    // Operations
    pub fn open_cache_directory(&self) -> Result<()>;
    pub fn set_next_market_wallpaper(&mut self) -> Result<bool>;
    pub fn keep_current_image(&mut self) -> Result<()>;
    pub fn blacklist_current_image(&mut self) -> Result<()>;
    pub fn set_kept_wallpaper(&mut self) -> Result<bool>;
}

impl Clone for TrayLogic {
    fn clone(&self) -> Self {
        Self::new().expect("Failed to clone TrayLogic")
    }
}
```

**Changes:** None to logic, just extracted to separate file.

### 2. GtkTrayBackend

**Location:** `mobile/src/tray/backend_gtk.rs`

**Refactoring:** Wrap existing `run_tray_mode()` implementation in trait.

```rust
pub struct GtkTrayBackend {
    logic: TrayLogic,
}

impl TrayBackend for GtkTrayBackend {
    fn new(logic: TrayLogic) -> Result<Self> {
        // Early GTK availability check
        if !is_gtk_available() {
            return Err(anyhow!("GTK tray manager not available"));
        }
        Ok(Self { logic })
    }
    
    fn run(mut self) -> Result<TrayExitAction> {
        // Current run_tray_mode() implementation
        // - Event loop with tao
        // - tray-icon crate
        // - Menu handling
    }
}

fn is_gtk_available() -> bool {
    EventLoopBuilder::<UserEvent>::with_user_event()
        .build()
        .is_ok()
}
```

**Changes:** Minimal - wrap existing code, add early failure detection.

### 3. XEmbedTrayBackend

**Location:** `mobile/src/tray/backend_xembed.rs`

**New implementation using x11rb.**

**Responsibilities:**
- X11 connection management
- XEmbed protocol handshake
- Icon window creation and rendering
- Event handling (expose, button clicks)
- Menu popup coordination

**Structure:**
```rust
pub struct XEmbedTrayBackend {
    logic: TrayLogic,
}

impl TrayBackend for XEmbedTrayBackend {
    fn new(logic: TrayLogic) -> Result<Self> {
        // Verify X11 connection available
        let _ = RustConnection::connect(None)
            .map_err(|e| anyhow!("X11 not available: {}", e))?;
        Ok(Self { logic })
    }
    
    fn run(mut self) -> Result<TrayExitAction> {
        // 1. Connect to X11
        // 2. Find system tray manager
        // 3. Create icon window
        // 4. Render logo.png
        // 5. Send SYSTEM_TRAY_REQUEST_DOCK
        // 6. Event loop
    }
}
```

## XEmbed Protocol Implementation

### Protocol Handshake

**Initialization sequence:**

1. **Connect to X11 server**
   ```rust
   let (conn, screen_num) = RustConnection::connect(None)?;
   let screen = &conn.setup().roots[screen_num];
   ```

2. **Intern required atoms**
   ```rust
   struct Atoms {
       tray_selection: Atom,   // _NET_SYSTEM_TRAY_S{screen}
       tray_opcode: Atom,       // _NET_SYSTEM_TRAY_OPCODE
       xembed_info: Atom,       // _XEMBED_INFO
   }
   
   impl Atoms {
       fn new(conn: &RustConnection, screen: usize) -> Result<Self> {
           let tray_selection = conn.intern_atom(
               false,
               format!("_NET_SYSTEM_TRAY_S{}", screen).as_bytes()
           )?.reply()?.atom;
           
           let tray_opcode = conn.intern_atom(false, b"_NET_SYSTEM_TRAY_OPCODE")?
               .reply()?.atom;
           
           let xembed_info = conn.intern_atom(false, b"_XEMBED_INFO")?
               .reply()?.atom;
           
           Ok(Self { tray_selection, tray_opcode, xembed_info })
       }
   }
   ```

3. **Find system tray manager window**
   ```rust
   let tray_manager = conn.get_selection_owner(atoms.tray_selection)?
       .reply()?.owner;
   
   if tray_manager == x11rb::NONE {
       return Err(anyhow!("No system tray manager found"));
   }
   ```

4. **Create icon window**
   ```rust
   let icon_window = conn.generate_id()?;
   conn.create_window(
       x11rb::COPY_FROM_PARENT as u8,
       icon_window,
       screen.root,
       0, 0, 24, 24,  // Initial size
       0,
       WindowClass::INPUT_OUTPUT,
       screen.root_visual,
       &CreateWindowAux::new()
           .background_pixel(screen.black_pixel)
           .event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS),
   )?;
   ```

5. **Set _XEMBED_INFO property**
   ```rust
   let xembed_info = [0u32, 1u32]; // version=0, flags=XEMBED_MAPPED
   conn.change_property32(
       PropMode::REPLACE,
       icon_window,
       atoms.xembed_info,
       AtomEnum::CARDINAL,
       &xembed_info,
   )?;
   ```

6. **Send SYSTEM_TRAY_REQUEST_DOCK message**
   ```rust
   const SYSTEM_TRAY_REQUEST_DOCK: u32 = 0;
   
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
   conn.flush()?;
   ```

### Icon Rendering

**Load and render logo.png to X11 window:**

```rust
fn render_icon(
    conn: &RustConnection,
    window: Window,
    screen: &Screen,
) -> Result<()> {
    // Load embedded icon (same as GTK backend)
    let icon_bytes = include_bytes!("../../resources/logo.png");
    let image = image::load_from_memory(icon_bytes)?;
    let rgba = image.to_rgba8();
    
    // Create pixmap for double-buffering
    let pixmap = conn.generate_id()?;
    conn.create_pixmap(screen.root_depth, pixmap, window, 24, 24)?;
    
    // Create graphics context
    let gc = conn.generate_id()?;
    conn.create_gc(gc, pixmap, &CreateGCAux::new())?;
    
    // Convert RGBA to X11 BGRA format
    let image_data: Vec<u8> = rgba.pixels()
        .flat_map(|p| [p[2], p[1], p[0], p[3]]) // RGBA -> BGRA
        .collect();
    
    // Draw to pixmap
    conn.put_image(
        ImageFormat::Z_PIXMAP,
        pixmap,
        gc,
        24, 24,
        0, 0,
        0,
        screen.root_depth,
        &image_data,
    )?;
    
    // Copy pixmap to window
    conn.copy_area(pixmap, window, gc, 0, 0, 0, 0, 24, 24)?;
    conn.flush()?;
    
    Ok(())
}
```

### Event Loop

**Main event processing:**

```rust
fn event_loop(
    mut self,
    conn: RustConnection,
    icon_window: Window,
    screen: &Screen,
) -> Result<TrayExitAction> {
    let mut menu_popup: Option<MenuPopup> = None;
    
    loop {
        let event = conn.wait_for_event()?;
        
        match event {
            Event::Expose(e) if e.window == icon_window => {
                // Redraw icon after window manager changes
                render_icon(&conn, icon_window, screen)?;
            }
            
            Event::ButtonPress(e) if e.window == icon_window => {
                match e.detail {
                    1 => {
                        // Left click - close menu if open
                        menu_popup = None;
                    }
                    3 => {
                        // Right click - show menu
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
            
            Event::ButtonPress(e) if menu_popup.as_ref()
                .map(|m| m.window == e.window)
                .unwrap_or(false) => 
            {
                // Menu item clicked
                if let Some(action) = menu_popup.as_mut()
                    .unwrap()
                    .handle_click(e.event_x, e.event_y, &mut self.logic)? 
                {
                    return Ok(action);
                }
                menu_popup = None;
            }
            
            _ => {}
        }
    }
}
```

**Event handling:**
- `Expose` → redraw icon
- `ButtonPress(1)` → left-click, close menu
- `ButtonPress(3)` → right-click, show menu
- Menu click → execute action, maybe exit

## Menu Popup System

**Location:** `mobile/src/tray/menu_popup.rs`

### Structure

```rust
pub struct MenuPopup {
    pub window: Window,
    items: Vec<MenuItem>,
}

struct MenuItem {
    id: MenuAction,
    label: String,
    enabled: bool,
    bounds: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum MenuAction {
    ShowApp,
    CacheDir,
    NextMarket,
    KeepCurrent,
    BlacklistCurrent,
    RandomFavorite,
    Quit,
}

struct Rect {
    x: i16,
    y: i16,
    width: u16,
    height: u16,
}
```

### Menu Creation

**Build menu based on TrayLogic state:**

```rust
impl MenuPopup {
    pub fn new(
        conn: &RustConnection,
        screen: &Screen,
        x: i16,
        y: i16,
        logic: &mut TrayLogic,
    ) -> Result<Self> {
        // Build menu items
        let items = vec![
            MenuItem::new(MenuAction::ShowApp, tr!("tray-show-app"), true),
            MenuItem::separator(),
            MenuItem::new(MenuAction::CacheDir, tr!("tray-cache-dir"), true),
            MenuItem::new(
                MenuAction::NextMarket,
                format!("{}\n{}", tr!("tray-next-market"), logic.get_wallpaper_page_status()),
                logic.has_next_available(),
            ),
            MenuItem::new(
                MenuAction::KeepCurrent,
                format_keep_label(logic),
                logic.can_keep(),
            ),
            MenuItem::new(
                MenuAction::BlacklistCurrent,
                format_blacklist_label(logic),
                logic.can_blacklist(),
            ),
            MenuItem::new(
                MenuAction::RandomFavorite,
                tr!("tray-random-favorite"),
                logic.has_kept_wallpapers(),
            ),
            MenuItem::separator(),
            MenuItem::new(MenuAction::Quit, tr!("tray-quit"), true),
        ];
        
        // Calculate window size
        let (width, height) = calculate_menu_size(&items);
        
        // Create undecorated popup window
        let window = conn.generate_id()?;
        conn.create_window(
            x11rb::COPY_FROM_PARENT as u8,
            window,
            screen.root,
            x, y,
            width, height,
            1, // 1px border
            WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &CreateWindowAux::new()
                .background_pixel(0xFFFFFF) // White
                .border_pixel(0x000000)     // Black
                .override_redirect(1)        // No WM decoration
                .event_mask(
                    EventMask::EXPOSURE |
                    EventMask::BUTTON_PRESS |
                    EventMask::LEAVE_WINDOW
                ),
        )?;
        
        conn.map_window(window)?;
        conn.flush()?;
        
        Ok(Self { window, items })
    }
}
```

### Menu Rendering

**Simple text rendering with X11 core fonts:**

```rust
impl MenuPopup {
    fn render(&self, conn: &RustConnection) -> Result<()> {
        let gc = conn.generate_id()?;
        conn.create_gc(gc, self.window, &CreateGCAux::new())?;
        
        let mut y = 5;
        for item in &self.items {
            if item.is_separator() {
                // Draw horizontal line
                conn.poly_line(
                    CoordMode::ORIGIN,
                    self.window,
                    gc,
                    &[Point { x: 5, y }, Point { x: 200, y }],
                )?;
                y += 5;
            } else {
                // Draw text
                let color = if item.enabled { 0x000000 } else { 0x808080 };
                conn.change_gc(gc, &ChangeGCAux::new().foreground(color))?;
                
                conn.image_text8(
                    self.window,
                    gc,
                    10,
                    y + 15,
                    item.label.as_bytes(),
                )?;
                
                // Store bounds for click detection
                item.bounds = Rect { x: 0, y, width: 200, height: 25 };
                y += 25;
            }
        }
        
        conn.flush()?;
        Ok(())
    }
}
```

### Click Handling

**Detect which menu item was clicked:**

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
        
        Ok(None) // Clicked outside items
    }
    
    fn execute_action(
        &mut self,
        action: MenuAction,
        logic: &mut TrayLogic,
    ) -> Result<Option<TrayExitAction>> {
        match action {
            MenuAction::ShowApp => Ok(Some(TrayExitAction::OpenGui)),
            MenuAction::Quit => Ok(Some(TrayExitAction::Quit)),
            MenuAction::CacheDir => {
                logic.open_cache_directory()?;
                Ok(None)
            }
            MenuAction::NextMarket => {
                logic.set_next_market_wallpaper()?;
                Ok(None)
            }
            MenuAction::KeepCurrent => {
                logic.keep_current_image()?;
                Ok(None)
            }
            MenuAction::BlacklistCurrent => {
                logic.blacklist_current_image()?;
                Ok(None)
            }
            MenuAction::RandomFavorite => {
                logic.set_kept_wallpaper()?;
                Ok(None)
            }
        }
    }
}
```

**Menu behavior:**
- Undecorated window (no title bar)
- White background, black 1px border
- Text-only items (no icons)
- Disabled items grayed out (#808080)
- Click executes action, closes menu
- Leave window event auto-closes menu

## Error Handling

### Fallback Decision Logic

```rust
pub fn run_tray_mode() -> Result<TrayExitAction> {
    let logic = TrayLogic::new()?; // Fail fast on DB error
    
    // Try GTK first
    match GtkTrayBackend::new(logic.clone()) {
        Ok(backend) => {
            log::info!("Using GTK tray backend");
            backend.run()
        }
        Err(e) => {
            log::warn!("GTK tray unavailable: {}", e);
            
            // Try XEmbed fallback
            match XEmbedTrayBackend::new(logic) {
                Ok(backend) => {
                    log::info!("Falling back to XEmbed tray");
                    backend.run()
                }
                Err(xembed_err) => {
                    Err(anyhow!(
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
```

### GTK Failure Cases

**What triggers XEmbed fallback:**
- `libayatana-appindicator.so` not found
- GTK initialization fails
- Event loop creation fails
- No GTK tray manager running

**Early detection in `GtkTrayBackend::new()`:**
```rust
fn new(logic: TrayLogic) -> Result<Self> {
    // Test event loop creation (fast check)
    let _event_loop = EventLoopBuilder::<UserEvent>::with_user_event()
        .build()
        .map_err(|e| anyhow!("GTK event loop failed: {}", e))?;
    
    // If this succeeds, GTK is likely available
    Ok(Self { logic })
}
```

### XEmbed Failure Cases

**What causes complete failure:**
- No X11 display (Wayland-only, no XWayland)
- No system tray manager (no i3bar, polybar, tint2, etc.)
- X11 connection fails

**Detection in `XEmbedTrayBackend::new()`:**
```rust
fn new(logic: TrayLogic) -> Result<Self> {
    // Test X11 connection
    let (conn, screen_num) = RustConnection::connect(None)
        .map_err(|e| anyhow!("X11 connection failed: {}", e))?;
    
    // Test system tray manager exists
    let atoms = Atoms::new(&conn, screen_num)?;
    let tray_manager = conn.get_selection_owner(atoms.tray_selection)?
        .reply()?.owner;
    
    if tray_manager == x11rb::NONE {
        return Err(anyhow!(
            "No system tray manager found.\n\
             Install: i3bar, polybar, tint2, or other tray-enabled panel"
        ));
    }
    
    Ok(Self { logic })
}
```

### Runtime Error Handling

**During event loop:**
```rust
fn event_loop(...) -> Result<TrayExitAction> {
    loop {
        match conn.wait_for_event() {
            Ok(event) => {
                // Handle event
            }
            Err(e) => {
                log::error!("X11 connection error: {}", e);
                return Ok(TrayExitAction::Quit); // Graceful exit
            }
        }
    }
}
```

**Menu operations:**
```rust
MenuAction::NextMarket => {
    match logic.set_next_market_wallpaper() {
        Ok(_) => log::info!("Wallpaper changed"),
        Err(e) => log::error!("Failed to change wallpaper: {}", e),
    }
    Ok(None) // Don't crash, just log error
}
```

### User-Facing Error Messages

**Helpful installation hints:**

| Error | Suggested Fix |
|-------|--------------|
| "libayatana not found" | `apt install libayatana-appindicator3-1` |
| "No system tray manager" | Install `i3bar`, `polybar`, `tint2`, or similar |
| "X11 connection failed" | Check `DISPLAY` variable, ensure X11/XWayland running |

**Philosophy:**
- Fail fast during initialization (bad config → clear error)
- Degrade gracefully during runtime (menu operation fails → log, continue)
- Provide actionable error messages (not just error codes)

## Testing Strategy

### Test Structure

```
mobile/tests/
├── tray_tests.rs              # Public API & fallback logic tests
├── tray/
│   ├── logic_tests.rs         # TrayLogic unit tests
│   ├── backend_gtk_tests.rs   # GTK backend tests
│   ├── backend_xembed_tests.rs # XEmbed pure function tests
│   └── menu_popup_tests.rs    # Menu state & click detection tests
```

### Unit Tests (TDD)

**Pure functions (test first):**

```rust
// Test RGBA → BGRA conversion
#[test]
fn test_rgba_to_x11_format() {
    let rgba = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 128, 64, 32]));
    let x11_data = rgba_to_x11_format(&rgba);
    
    assert_eq!(x11_data, vec![64, 128, 255, 32]); // BGRA
}

// Test menu size calculation
#[test]
fn test_calculate_menu_size() {
    let items = vec![
        MenuItem::new(MenuAction::ShowApp, "Show App", true),
        MenuItem::separator(),
        MenuItem::new(MenuAction::Quit, "Quit", true),
    ];
    
    let (width, height) = calculate_menu_size(&items);
    assert_eq!(height, 25 + 5 + 25); // Item + separator + item
}

// Test menu item bounds detection
#[test]
fn test_menu_bounds_contains() {
    let rect = Rect { x: 10, y: 20, width: 100, height: 25 };
    
    assert!(rect.contains(50, 30));  // Inside
    assert!(!rect.contains(5, 30));  // Outside left
    assert!(!rect.contains(150, 30)); // Outside right
}

// Test menu item enabled state
#[test]
fn test_menu_item_enabled_based_on_logic() {
    let mut logic = TrayLogic::new().unwrap();
    
    // No wallpaper set
    assert!(!should_enable_keep(&logic));
    assert!(!should_enable_blacklist(&logic));
    
    // Set wallpaper
    logic.set_next_market_wallpaper().unwrap();
    
    // Now enabled
    assert!(should_enable_keep(&logic));
    assert!(should_enable_blacklist(&logic));
}
```

### Integration Tests

**Fallback logic (requires X11 or mocking):**

```rust
#[test]
#[cfg(target_os = "linux")]
fn test_fallback_to_xembed_when_gtk_fails() {
    // This test verifies error handling, not full execution
    // (Full execution requires display server)
    
    std::env::remove_var("GTK_MODULES"); // Force GTK unavailable
    
    let result = run_tray_mode();
    
    // Should attempt fallback, not panic
    // Error message should mention both backends
    if result.is_err() {
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("GTK") || msg.contains("XEmbed"));
    }
}

#[test]
#[ignore] // Run with: cargo test -- --ignored
fn test_xembed_full_flow_with_xvfb() {
    // Integration test requiring Xvfb
    // Verifies full XEmbed initialization
    
    let logic = TrayLogic::new().unwrap();
    let backend = XEmbedTrayBackend::new(logic).unwrap();
    
    // Verify atoms initialized
    // Verify window created
    // Verify icon rendered
}
```

### TDD Workflow

**Implementation order:**

1. **Phase 1: Refactoring (existing tests)**
   - Extract `TrayLogic` to `tray/logic.rs`
   - Move existing tests to `tray/logic_tests.rs`
   - Verify all tests still pass

2. **Phase 2: Trait & GTK backend (minimal tests)**
   - Define `TrayBackend` trait
   - Wrap existing code in `GtkTrayBackend`
   - Test: GTK backend still works

3. **Phase 3: Pure functions (TDD)**
   - RED: Write test for `rgba_to_x11_format`
   - GREEN: Implement conversion
   - RED: Write test for `calculate_menu_size`
   - GREEN: Implement calculation
   - REFACTOR: Clean up

4. **Phase 4: XEmbed protocol (TDD)**
   - RED: Write test for `Atoms::new()`
   - GREEN: Implement atom interning
   - RED: Write test for icon rendering
   - GREEN: Implement rendering
   - Repeat for each protocol step

5. **Phase 5: Menu system (TDD)**
   - RED: Test menu item bounds detection
   - GREEN: Implement `Rect::contains()`
   - RED: Test click handling
   - GREEN: Implement `handle_click()`
   - REFACTOR: Extract helpers

6. **Phase 6: Integration (manual + CI)**
   - Test with Xvfb in CI
   - Manual testing on real systems
   - Edge case handling

### CI/CD

**GitHub Actions with Xvfb:**

```yaml
test:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    
    - name: Install dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y \
          libayatana-appindicator3-dev \
          xvfb \
          libx11-dev \
          libgtk-3-dev
    
    - name: Run unit tests
      run: cargo test --manifest-path mobile/Cargo.toml
    
    - name: Run integration tests (headless)
      run: xvfb-run cargo test --manifest-path mobile/Cargo.toml -- --ignored
```

**Testing pyramid:**
- Many: Pure function tests (fast, no X11 required)
- Some: Logic tests (database, business logic)
- Few: Integration tests (full flow with Xvfb)

## Dependencies

**Add to `mobile/Cargo.toml`:**

```toml
[target.'cfg(target_os = "linux")'.dependencies]
x11rb = { version = "0.13", features = ["allow-unsafe-code"] }
```

**Keep existing:**
```toml
tray-icon = { version = "0.24", default-features = false, features = ["gtk"] }
gtk = "0.18"
```

**Why both?**
- `tray-icon` + `gtk` for GTK backend (primary)
- `x11rb` for XEmbed backend (fallback)
- Adds ~1.5MB to binary size (acceptable for fallback functionality)

## Implementation Phases

### Phase 1: Refactoring (1-2 hours)
- Extract `TrayLogic` to `tray/logic.rs`
- Move existing tests
- Verify no behavior changes

### Phase 2: Trait Abstraction (1 hour)
- Define `TrayBackend` trait in `tray/mod.rs`
- Create `GtkTrayBackend` in `tray/backend_gtk.rs`
- Wrap existing code, minimal changes

### Phase 3: XEmbed Core (3-4 hours, TDD)
- Implement `XEmbedTrayBackend` in `tray/backend_xembed.rs`
- Protocol handshake (atoms, window, docking)
- Icon rendering (RGBA → BGRA, put_image)
- Event loop (Expose, ButtonPress)

### Phase 4: Menu System (2-3 hours, TDD)
- Implement `MenuPopup` in `tray/menu_popup.rs`
- Menu creation (items, sizing)
- Rendering (text, separators)
- Click detection and action execution

### Phase 5: Integration & Testing (2-3 hours)
- Fallback logic integration
- Error handling refinement
- Integration tests with Xvfb
- Manual testing on real systems

**Total estimated time:** 9-13 hours

## Success Criteria

- [ ] GTK backend works identically to current implementation
- [ ] XEmbed backend initializes on systems without GTK
- [ ] XEmbed shows tray icon with logo.png
- [ ] Right-click menu works in XEmbed mode
- [ ] All menu operations work (next, keep, blacklist, favorite)
- [ ] Fallback happens automatically and transparently
- [ ] Clear error messages when both backends fail
- [ ] All unit tests pass
- [ ] Integration tests pass in CI (Xvfb)
- [ ] Manual testing confirms functionality on:
  - Ubuntu with GTK (uses GTK backend)
  - Minimal i3 setup without libayatana (uses XEmbed backend)
  - OpenBSD with X11 (uses XEmbed backend)

## Future Enhancements (Out of Scope)

- Wayland native protocol support (StatusNotifierItem)
- Animated icons or status badges
- Custom fonts in menu (currently uses X11 core fonts)
- Menu icons (currently text-only)
- Menu theming (currently hardcoded colors)
- Notification system integration
- SNI/DBus backend for modern Linux desktops

## References

- [freedesktop.org System Tray Protocol](https://specifications.freedesktop.org/systemtray-spec/systemtray-spec-latest.html)
- [XEmbed Protocol Specification](https://specifications.freedesktop.org/xembed-spec/xembed-spec-latest.html)
- [x11rb documentation](https://docs.rs/x11rb/)
- [tray-icon crate](https://docs.rs/tray-icon/)
