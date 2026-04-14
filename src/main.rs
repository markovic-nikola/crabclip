mod config;
mod history;
mod hotkey;
mod tray;
mod watcher;

use config::{history_path, manage_autostart, Settings};
use history::History;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn main() {
    // 1. Load settings and history
    let settings = Settings::load();
    let hist_path = history_path();
    let history = History::load(&hist_path, settings.max_history);
    let history = Arc::new(Mutex::new(history));
    let poll_interval = Duration::from_millis(settings.poll_interval_ms);
    let settings = Arc::new(Mutex::new(settings));

    // 2. Clipboard watcher channel + suppress flag
    let (clip_tx, clip_rx) = std::sync::mpsc::channel();
    let suppress: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    // 3. Start clipboard watcher thread
    let _watcher = watcher::start_watcher(
        clip_tx,
        poll_interval,
        Arc::clone(&suppress),
        Arc::clone(&settings),
    );

    // 4. Handle autostart
    if settings.lock().unwrap().launch_at_login {
        manage_autostart(true);
    }

    // 5. Run tray icon on main thread (GTK main loop — blocks forever)
    //    Hotkey is registered inside run_tray after gtk::init()
    tray::run_tray(history, settings, clip_rx, suppress, hist_path);
}
