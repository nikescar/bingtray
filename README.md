# BingTray - Bing Wallpaper Manager

A Rust application that automatically downloads and sets Bing daily wallpapers as your desktop background.

## Features

- Automatically downloads Bing daily wallpapers from various market regions
- Supports multiple Linux desktop environments (GNOME, KDE, XFCE, MATE, etc.)
- Blacklist unwanted wallpapers
- Keep favorite wallpapers
- Automatic wallpaper rotation
- Command-line interface

## Requirements

For GUI functionality on Linux, you may need to install:

```bash
# Ubuntu/Debian
sudo apt-get install libgtk-3-dev libpango1.0-dev libgdk-pixbuf-2.0-dev libatk1.0-dev libglib2.0-dev

# Or for other distros, install equivalent GTK development packages
```

## Installation

1. Clone this repository
2. Install Rust if you haven't already: https://rustup.rs/
3. Build the project:

```bash
cargo build --release
```

## Usage

### Interactive Mode

Run the main application:

```bash
cargo run
```

This will start an interactive command-line interface where you can:
1. Set next wallpaper
2. Keep current wallpaper (moves to favorites)
3. Blacklist current wallpaper
4. Download more wallpapers
5. Exit

### Test Mode

Run the test binary to check functionality:

```bash
cargo run --bin test
```

## Configuration

The application creates a configuration directory at `~/.config/bingtray/` with:

- `marketcodes.conf` - List of Bing market codes and last visit timestamps
- `blacklist.conf` - Hashes of blacklisted wallpapers
- `unprocessed/` - Downloaded wallpapers ready to be used
- `keepfavorite/` - Wallpapers you've marked as favorites

## Supported Desktop Environments

- GNOME / Unity / Cinnamon (via gsettings)
- MATE (via gsettings/mateconftool-2)
- XFCE4 (via xfconf-query)
- LXDE (via pcmanfm)
- Various window managers (via fbsetbg, bsetbg, etc.)

## How It Works

1. Downloads wallpapers from Bing's API using various market codes (en-US, ja-JP, etc.)
2. Stores images with format: `{title}.{hash}.jpg`
3. Detects your desktop environment and uses appropriate commands to set wallpaper
4. Manages a rotation system and respects your preferences (blacklist/favorites)

## Building for Different Targets

The project is designed to work on Linux systems. For other platforms, wallpaper setting functionality may need to be adapted.
