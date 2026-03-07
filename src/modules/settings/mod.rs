mod menu;
mod persistence;
mod plugin;
mod systems;

pub(crate) use persistence::load_settings;
pub(crate) use plugin::SettingsPlugin;

use bevy::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicI32};

const SETTINGS_FILE: &str = "settings.json";
const DEFAULT_STAGE_DISTANCE: f32 = 6.0;
const DISTANCE_STEP: f32 = 0.5;
const MIN_DISTANCE: f32 = 1.0;
const MAX_DISTANCE: f32 = 30.0;
const DEFAULT_NUM_SCREENS: u32 = 6;
const MIN_NUM_SCREENS: u32 = 1;
const MAX_NUM_SCREENS: u32 = 6;

static DISTANCE_STEPS: AtomicI32 = AtomicI32::new(0);
static SCREEN_STEPS: AtomicI32 = AtomicI32::new(0);
pub(crate) static CENTER_STAGE: AtomicBool = AtomicBool::new(false);

#[derive(Resource, Clone)]
pub(crate) struct AppSettings {
    pub(crate) stage_distance: f32,
    pub(crate) num_screens: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            stage_distance: DEFAULT_STAGE_DISTANCE,
            num_screens: DEFAULT_NUM_SCREENS,
        }
    }
}
