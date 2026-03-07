use bevy::prelude::*;
use bevy::window::{Monitor, MonitorSelection, PrimaryMonitor, PrimaryWindow, WindowMode};
use std::sync::atomic::Ordering;

use super::{
    AppSettings, CENTER_STAGE, DISTANCE_STEP, DISTANCE_STEPS, MAX_DISTANCE, MAX_NUM_SCREENS,
    MIN_DISTANCE, MIN_NUM_SCREENS, SCREEN_STEPS,
};
use crate::modules::hmd::{CalibrationOffset, ImuStore};
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

pub(super) fn center_stage(imu_store: Option<Res<ImuStore>>) {
    if !CENTER_STAGE.swap(false, Ordering::Relaxed) {
        return;
    }

    let Some(imu_store) = imu_store else {
        warn!("No IMU store available \u{2014} cannot center stage");
        return;
    };

    let dcmimu = imu_store.dcmimu.lock().unwrap();
    let orientation = dcmimu.all();
    drop(dcmimu);

    *imu_store.calibration.lock().unwrap() = Some(CalibrationOffset {
        yaw: orientation.yaw,
        pitch: orientation.pitch,
        roll: orientation.roll,
    });

    info!("Center stage: recalibrated to current orientation");
}

/// Maximum number of update frames to wait for the glasses monitor to appear.
const GLASSES_SCAN_MAX_FRAMES: u32 = 300;

pub(super) fn select_glasses_fullscreen(
    mut done: Local<bool>,
    mut frames_waited: Local<u32>,
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

    let is_virtual = |m: &Monitor| {
        m.name
            .as_ref()
            .is_some_and(|n| n.starts_with("Virtual Screen"))
    };

    let glasses_entity = settings.glasses_monitor_name.as_ref().map_or_else(
        || {
            // Pick the non-primary, non-virtual monitor with the highest refresh rate.
            // XREAL Air glasses run at 120Hz vs virtual screens at 60Hz.
            monitors
                .iter()
                .filter(|(_, m, primary)| primary.is_none() && !is_virtual(m))
                .max_by_key(|(_, m, _)| m.refresh_rate_millihertz.unwrap_or(0))
                .map(|(entity, _, _)| entity)
        },
        |name_filter| {
            let filter_lower = name_filter.to_lowercase();
            monitors
                .iter()
                .filter(|(_, m, _)| !is_virtual(m))
                .find(|(_, monitor, _)| {
                    monitor
                        .name
                        .as_ref()
                        .is_some_and(|n| n.to_lowercase().contains(&filter_lower))
                })
                .map(|(entity, _, _)| entity)
        },
    );

    if let Some(entity) = glasses_entity {
        *done = true;

        for (e, monitor, primary) in &monitors {
            info!(
                "Monitor {:?}: name={:?}, size={}x{}, refresh={}mHz, primary={}, virtual={}",
                e,
                monitor.name,
                monitor.physical_width,
                monitor.physical_height,
                monitor.refresh_rate_millihertz.unwrap_or(0),
                primary.is_some(),
                is_virtual(monitor),
            );
        }

        let Ok(mut window) = windows.single_mut() else {
            return;
        };
        let name = monitors
            .get(entity)
            .ok()
            .and_then(|(_, m, _)| m.name.clone())
            .unwrap_or_else(|| "unknown".into());
        info!("Switching to fullscreen on glasses monitor: {name}");
        window.mode = WindowMode::BorderlessFullscreen(MonitorSelection::Entity(entity));
    } else {
        *frames_waited += 1;
        if *frames_waited >= GLASSES_SCAN_MAX_FRAMES {
            *done = true;
            warn!(
                "No glasses monitor found after {GLASSES_SCAN_MAX_FRAMES} frames; staying windowed on primary"
            );
        }
    }
}
