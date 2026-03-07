use std::fs;
use std::path::PathBuf;

use super::{
    AppSettings, DEFAULT_NUM_SCREENS, DEFAULT_STAGE_DISTANCE, MAX_NUM_SCREENS, MIN_NUM_SCREENS,
    SETTINGS_FILE,
};

fn settings_path() -> PathBuf {
    PathBuf::from(SETTINGS_FILE)
}

pub(crate) fn load_settings() -> AppSettings {
    let path = settings_path();
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&data) {
            let stage_distance = val
                .get("stage_distance")
                .and_then(serde_json::Value::as_f64)
                .map_or(DEFAULT_STAGE_DISTANCE, |d| d as f32);
            let num_screens = val
                .get("num_screens")
                .and_then(serde_json::Value::as_u64)
                .map_or(DEFAULT_NUM_SCREENS, |n| {
                    (n as u32).clamp(MIN_NUM_SCREENS, MAX_NUM_SCREENS)
                });
            return AppSettings {
                stage_distance,
                num_screens,
            };
        }
    }
    AppSettings::default()
}

pub(super) fn save_settings(settings: &AppSettings) {
    let val = serde_json::json!({
        "stage_distance": settings.stage_distance,
        "num_screens": settings.num_screens,
    });
    if let Ok(data) = serde_json::to_string_pretty(&val) {
        if let Err(e) = fs::write(settings_path(), data) {
            eprintln!("Failed to write settings: {e}");
        }
    }
}
