use std::fs;
use std::path::PathBuf;

use super::{AppSettings, MAX_NUM_SCREENS, MIN_NUM_SCREENS, SETTINGS_FILE};

fn settings_path() -> PathBuf {
    PathBuf::from(SETTINGS_FILE)
}

fn clamp_settings(mut settings: AppSettings) -> AppSettings {
    settings.num_screens = settings.num_screens.clamp(MIN_NUM_SCREENS, MAX_NUM_SCREENS);
    settings
}

pub(crate) fn load_settings() -> AppSettings {
    fs::read_to_string(settings_path())
        .ok()
        .and_then(|data| serde_json::from_str(&data).ok())
        .map(clamp_settings)
        .unwrap_or_default()
}

pub(super) fn save_settings(settings: &AppSettings) {
    if let Ok(data) = serde_json::to_string_pretty(settings) {
        if let Err(e) = fs::write(settings_path(), data) {
            eprintln!("Failed to write settings: {e}");
        }
    }
}
