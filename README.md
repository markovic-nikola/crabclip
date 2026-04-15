<p align="center">
  <img src="assets/logo.png" alt="CrabClip" width="128" />
</p>

<h1 align="center">CrabClip</h1>

<p align="center">
  A lightweight Linux clipboard manager with system tray integration, image support, and clipboard history — built in Rust.
</p>

---

CrabClip runs silently in the background with a system tray icon. It monitors the clipboard for text and image copies, stores a configurable history, and lets you re-copy past entries from the tray menu.

## Features

- Text and image clipboard history with configurable size (10–50 entries)
- Image thumbnails in the tray menu
- Pin entries to keep them permanently
- Global hotkey (`Ctrl+Alt+C`) to open clipboard history at cursor
- Preferences menu for Launch at Login, Show Images, and Max History
- Lightweight — single binary with minimal system dependencies

## Requirements

- Debian/Ubuntu-based Linux distribution (e.g., Ubuntu, Linux Mint, Pop!_OS)
- X11 display server (Wayland is not supported)
- GTK 3

## Installation

Download the latest `.deb` from [Releases](../../releases) and install:

```bash
sudo dpkg -i crabclip_*.deb
```

### From Source

```bash
sudo apt install libgtk-3-dev libayatana-appindicator3-dev libxdo-dev pkg-config
git clone https://github.com/your-username/crabclip.git
cd crabclip
cargo build --release
```

The binary will be at `target/release/crabclip`.

## Usage

```bash
./target/release/crabclip
```

CrabClip will appear in your system tray. Click the icon to browse clipboard history, or press `Ctrl+Alt+C` to open the history menu at your cursor.

## Configuration

Settings are stored at `~/.config/crabclip/settings.json` and can be changed from the **Preferences** submenu in the tray:

| Setting | Default | Description |
|---------|---------|-------------|
| Max History | 20 | Number of clipboard entries to keep |
| Show Images | On | Capture and display image clipboard entries |
| Launch at Login | On | Auto-start via XDG autostart |

## Built With

- [Rust](https://www.rust-lang.org/)
- [GTK 3](https://www.gtk.org/) / [libappindicator](https://launchpad.net/libappindicator) — system tray
- [arboard](https://github.com/1Password/arboard) — clipboard access
- [tray-icon](https://github.com/tauri-apps/tray-icon) — tray icon management

## License

MIT
