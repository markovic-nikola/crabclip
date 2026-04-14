# CrabClip — Detailed Build Spec for Claude Code

## Project Summary

Build **CrabClip**, a Linux clipboard manager written in Rust that runs silently in the background with a system tray icon. It monitors the clipboard, stores a history of text and image entries, and lets the user browse and re-copy past entries via a popup UI triggered by a tray click or `Ctrl+Alt+C`.

Target platform: **Linux Mint (X11)**

---

## App Name & Paths

| Item | Value |
|---|---|
| Binary name | `crabclip` |
| Config dir | `~/.config/crabclip/` |
| History file | `~/.config/crabclip/history.json` |
| Settings file | `~/.config/crabclip/settings.json` |
| Tray icon | `assets/icon.png` (32x32, embedded at compile time) |
| Autostart file | `~/.config/autostart/crabclip.desktop` |

---

## System Dependencies (must document in README)

```bash
sudo apt install libgtk-3-dev libxdo-dev libglib2.0-dev pkg-config
```

---

## Cargo.toml Dependencies

```toml
[package]
name = "crabclip"
version = "0.1.0"
edition = "2021"

[dependencies]
arboard        = { version = "3", features = ["image-data"] }
tray-icon      = "0.18"
global-hotkey  = "0.6"
egui           = "0.31"
eframe         = "0.31"
image          = "0.25"
serde          = { version = "1", features = ["derive"] }
serde_json     = "1"
dirs           = "5"
chrono         = { version = "0.4", features = ["serde"] }
uuid           = { version = "1", features = ["v4"] }

[build-dependencies]
# none needed
```

---

## File Structure

```
crabclip/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── build.rs                    # optional: embed icon
├── assets/
│   └── icon.png                # 32x32 tray icon, crab emoji style
├── src/
│   ├── main.rs                 # entry point
│   ├── watcher.rs              # clipboard polling thread
│   ├── history.rs              # history data structure + persistence
│   ├── tray.rs                 # tray icon + context menu
│   ├── hotkey.rs               # Ctrl+Alt+C global hotkey
│   ├── ui.rs                   # egui popup window
│   └── config.rs               # settings struct + load/save
└── autostart/
    └── crabclip.desktop        # autostart entry for Linux Mint
```

---

## Data Model (`history.rs`)

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// The content of a single clipboard entry.
#[derive(Clone, Serialize, Deserialize)]
pub enum ClipContent {
    Text(String),
    /// PNG-encoded image bytes stored as base64 in JSON
    Image(Vec<u8>),
}

/// One entry in the clipboard history.
#[derive(Clone, Serialize, Deserialize)]
pub struct ClipEntry {
    pub id: String,                  // uuid v4
    pub content: ClipContent,
    pub timestamp: DateTime<Utc>,
    pub pinned: bool,
}

/// The full history store.
pub struct History {
    pub entries: std::collections::VecDeque<ClipEntry>,
    pub max_size: usize,             // default 100
}

impl History {
    pub fn new(max_size: usize) -> Self { ... }
    pub fn push(&mut self, content: ClipContent) { ... } // dedup + trim
    pub fn load_from_disk(path: &std::path::Path) -> Self { ... }
    pub fn save_to_disk(&self, path: &std::path::Path) -> anyhow::Result<()> { ... }
    pub fn clear(&mut self) { ... }
    pub fn remove(&mut self, id: &str) { ... }
    pub fn toggle_pin(&mut self, id: &str) { ... }
}
```

**Deduplication rule:** Before pushing, check if the last non-pinned entry has identical content. If so, skip. Do a full-scan dedup for text (case-sensitive exact match). For images, compare byte length + first 256 bytes as a fast fingerprint.

---

## Config (`config.rs`)

```rust
#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub max_history: usize,          // default: 100
    pub poll_interval_ms: u64,       // default: 500
    pub launch_at_login: bool,       // default: true
    pub show_images: bool,           // default: true
    pub hotkey: String,              // default: "ctrl+alt+c"
}

impl Default for Settings { ... }

