use crate::config::Settings;
use crate::history::{ClipContent, History};
use arboard::SetExtLinux;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tray_icon::menu::{
    CheckMenuItem, Icon as MenuIcon, IconMenuItem, Menu, MenuEvent, MenuId, MenuItem,
    PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, TrayIconBuilder};

use crate::history::ClipEntry;

const MAX_HISTORY_OPTIONS: [usize; 5] = [10, 20, 30, 40, 50];

/// Maps menu item IDs to clipboard entry IDs for click-to-copy.
struct MenuState {
    entry_map: HashMap<MenuId, String>,
    menu: Menu,
    clear_id: MenuId,
    quit_id: MenuId,
    /// Number of dynamic history items currently in the menu (before the separator).
    history_count: usize,
    // Preferences
    launch_at_login: CheckMenuItem,
    show_images: CheckMenuItem,
    max_history_items: Vec<(CheckMenuItem, usize)>,
}

pub fn run_tray(
    history: Arc<Mutex<History>>,
    settings: Arc<Mutex<Settings>>,
    clip_rx: mpsc::Receiver<ClipEntry>,
    suppress: Arc<Mutex<Option<String>>>,
    history_path: PathBuf,
) {
    gtk::init().expect("Failed to init GTK");

    // Register global hotkey after gtk::init() so X11 connection is ready
    let hotkey_result = crate::hotkey::register_hotkey();
    let hotkey_id = hotkey_result.as_ref().map(|(_, hk)| hk.id());
    // Must stay alive for the hotkey to remain registered
    let _hotkey_manager = hotkey_result;

    // Read current settings for initial menu state
    let current_settings = settings.lock().unwrap().clone();

    // Build the menu — history entries go directly at the top,
    // followed by a separator, Clear History, Preferences, and Quit.
    let clear_item = MenuItem::new("Clear History", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let clear_id = clear_item.id().clone();
    let quit_id = quit_item.id().clone();

    // Build Preferences submenu
    let launch_at_login = CheckMenuItem::new("Launch at Login", true, current_settings.launch_at_login, None);
    let show_images = CheckMenuItem::new("Show Images", true, current_settings.show_images, None);

    let max_history_sub = Submenu::new("Max History", true);
    let mut max_history_items = Vec::new();
    for &size in &MAX_HISTORY_OPTIONS {
        let checked = current_settings.max_history == size;
        let item = CheckMenuItem::new(&size.to_string(), true, checked, None);
        let _ = max_history_sub.append(&item);
        max_history_items.push((item, size));
    }

    let prefs_sub = Submenu::new("Preferences", true);
    let _ = prefs_sub.append(&launch_at_login);
    let _ = prefs_sub.append(&show_images);
    let _ = prefs_sub.append(&PredefinedMenuItem::separator());
    let _ = prefs_sub.append(&max_history_sub);

    let menu = Menu::new();
    // Placeholder for empty state
    let empty = MenuItem::new("(empty)", false, None);
    let _ = menu.append(&empty);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&clear_item);
    let _ = menu.append(&prefs_sub);
    let _ = menu.append(&quit_item);

    // Load and set tray icon
    let icon_bytes = include_bytes!("../assets/icon.png");
    let icon_image = image::load_from_memory(icon_bytes)
        .expect("Failed to load embedded icon")
        .to_rgba8();
    let (w, h) = icon_image.dimensions();
    let icon = Icon::from_rgba(icon_image.into_raw(), w, h).expect("Failed to create tray icon");

    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu.clone()))
        .with_tooltip("CrabClip")
        .with_icon(icon)
        .build()
        .expect("Failed to build tray icon");

    let mut state = MenuState {
        entry_map: HashMap::new(),
        menu,
        clear_id,
        quit_id,
        history_count: 1, // the "(empty)" placeholder
        launch_at_login,
        show_images,
        max_history_items,
    };

    // Populate menu from existing history
    {
                    let si = state.show_images.is_checked();
                    rebuild_history_menu(&mut state, &history, si);
                }

    let mut last_save = Instant::now();
    let mut dirty = false;

    // Use glib timeout to poll channels periodically
    let menu_rx = MenuEvent::receiver().clone();
    let hotkey_rx = global_hotkey::GlobalHotKeyEvent::receiver().clone();

    gtk::glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        // Poll clipboard watcher
        while let Ok(entry) = clip_rx.try_recv() {
            let mut hist = history.lock().unwrap();
            hist.push(entry.content);
            dirty = true;
            drop(hist);
            {
                    let si = state.show_images.is_checked();
                    rebuild_history_menu(&mut state, &history, si);
                }
        }

        // Poll hotkey events — Ctrl+Alt+C copies the previous clipboard entry
        if let Some(hk_id) = hotkey_id {
            while let Ok(event) = hotkey_rx.try_recv() {
                if event.id() == hk_id {
                    let hist = history.lock().unwrap();
                    // Skip the first entry (current clipboard) and copy the second one
                    if let Some(entry) = hist.entries.get(1) {
                        let entry_id = entry.id.clone();
                        drop(hist);
                        copy_entry_to_clipboard(&history, &entry_id, &suppress);
                        history.lock().unwrap().move_to_top(&entry_id);
                        dirty = true;
                        {
                    let si = state.show_images.is_checked();
                    rebuild_history_menu(&mut state, &history, si);
                }
                    }
                }
            }
        }

        // Poll menu events
        while let Ok(event) = menu_rx.try_recv() {
            let id: &MenuId = event.id();

            if *id == state.quit_id {
                let hist = history.lock().unwrap();
                let _ = hist.save(&history_path);
                let _ = settings.lock().unwrap().save();
                std::process::exit(0);
            } else if *id == state.clear_id {
                history.lock().unwrap().clear_unpinned();
                dirty = true;
                {
                    let si = state.show_images.is_checked();
                    rebuild_history_menu(&mut state, &history, si);
                }
            } else if *id == *state.launch_at_login.id() {
                let checked = state.launch_at_login.is_checked();
                let mut s = settings.lock().unwrap();
                s.launch_at_login = checked;
                let _ = s.save();
                crate::config::manage_autostart(checked);
            } else if *id == *state.show_images.id() {
                let checked = state.show_images.is_checked();
                let mut s = settings.lock().unwrap();
                s.show_images = checked;
                let _ = s.save();
                drop(s);
                rebuild_history_menu(&mut state, &history, checked);
            } else if let Some((new_size, items)) = state
                .max_history_items
                .iter()
                .find(|(item, _)| *id == *item.id())
                .map(|(_, size)| (*size, &state.max_history_items))
            {
                // Radio behavior: uncheck all, check the selected one
                for (item, size) in items {
                    item.set_checked(*size == new_size);
                }
                let mut s = settings.lock().unwrap();
                s.max_history = new_size;
                let _ = s.save();
                drop(s);
                let mut hist = history.lock().unwrap();
                hist.max_size = new_size;
                hist.trim();
                drop(hist);
                dirty = true;
                {
                    let si = state.show_images.is_checked();
                    rebuild_history_menu(&mut state, &history, si);
                }
            } else if let Some(entry_id) = state.entry_map.get(id).cloned() {
                // Copy clicked entry to clipboard and move it to the top
                copy_entry_to_clipboard(&history, &entry_id, &suppress);
                history.lock().unwrap().move_to_top(&entry_id);
                dirty = true;
                {
                    let si = state.show_images.is_checked();
                    rebuild_history_menu(&mut state, &history, si);
                }
            }
        }

        // Auto-save every 30 seconds
        if dirty && last_save.elapsed() > std::time::Duration::from_secs(30) {
            let hist = history.lock().unwrap();
            let _ = hist.save(&history_path);
            last_save = Instant::now();
            dirty = false;
        }

        gtk::glib::ControlFlow::Continue
    });

    gtk::main();
}

