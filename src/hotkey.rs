use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::GlobalHotKeyManager;

pub fn register_hotkey() -> Option<(GlobalHotKeyManager, HotKey)> {
    let manager = match GlobalHotKeyManager::new() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to create hotkey manager: {e}");
            return None;
        }
    };

    let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyC);

    match manager.register(hotkey) {
        Ok(()) => {
            eprintln!("Registered global hotkey: Ctrl+Alt+C");
            Some((manager, hotkey))
        }
        Err(e) => {
            eprintln!("Failed to register Ctrl+Alt+C (may be taken by another app): {e}");
            None
        }
    }
}
