use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub docked_game_device: String,
    pub docked_voice_device: String,
    pub docked_same_device: bool,
    pub undocked_game_device: String,
    pub undocked_voice_device: String,
    pub undocked_same_device: bool,
    pub poll_interval: u64,
    pub auto_start: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            docked_game_device: String::new(),
            docked_voice_device: String::new(),
            docked_same_device: true,
            undocked_game_device: String::new(),
            undocked_voice_device: String::new(),
            undocked_same_device: false,
            poll_interval: 2,
            auto_start: false,
        }
    }
}

fn config_path(app_dir: &Path) -> PathBuf {
    app_dir.join("config.json")
}

pub fn load(app_dir: &Path) -> AppConfig {
    let path = config_path(app_dir);
    if path.exists() {
        let data = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        AppConfig::default()
    }
}

pub fn save(app_dir: &Path, config: &AppConfig) -> Result<(), String> {
    let path = config_path(app_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}
