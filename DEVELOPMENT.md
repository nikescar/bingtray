# development


- bingtray
  - bingtray : core application
    - desktop    : desktop cli, gui application
    - mobile      : android, ios application
    - web         : firefox, chrome, safari extension(opera, vivaldi, edge, brave, arc, torbrowser, zenbrowser, waterfox)

# troubleshoot

## pkg-config problems
```
cargo:warning=Could not run `PKG_CONFIG_ALLOW_SYSTEM_CFLAGS=1 pkg-config --libs --cflags gio-2.0 'gio-2.0 >= 2.56'`
  The pkg-config command could not be found.
```
solutions
```bash debian-bookworm
$ sudo apt install musl-tools libgtk-3-dev libxdo-dev libappindicator3-dev libglib2.0-dev libgdk-pixbuf-2.0-dev libwayland-dev libcairo2-dev libpixman-1-dev libpango1.0-dev libxdo-dev librust-glib-sys-dev librust-gio-sys-dev librust-gobject-sys-dev librust-gdk-sys-dev libwebkit2gtk-4.1-dev librust-gdk-pixbuf-sys-dev librust-cairo-sys-rs-dev librust-pango-sys-dev librust-atk-sys-dev libgdk-pixbuf2.0-dev libatk1.0-dev musl-dev
```
```bash debian-trixie
$ sudo apt install musl-tools libgtk-3-dev libxdo-dev libappindicator3-dev libglib2.0-dev libgdk-pixbuf-2.0-dev libwayland-dev libcairo2-dev libpixman-1-dev libpango1.0-dev libxdo-dev librust-glib-sys-dev librust-gio-sys-dev librust-gobject-sys-dev librust-gdk-sys-0.18-dev libwebkit2gtk-4.1-dev librust-gdk-pixbuf-sys-dev librust-cairo-sys-rs-dev librust-pango-sys-dev librust-atk-sys-0.18-dev libgdk-pixbuf-2.0-dev libatk1.0-dev musl-dev
```

