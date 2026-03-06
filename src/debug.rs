use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
};

use crate::camera::MainCamera;
use crate::stage::{AssetHandles, ScreenMarker};

pub struct DebugPlugin;

#[derive(Resource)]
struct DebugTimer(Timer);

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DebugTimer(Timer::from_seconds(2.0, TimerMode::Repeating)))
            .add_systems(Update, debug_transforms)
            .add_plugins((FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin::default()));
    }
}

fn debug_transforms(
    time: Res<Time>,
    mut timer: ResMut<DebugTimer>,
    camera_query: Query<&Transform, With<MainCamera>>,
    screen_query: Query<(Entity, &Transform), With<ScreenMarker>>,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }

    for cam_tf in &camera_query {
        let (pitch, yaw, roll) = cam_tf.rotation.to_euler(EulerRot::XYZ);
        info!(
            "[DEBUG] Camera pos=({:.3}, {:.3}, {:.3}) pitch={:.3}° yaw={:.3}° roll={:.3}°",
            cam_tf.translation.x,
            cam_tf.translation.y,
            cam_tf.translation.z,
            pitch.to_degrees(),
            yaw.to_degrees(),
            roll.to_degrees(),
        );
    }

    for (entity, tf) in &screen_query {
        info!(
            "[DEBUG] Screen {:?} pos=({:.3}, {:.3}, {:.3})",
            entity, tf.translation.x, tf.translation.y, tf.translation.z,
        );
    }
}

#[allow(dead_code)]
fn print_position(query: Query<(Entity, &Transform)>) {
    // Log the entity ID and translation of each entity with a `Position` component.
    for (entity, transform) in query.iter() {
        info!(
            "Entity {:?} is at position {:?},",
            entity, transform.translation
        );
    }
}

#[allow(dead_code)]
fn check_asset_handles(asset_server: Res<AssetServer>, asset_handles: Res<AssetHandles>) {
    for (i, handle) in asset_handles.screens.iter().enumerate() {
        info!(
            "AssetStates: screen[{}] {:?}",
            i,
            asset_server.get_load_state(handle.id()),
        );
    }
}
