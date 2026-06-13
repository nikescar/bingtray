# BingTray - Project Guide

## 1. Overview

BingTray downloads and displays weekly wallpapers from Bing.com for desktop (Windows, Mac, Linux), CLI, and Android platforms.

## 2. Features

Three entry points: CLI, GUI, TRAY
- **Common**: Download & Set Next Wallpaper, Keep Current Image, Blacklist Current Image, Set Random Favorite
- **CLI**: Menu-driven navigation, no preview, background wallpaper changes
- **GUI (egui)**: Carousel view with image selection, favorite/blacklist controls
- **TRAY**: Same functionality as CLI

## 3. Supported Architectures

- **Platforms**: Windows, macOS, Linux, Android, iOS (planned)
- **Architectures**: aarch64 (arm64), armv7, x86_64

## 4. Repository Structure

```
bingtray/
├── mobile/                          # Main application workspace
│   ├── src/
│   │   ├── db/                      # Database layer (Diesel SQLite)
│   │   │   ├── mod.rs              # Connection & migrations
│   │   │   ├── models.rs           # BingImage, MarketCode, ConfigKv models
│   │   │   └── operations.rs      # CRUD operations
│   │   ├── viewmodel/              # ViewModel layer (MVVM architecture)
│   │   │   ├── mod.rs              # Message types & ViewModel struct
│   │   │   ├── commands.rs         # Command handlers
│   │   │   └── background.rs       # Asupersync background thread
│   │   ├── bingtray.rs             # Main GUI application
│   │   ├── main.rs                 # Desktop entry point
│   │   ├── main_android.rs         # Android entry point
│   │   ├── cli.rs                  # CLI entry point
│   │   ├── tray.rs                 # System tray functionality
│   │   ├── api_bingimage.rs        # Bing API client
│   │   ├── api_setwallpaper.rs     # Wallpaper setting (desktop)
│   │   ├── calc_bingimage.rs       # Image processing logic
│   │   └── schema.rs               # Diesel schema (generated)
│   ├── migrations/                  # Diesel SQL migrations
│   ├── tests/                       # Unit & integration tests
│   │   ├── db_tests.rs
│   │   ├── viewmodel_tests.rs
│   │   └── integration_tests.rs
│   └── Cargo.toml
├── reference/                       # Reference implementations
│   ├── uad-shizuku/                # Diesel ORM examples
│   ├── asupersync/                 # Async runtime examples
│   └── mvvm.rs                     # MVVM pattern reference
├── docs/
│   └── superpowers/
│       ├── specs/                  # Design specifications
│       └── plans/                  # Implementation plans
└── IMPLEMENTATION_SUMMARY.txt      # Recent changes summary
```

## 5. Architecture

### Database Layer (Diesel + SQLite)

**Location**: `mobile/src/db/`

- **Migrations**: Embedded diesel migrations in `mobile/migrations/`
- **Schema**: Auto-generated `schema.rs` from migrations
- **Models**: Queryable and Insertable structs
  - `BingImage`: Image metadata with URL, title, copyright, status
  - `MarketCode`: Market tracking (en-US, ja-JP, etc.)
  - `ConfigKv`: Key-value configuration storage
- **Operations**: CRUD functions for all tables
- **Storage**: SQLite with WAL mode, stored in system config directory

**Database Path**:
- Desktop: `~/.config/bingtray/bingtray.db`
- Android: `/data/data/pe.nikescar.bingtray/files/bingtray.db`

### ViewModel Layer (MVVM Pattern)

**Location**: `mobile/src/viewmodel/`

**Architecture**: Three-layer separation (Database → ViewModel → UI)

**Conditional Threading**:
- **GUI/Android**: Async mode with background thread
  - Uses `std::sync::mpsc` channels for UI communication
  - Asupersync runtime for I/O tasks
  - Non-blocking UI updates via event polling
- **CLI**: Sync mode, direct function calls
  - No background thread overhead
  - Blocking operations acceptable for CLI use case

**Message Types**:
- `ViewModelCommand`: UI → Background thread (DownloadImages, SetWallpaper, etc.)
- `ViewModelEvent`: Background thread → UI (DownloadComplete, ImagesLoaded, etc.)

**Reference Implementation**: See `reference/mvvm.rs` for message-passing pattern

### UI Layer

- **Desktop GUI**: egui-based with Material3 design
- **Android**: Native activity using egui
- **CLI**: Text-based menu system
- **Tray**: System tray icon with context menu

## 6. Entry Points

All entry points are available in a single binary. The mode is determined at runtime:

1. **Desktop GUI App**: `cargo run -- --gui`
   - Tray icon + egui window
   - ViewModel in async mode

2. **Tray Mode**: `cargo run -- --tray` (or default when no terminal)
   - System tray icon only
   - ViewModel in async mode

3. **CLI**: `cargo run` (when run from terminal)
   - Terminal-only interface
   - ViewModel in sync mode (no background thread overhead)

4. **Android**: Built via gradle
   - Native activity
   - ViewModel in async mode

## 7. Local Database and Configuration

### Database