impl Settings {
    pub fn load() -> Self { ... }  // reads ~/.config/crabclip/settings.json
    pub fn save(&self) { ... }
}
```

---

## Clipboard Watcher (`watcher.rs`)

- Runs in a **dedicated thread** (spawn with `std::thread::spawn`).
- Uses `arboard::Clipboard` to poll.
- Every `poll_interval_ms`, check:
  1. Try `clipboard.get_text()` — if changed from last value, push `ClipContent::Text`.
  2. Try `clipboard.get_image()` — if changed (use byte-length + partial hash), encode to PNG using the `image` crate, push `ClipContent::Image(png_bytes)`.
- Communicate new entries back to the main thread via `std::sync::mpsc::channel`.
- Keep track of last seen text and last seen image hash to detect changes.
- On any clipboard error (e.g. clipboard temporarily unavailable), log and continue — do NOT panic.

```rust
pub fn start_watcher(
    tx: std::sync::mpsc::Sender<ClipEntry>,
    settings: Arc<Settings>,
) -> std::thread::JoinHandle<()>
```

---

## Tray Icon (`tray.rs`)

Use the `tray-icon` crate with a GTK event loop.

### Behavior
- On **left-click**: toggle the popup window open/closed.
- On **right-click**: show a context menu with:
  - Last 5 text entries (truncated to 40 chars each) — click to re-copy
  - Separator
  - `Open History` — opens the full popup
  - `Clear History` — with confirmation
  - `Settings` — opens settings panel (inside the same egui window, different tab)
  - Separator
  - `Quit`

### Icon
- Embed `assets/icon.png` at compile time using `include_bytes!`.
- Use a 32x32 PNG with a crab emoji-style icon (orange crab on transparent background).

---

## Global Hotkey (`hotkey.rs`)

- Use `global-hotkey` crate.
- Register `Ctrl+Alt+C` on startup.
- On trigger: send a message to the main thread to toggle the popup window.
- If registration fails (key already taken by another app), log a warning and continue without the hotkey — do not crash.

```rust
pub fn register_hotkey(tx: std::sync::mpsc::Sender<AppEvent>) -> Option<GlobalHotKeyManager>
```

---

## Popup UI (`ui.rs`)

Use `eframe` + `egui`. The window should:

### Window properties
- Title: `CrabClip 🦀`
- Size: `420 × 600` px, resizable
- Not shown in taskbar (`NativeOptions::taskbar = false`)
- Centered on screen on first open
- Remembers position between sessions

### Layout — two tabs

#### Tab 1: History
- **Search bar** at the top (filters entries live by text content)
- **Scrollable list** of entries, newest first
- Pinned entries shown at the top with a 📌 icon
- Each entry shows:
  - **Text entry**: first 2 lines of text, timestamp (e.g. "2 min ago"), copy button, pin button, delete button
  - **Image entry**: thumbnail (max 120×80 px), dimensions, file size, timestamp, copy button, pin button, delete button
- Clicking anywhere on an entry (not a button) copies it to clipboard and closes the window
- Hover highlight on entries
- "Clear All" button at the bottom (clears unpinned entries only)

#### Tab 2: Settings
- Max history size (number input, 10–500)
- Poll interval (slider, 200ms–2000ms)
- Toggle: Show images in history
- Toggle: Launch at login (writes/removes the `.desktop` autostart file)
- Hotkey display (static label for now: `Ctrl+Alt+C`)
- "Save" button

### Window behavior
- Opening the window: call `egui_window.set_visible(true)` and bring to front
- Closing: hide (don't destroy), so re-opening is instant
- `Escape` key closes the window

---

## Autostart (`autostart/crabclip.desktop`)

```ini
[Desktop Entry]
Type=Application
Name=CrabClip
GenericName=Clipboard Manager
Comment=Clipboard history manager with tray icon
Exec=/usr/local/bin/crabclip
Icon=crabclip
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
Categories=Utility;
```

The app should **automatically install this file** to `~/.config/autostart/crabclip.desktop` on first run if `settings.launch_at_login == true`. If the user toggles "Launch at login" to off in settings, delete the file.

---

## Main Entry Point (`main.rs`)

Orchestrates everything:

```
1. Load Settings
2. Load History from disk (or create empty)
3. Wrap history in Arc<Mutex<History>>
4. Create mpsc channels:
   - clip_tx / clip_rx  (watcher → main)
   - event_tx / event_rx  (hotkey + tray → main)
5. Start clipboard watcher thread (clip_tx)
6. Register global hotkey (event_tx)
7. Set up tray icon
8. Install autostart .desktop file if needed
9. Start eframe (egui) app — this takes over the main thread
   - Inside the eframe update loop:
     a. Drain clip_rx → push new entries to history
     b. Drain event_rx → handle ToggleWindow / TrayClick events
     c. Save history to disk every 30 seconds (debounced)
     d. Render the UI if window is visible
```

**On exit (Quit menu item):**
1. Save history to disk immediately
2. Save settings
3. Exit process

---

## Error Handling Philosophy

- Never `unwrap()` on clipboard operations — clipboard can be temporarily locked by other apps.
- Never `unwrap()` on file I/O — config dir might not exist yet (create it with `fs::create_dir_all`).
- Log errors to stderr (no log file needed for MVP).
- The app must never crash due to a clipboard or I/O error.

---

## Build & Run Instructions (for README)

```bash
# Install system deps
sudo apt install libgtk-3-dev libxdo-dev libglib2.0-dev pkg-config

# Clone and build
git clone https://github.com/you/crabclip
cd crabclip
cargo build --release

# Run
./target/release/crabclip

# Install to system
sudo cp target/release/crabclip /usr/local/bin/crabclip
```

---

## Implementation Order for Claude Code

Please implement in this exact order, making sure each step compiles and works before moving to the next:

1. **Scaffold** — `Cargo.toml`, file stubs, `Settings` struct, `History` struct with load/save
2. **Watcher** — clipboard polling, text + image detection, mpsc channel, print to stdout
3. **Tray icon** — show icon, "Quit" works, left-click prints to stdout
4. **Hotkey** — `Ctrl+Alt+C` prints "hotkey triggered" to stdout
5. **egui window** — opens on hotkey/tray click, shows placeholder text, closes on Escape
6. **History UI** — full scrollable list with text + image thumbnails, click to re-copy
7. **Settings tab** — all settings wired up, autostart file written/removed
8. **Polish** — search bar, pin, delete, "Clear All", timestamps, window position memory

---

## Notes & Constraints

- Target **X11 only** for now (Linux Mint default). No Wayland-specific code needed.
- GTK3 is already present on Linux Mint — safe to depend on it.
- Images in history should be stored as **PNG bytes** in the JSON history file encoded as **base64**.
- The history file can grow large with images — cap individual image entries at **2 MB** (skip larger clipboard images).
- The popup window should open **near the tray icon** if possible, otherwise centered.
- Use `egui`'s built-in dark theme as default.
- All user-visible strings should be in English.
