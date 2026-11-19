# BingTray - Project Guide

## Quick Start

**What**: Cross-platform Bing wallpaper manager  
**Platforms**: Windows, macOS, Linux, Android  
**Architectures**: aarch64, armv7, x86_64

**Run modes**:
```bash
cargo run              # CLI (terminal detected)
cargo run -- --gui     # GUI window
cargo run -- --tray    # System tray only
```

**Features**: Download/set wallpapers, carousel view (20/page), favorites, blacklist, crop controls

## Architecture

See **[ARCH.sudolang.md](ARCH.sudolang.md)** for complete architecture specification.

**Core pattern**: MVVM (Model-View-ViewModel)
- **Database** → SQLite + Diesel ORM
- **ViewModel** → Message-passing with async (GUI) or sync (CLI) modes
- **UI** → egui (Desktop/Android), terminal (CLI), tray icon

**Key constraint**: Historical data loads sync BEFORE UI renders. Cache manager populates 3 images in background.

## Repository Structure

```
mobile/
├── src/
│   ├── db/                      # Database layer (Diesel SQLite)
│   │   ├── mod.rs              # Connection & migrations
│   │   ├── models.rs           # BingImage, MarketCode, ConfigKv
│   │   └── operations.rs       # CRUD operations
│   ├── viewmodel/              # ViewModel layer (MVVM)
│   │   ├── mod.rs              # Command/Event types, ViewModel struct
│   │   ├── commands.rs         # Command handlers
│   │   ├── background.rs       # Async background thread
│   │   ├── sources.rs          # Image sources (Bing API, GitHub archive)
│   │   └── cache_manager.rs   # Image caching & historical data
│   ├── bingtray.rs             # Main GUI (egui + Material3)
│   ├── main.rs                 # Desktop entry point
│   ├── main_android.rs         # Android entry point
│   ├── cli.rs                  # CLI entry point
│   ├── tray.rs                 # System tray (GTK + XEmbed fallback)
│   ├── api_bingimage.rs        # Bing API client
│   ├── api_setwallpaper.rs     # Wallpaper setter (desktop)
│   ├── utils.rs                # Utilities (sanitize_filename, etc.)
│   ├── i18n.rs                 # Internationalization
│   └── schema.rs               # Diesel schema (auto-generated)
├── migrations/                  # Diesel SQL migrations
├── tests/                       # Unit & integration tests
└── Cargo.toml
reference/                       # Reference implementations
└── ARCH.sudolang.md            # Complete architecture specification
```

## Database Layer

**ORM**: Diesel SQLite with embedded migrations  
**Models**: `BingImage`, `MarketCode`, `ConfigKv`  
**Path**:
- Desktop: `~/.config/bingtray/bingtray.db`
- Android: `/data/data/pe.nikescar.bingtray/files/bingtray.db`

**Features**: WAL mode, 30s busy timeout, auto-migrations on first connection

**Create migration**:
```bash
cd mobile
diesel migration generate <name>
# Edit up.sql/down.sql
```

## ViewModel Layer

**Pattern**: MVVM with message-passing

**Threading modes**:
- **Async** (GUI/Android/Tray): Background thread + `std::sync::mpsc` + asupersync runtime
- **Sync** (CLI): Direct calls, no threading overhead

**Messages**:
- `ViewModelCommand`: UI → Background (DownloadImages, SetWallpaper, ToggleFavorite, etc.)
- `ViewModelEvent`: Background → UI (DownloadComplete, ImagesLoaded, WallpaperSet, etc.)

**Cache Manager**:
- Populates historical data sync BEFORE UI loads
- Caches 3 images in background for wallpaper
- Stores in `~/.config/bingtray/cache/images`

## UI Layer

| Platform | Framework | Entry Point | ViewModel Mode |
|----------|-----------|-------------|----------------|
| Desktop GUI | eframe + egui | `main.rs` | Async |
| Desktop Tray | GTK/XEmbed | `tray.rs` | Async |
| Desktop CLI | Terminal | `cli.rs` | Sync |
| Android | Native activity + egui | `main_android.rs` | Async |

**GUI features**: Carousel (20/page), filters (All/Favorites/Blacklisted/Unprocessed), crop controls, Material3 theme

**Tray backends**: GTK (`tray-icon`) → XEmbed (`x11rb`) fallback

## Build & Test

**Build**:
```bash
cargo build              # Dev (opt-level 0)
cargo build --release    # Release (LTO + stripped)
```

**Test**:
```bash
cargo test --manifest-path mobile/Cargo.toml
```

**Format & Lint**:
```bash
cargo fmt
cargo clippy
```

**Run modes**: Binary auto-detects mode at runtime (terminal/flags/double-click)

## Dependencies

**Core**: eframe, egui, diesel, diesel_migrations, asupersync, anyhow, serde, image, ehttp  
**Desktop**: tray-icon, x11rb (XEmbed fallback), wallpaper  
**Android**: ndk-context, jni, android-activity  
**UI**: egui_material3, egui_i18n

## Development Workflow

1. Edit code in `mobile/src/`
2. Run: `cargo run [--gui|--tray]` or just `cargo run` (CLI)
3. Test: `cargo test --manifest-path mobile/Cargo.toml`
4. Format/lint: `cargo fmt && cargo clippy`

## Platform-Specific Notes

**Linux tray**: Requires `libayatana-appindicator3` (GTK) or X11 (XEmbed fallback)  
**Android build**: Via gradle in Android Studio  
**CLI detection**: `std::io::stdout().is_terminal()`

## Recent Changes

**2026-06-16 - XEmbed Tray Fallback**:
- ✅ GTK → XEmbed runtime fallback for minimal X11 systems
- ✅ Trait-based backend abstraction
- Impact: Works without GTK, +1.5MB binary size

**2026-06-12 - Diesel + MVVM Migration**:
- ✅ DataFusion/Parquet → Diesel/SQLite
- ✅ MVVM architecture with conditional threading
- ✅ Removed 1,323 lines of DataFusion code
- Impact: Standard SQL, clear separation, non-blocking UI

## Future Work

- [ ] Complete Android/CLI ViewModel integration
- [ ] Implement async download tasks
- [ ] Restore status icons
- [ ] iOS/WASM support
