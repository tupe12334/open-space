use bevy::prelude::*;
use std::sync::atomic::Ordering;

use super::{
    AppSettings, DISTANCE_STEP, DISTANCE_STEPS, MAX_DISTANCE, MAX_NUM_SCREENS, MIN_DISTANCE,
    MIN_NUM_SCREENS, SCREEN_STEPS,
};
use crate::modules::stage::ScreenMarker;

use super::persistence::save_settings;

pub(super) fn poll_menu_changes(
    mut settings: ResMut<AppSettings>,
    mut screen_transforms: Query<&mut Transform, With<ScreenMarker>>,
) {
    let dist_steps = DISTANCE_STEPS.swap(0, Ordering::Relaxed);
    let scr_steps = SCREEN_STEPS.swap(0, Ordering::Relaxed);

    if dist_steps == 0 && scr_steps == 0 {
        return;
    }

    if dist_steps != 0 {
        let delta = dist_steps as f32 * DISTANCE_STEP;
        settings.stage_distance =
            (settings.stage_distance + delta).clamp(MIN_DISTANCE, MAX_DISTANCE);

        for mut transform in &mut screen_transforms {
            transform.translation.z = -settings.stage_distance;
        }
    }

    if scr_steps != 0 {
        let new_count = (settings.num_screens as i32 + scr_steps)
            .clamp(MIN_NUM_SCREENS as i32, MAX_NUM_SCREENS as i32) as u32;
        settings.num_screens = new_count;
    }

    save_settings(&settings);
}