fn rebuild_history_menu(state: &mut MenuState, history: &Arc<Mutex<History>>, show_images: bool) {
    // Remove existing history items from the top of the menu
    for _ in 0..state.history_count {
        state.menu.remove_at(0);
    }
    state.entry_map.clear();

    let hist = history.lock().unwrap();

    let is_visible = |e: &&ClipEntry| show_images || matches!(e.content, ClipContent::Text(_));

    if hist.entries.is_empty() || !hist.entries.iter().any(|e| is_visible(&&e)) {
        let empty = MenuItem::new("(empty)", false, None);
        let _ = state.menu.insert(&empty, 0);
        state.history_count = 1;
        return;
    }

    // Insert in reverse order at position 0 so the final order is:
    // pinned entries, separator (if pinned exist), unpinned entries

    let unpinned: Vec<_> = hist.entries.iter().filter(|e| !e.pinned && is_visible(e)).collect();
    let pinned: Vec<_> = hist.entries.iter().filter(|e| e.pinned && is_visible(e)).collect();

    let mut count = 0;

    // Insert unpinned (reversed so newest ends up at the top)
    for entry in unpinned.iter().rev() {
        insert_entry_item(&state.menu, &mut state.entry_map, entry, false);
        count += 1;
    }

    // Insert separator between pinned and unpinned
    if !pinned.is_empty() && !unpinned.is_empty() {
        let _ = state.menu.insert(&PredefinedMenuItem::separator(), 0);
        count += 1;
    }

    // Insert pinned (reversed so first pinned ends up at the top)
    for entry in pinned.iter().rev() {
        insert_entry_item(&state.menu, &mut state.entry_map, entry, true);
        count += 1;
    }

    state.history_count = count;
}

