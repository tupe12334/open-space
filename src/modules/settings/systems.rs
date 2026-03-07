use bevy::prelude::*;
use bevy::window::{Monitor, MonitorSelection, PrimaryMonitor, PrimaryWindow, WindowMode};
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

pub(super) fn select_glasses_fullscreen(
    mut done: Local<bool>,
    settings: Res<AppSettings>,
    monitors: Query<(Entity, &Monitor, Option<&PrimaryMonitor>)>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if *done {
        return;
    }

    // Wait until winit has populated monitor entities.
    if monitors.is_empty() {
        return;
    }

    *done = true;

    for (entity, monitor, primary) in &monitors {
        info!(
            "Monitor {:?}: name={:?}, size={}x{}, refresh={}mHz, primary={}",
            entity,
            monitor.name,
            monitor.physical_width,
            monitor.physical_height,
            monitor.refresh_rate_millihertz.unwrap_or(0),
            primary.is_some()
        );
    }

    let Ok(mut window) = windows.single_mut() else {
        return;
    };

    let glasses_entity = settings.glasses_monitor_name.as_ref().map_or_else(
        || {
            // Pick the non-primary monitor with the highest refresh rate.
            // XREAL Air glasses run at 120Hz vs virtual screens at 60Hz.
            monitors
                .iter()
                .filter(|(_, _, primary)| primary.is_none())
                .max_by_key(|(_, m, _)| m.refresh_rate_millihertz.unwrap_or(0))
                .map(|(entity, _, _)| entity)
        },
        |name_filter| {
            let filter_lower = name_filter.to_lowercase();
            monitors
                .iter()
                .find(|(_, monitor, _)| {
                    monitor
                        .name
                        .as_ref()
                        .is_some_and(|n| n.to_lowercase().contains(&filter_lower))
                })
                .map(|(entity, _, _)| entity)
        },
    );

    match glasses_entity {
        Some(entity) => {
            let name = monitors
                .get(entity)
                .ok()
                .and_then(|(_, m, _)| m.name.clone())
                .unwrap_or_else(|| "unknown".into());
            info!("Switching to fullscreen on glasses monitor: {name}");
            window.mode = WindowMode::BorderlessFullscreen(MonitorSelection::Entity(entity));
        }
        None => {
            warn!("No glasses monitor found; staying windowed");
        }
    }
}
