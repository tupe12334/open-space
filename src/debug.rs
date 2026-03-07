use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
    window::Monitor,
};

use crate::camera::MainCamera;
use crate::stage::ScreenMarker;

pub struct DebugPlugin;

#[derive(Resource)]
struct DebugTimer(Timer);

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DebugTimer(Timer::from_seconds(2.0, TimerMode::Repeating)))
            .add_systems(Update, debug_transforms)
            .add_plugins((
                FrameTimeDiagnosticsPlugin::default(),
                LogDiagnosticsPlugin::default(),
            ));
    }
}

fn debug_transforms(
    time: Res<Time>,
    mut timer: ResMut<DebugTimer>,
    camera_query: Query<&Transform, With<MainCamera>>,
    screen_query: Query<(Entity, &Transform), With<ScreenMarker>>,
    window_query: Query<&Window>,
    monitor_query: Query<(Entity, &Monitor)>,
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

    for window in &window_query {
        info!(
            "[DEBUG] Window pos=({:?}) size=({:.0}x{:.0}) scale={:.2}",
            window.position,
            window.resolution.width(),
            window.resolution.height(),
            window.resolution.scale_factor(),
        );
    }

    let monitor_count = monitor_query.iter().count();
    info!("[DEBUG] Active monitors: {monitor_count}");
}