fn format_entry_label(entry: &ClipEntry, pinned: bool) -> String {
    let prefix = if pinned { "[PIN] " } else { "" };
    match &entry.content {
        ClipContent::Text(text) => {
            let preview: String = text
                .chars()
                .take(40)
                .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
                .collect();
            let suffix = if text.len() > 40 { "..." } else { "" };
            format!("{prefix}{preview}{suffix}")
        }
        ClipContent::Image { width, height, .. } => {
            format!("{prefix}[Image {width}x{height}]")
        }
    }
}

/// Generate a menu icon thumbnail from a base64-encoded PNG.
fn make_thumbnail_icon(png_base64: &str) -> Option<MenuIcon> {
    let png_bytes = STANDARD.decode(png_base64).ok()?;
    let img = image::load_from_memory(&png_bytes).ok()?;
    let thumb = img.thumbnail(16, 16);
    let rgba = thumb.to_rgba8();
    let (w, h) = rgba.dimensions();
    MenuIcon::from_rgba(rgba.into_raw(), w, h).ok()
}


/// Insert a history entry into the menu at position 0.
/// Image entries use `IconMenuItem` with a real thumbnail;
/// text entries use plain `MenuItem`.
fn insert_entry_item(
    menu: &Menu,
    entry_map: &mut HashMap<MenuId, String>,
    entry: &ClipEntry,
    pinned: bool,
) {
    let label = format_entry_label(entry, pinned);
    match &entry.content {
        ClipContent::Image { png_base64, .. } => {
            let icon = make_thumbnail_icon(png_base64);
            let item = IconMenuItem::new(&label, true, icon, None);
            entry_map.insert(item.id().clone(), entry.id.clone());
            let _ = menu.insert(&item, 0);
        }
        _ => {
            let item = MenuItem::new(&label, true, None);
            entry_map.insert(item.id().clone(), entry.id.clone());
            let _ = menu.insert(&item, 0);
        }
    }
}

fn copy_entry_to_clipboard(
    history: &Arc<Mutex<History>>,
    entry_id: &str,
    suppress: &Arc<Mutex<Option<String>>>,
) {
    let hist = history.lock().unwrap();
    let Some(entry) = hist.entries.iter().find(|e| e.id == entry_id) else {
        return;
    };
    let content = entry.content.clone();
    drop(hist);

    let suppress = Arc::clone(suppress);

    // Spawn a thread so the Clipboard stays alive to serve X11 selection requests.
    // The .wait() call blocks until another app overwrites the clipboard.
    std::thread::spawn(move || {
        let Ok(mut clip) = arboard::Clipboard::new() else {
            return;
        };
        match &content {
            ClipContent::Text(text) => {
                if let Ok(mut s) = suppress.lock() {
                    *s = Some(text.clone());
                }
                let _ = clip.set().wait().text(text.clone());
            }
            ClipContent::Image {
                png_base64,
                width,
                height,
            } => {
                if let Ok(png_bytes) = STANDARD.decode(png_base64) {
                    if let Ok(img) = image::load_from_memory(&png_bytes) {
                        let rgba = img.to_rgba8();
                        let img_data = arboard::ImageData {
                            width: *width as usize,
                            height: *height as usize,
                            bytes: Cow::Owned(rgba.into_raw()),
                        };
                        let _ = clip.set().wait().image(img_data);
                    }
                }
            }
        }
    });
}