Uses Diesel SQLite ORM saved in platform-specific directories:
- **Desktop**: `~/.config/bingtray/` (Linux), `%APPDATA%\bingtray\` (Windows)
- **Android**: Application private data directory

### Diesel References

Example implementations in `reference/uad-shizuku/mobile/src/`:
- `db.rs` - Connection setup with migrations
- `db_package_cache.rs` - CRUD operations pattern
- Other `db_*.rs` files - Various query patterns

### Schema Migration

Migrations are embedded in the binary and run automatically on first connection.
To create new migrations:
```bash
cd mobile
diesel migration generate <migration_name>
# Edit up.sql and down.sql
# Schema will auto-update on next connection
```

## 8. Async Support

### Asupersync

Used for structured concurrency in I/O tasks:
- Download operations
- Database writes (in future iterations)
- Background image processing

**Reference**: `reference/asupersync/README.md`

**Pattern**: Tasks spawned with `Cx::spawn` in scopes, guaranteed cleanup on cancel

### ViewModel Threading

**Runtime Mode Selection**:
- **GUI/Tray/Android**: `ViewModel::new_async()` - Uses background thread with async runtime
- **CLI**: `ViewModel::new_sync()` - Direct synchronous calls, no threading overhead

Background thread pattern (GUI/Android) from `reference/mvvm.rs`:
- UI thread: egui rendering + event polling
- Background thread: Asupersync runtime processing ViewModelCommands
- Communication: `std::sync::mpsc` channels

CLI mode uses direct function calls with no message passing.

## 9. Testing

### Unit Tests

**Database tests** (`mobile/tests/db_tests.rs`):
- CRUD operations
- Pagination
- SQL injection protection
- Config operations

**ViewModel tests** (`mobile/tests/viewmodel_tests.rs`):
- Message passing
- Command handling
- Sync vs async modes

### Integration Tests

**Entry point tests** (`mobile/tests/integration_tests.rs`):
- Desktop initialization
- CLI sync mode
- Database persistence

Run tests:
```bash
cargo test --manifest-path mobile/Cargo.toml
```

## 10. Build

### Single Binary for All Modes

A single `cargo build` produces a binary that supports all modes (GUI, tray, CLI):

```bash
# Development build (all modes included)
cargo build

# Release build (all modes included)
cargo build --release
```

### Build Profiles

- `dev`: Fast incremental builds (opt-level 0)
- `dev-release`: Optimized without LTO for iteration
- `release`: Full optimization with LTO and symbol stripping

### Runtime Mode Selection

The binary automatically selects the appropriate mode:
- **Terminal detected**: CLI mode (sync ViewModel)
- **`--gui` flag**: GUI window mode (async ViewModel)
- **`--tray` flag**: Tray mode (async ViewModel)
- **Double-click / no terminal**: Tray mode (async ViewModel)

## 11. Dependencies

### Core

- **egui** + **eframe**: Cross-platform GUI
- **diesel** + **diesel_migrations**: SQLite ORM
- **asupersync**: Structured concurrency runtime
- **tokio**: Async runtime (workspace dependency)
- **anyhow**: Error handling

### Platform-Specific

- **Desktop**: tray-icon, wallpaper, trash
- **Android**: ndk-context, jni, android-activity
- **WASM**: wasm-bindgen (planned)

## 12. Recent Changes (2026-06-12)

### Diesel SQLite + MVVM Implementation

**Completed**:
- ✅ Migrated from DataFusion/Parquet to Diesel/SQLite
- ✅ Implemented MVVM architecture with ViewModel layer
- ✅ Conditional threading (async for GUI/Android, sync for CLI)
- ✅ Comprehensive unit and integration tests
- ✅ Removed 1,323 lines of DataFusion code

**Impact**:
- Database operations now use standard SQL patterns
- Clear separation between UI and business logic
- Non-blocking UI during downloads
- Easier to test and maintain

**Documentation**:
- Design spec: `docs/superpowers/specs/2026-06-12-diesel-mvvm-design.md`
- Implementation plan: `docs/superpowers/plans/2026-06-12-diesel-mvvm-implementation.md`
- Summary: `IMPLEMENTATION_SUMMARY.txt`

### Known Limitations

- Status icons temporarily disabled (migrating to ViewModel)
- Android/CLI ViewModel integration pending
- Async download tasks still use placeholder stubs

## 13. Development Workflow

1. **Start development**: Edit code in `mobile/src/`
2. **Run locally**:
   - GUI mode: `cargo run -- --gui`
   - Tray mode: `cargo run -- --tray`
   - CLI mode: `cargo run` (from terminal)
3. **Test**: `cargo test --manifest-path mobile/Cargo.toml`
4. **Build release**: `cargo build --release`
5. **Format**: `cargo fmt`
6. **Lint**: `cargo clippy`

## 14. Platform-Specific Notes

### Android

- Entry point: `mobile/src/main_android.rs`
- Build: Via gradle in Android Studio
- ViewModel: Uses async mode with background thread

### CLI

- Entry point: `mobile/src/cli.rs`
- Detection: Terminal detected via `std::io::stdout().is_terminal()`
- ViewModel: Uses sync mode (no background thread)

### Desktop

- Entry point: `mobile/src/main.rs`
- Features: Tray icon, GUI window
- ViewModel: Uses async mode with background thread

## 15. Future Work

- [ ] Complete Android ViewModel integration
- [ ] Complete CLI ViewModel integration
- [ ] Implement async download tasks with Asupersync
- [ ] Restore status icon functionality via ViewModel
- [ ] iOS support
- [ ] WASM browser support
