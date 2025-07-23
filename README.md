# BingTray - Bing Wallpaper Manager

A cross-platform wallpaper manager that downloads and manages Bing's weekly wallpapers. bing wallpapers are updated weekly in 43 global stores. each store has different sets of 8 images many of them are shared beween markets. We are visiting random market wallpaper list and download new images if we dont have them. If you exhausted all market images, you have wait for a week to get new images. 

![bingtray-gui](./imgs/bingtray-gui.gif "Bingtray-gui")

## Usage

```bash
# Run the interactive CLI menu:
$ bingcli
# Run the GUI (currently falls back to CLI mode):
$ bingtray-gui
# Run cli application from gui binary
$ bingtray-gui --cli
```

## Configuration

The application creates configuration files in:
- Linux: `~/.config/bingtray/`
- MAC OSX: ``
- Windows: `C:\Users\{Username}\Appdata\Roaming\bingtray`

### Directory structure:
- `unprocessed/`: Downloaded wallpapers waiting to be used
- `keepfavorite/`: Wallpapers you've marked as favorites
- `blacklist.conf`: Hash list of blacklisted images
- `marketcodes.conf`: Market codes and last download timestamps

## Usage

After starting the application, you'll see a tray icon with the following options:

- **Next Market wallpaper**: Set the next available wallpaper from the unprocessed folder
- **Keep "[title]"**: Move the current wallpaper to favorites and set the next one
- **Blacklist "[title]"**: Remove the current wallpaper and add it to blacklist
- **Exit**: Close the application

## Supported Desktop Environments

- **Windows**: Uses PowerShell via `Command::new` (default) or via `powershell_script` crate (optional feature)
- **Mac OSX**: Via AppleScript
- **GNOME/Unity/Cinnamon**: Via gsettings
- **MATE**: Via gsettings
- **XFCE4**: Via xfconf-query
- **LXDE**: Via pcmanfm
- **Fluxbox/JWM/Openbox/AfterStep**: Via fbsetbg
- **IceWM**: Via icewmbg
- **Blackbox**: Via bsetbg

### Windows PowerShell Options

By default, the Windows implementation uses `Command::new` to call PowerShell directly. If you prefer to use the `powershell_script` crate (which some antivirus software may flag), you can enable it as a feature:

```bash
# Build with powershell_script crate feature
cargo build --features powershell_script
```

## Todos

* android? https://stackoverflow.com/a/46960602