use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub max_history: usize,
    pub poll_interval_ms: u64,
    pub launch_at_login: bool,
    pub show_images: bool,
    pub hotkey: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_history: 20,
            poll_interval_ms: 500,
            launch_at_login: true,
            show_images: true,
            hotkey: "ctrl+alt+c".to_string(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let path = settings_path();
        match fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = settings_path();
        fs::create_dir_all(config_dir())?;
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .expect("No config directory found")
        .join("crabclip")
}

pub fn history_path() -> PathBuf {
    config_dir().join("history.json")
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn manage_autostart(enable: bool) {
    let Some(autostart_dir) = dirs::config_dir().map(|d| d.join("autostart")) else {
        return;
    };
    let desktop_path = autostart_dir.join("crabclip.desktop");

    if enable {
        let _ = fs::create_dir_all(&autostart_dir);
        let content = include_str!("../autostart/crabclip.desktop");
        let _ = fs::write(&desktop_path, content);
    } else {
        let _ = fs::remove_file(&desktop_path);
    }
}
