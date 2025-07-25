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

- **Windows**: Via winapi
- **Mac OSX**: Via AppleScript
- **GNOME/Unity/Cinnamon**: Via gsettings
- **MATE**: Via gsettings
- **XFCE4**: Via xfconf-query
- **LXDE**: Via pcmanfm
- **Fluxbox/JWM/Openbox/AfterStep**: Via fbsetbg
- **IceWM**: Via icewmbg
- **Blackbox**: Via bsetbg

<details markdown>
<summary> Todos </summary>

## Todos

* android? https://stackoverflow.com/a/46960602
* remove windws i686 build due to virustotal detected - https://www.virustotal.com/gui/file-analysis/MTVlM2Q3MzFmMzNlMWM4MGVjNmNhNTNmM2Q3MjZjMzE6MTc1MzI1NzA0OA==
* add remove config menu
* add bing image history
* add license on cli and gui and link
* add history access of wallpapers from https://raw.githubusercontent.com/v5tech/bing-wallpaper/refs/heads/main/bing-wallpaper.md
https://github.com/niumoo/bing-wallpaper/tree/main
* version code
* check update

</details>