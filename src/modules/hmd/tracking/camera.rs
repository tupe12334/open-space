use bevy::prelude::*;

use super::types::ImuStore;

/// Updates camera orientation based on HMD/glasses IMU data
///
/// # Arguments
/// * `query` - Query for all camera transforms that need updating
/// * `state` - Shared state containing IMU orientation data
///
/// # Details
/// Converts IMU orientation (yaw, pitch, roll) into a quaternion rotation
/// and applies it to all camera entities in the scene.
///
/// The rotation order is YXZ (yaw, roll inverted, pitch) to match the IMU coordinate system.
pub(crate) fn update_camera_orientation(
    mut query: Query<&mut Transform, With<Camera>>,
    state: Res<ImuStore>,
) {
    let Some(cal) = *state.calibration.lock().unwrap() else {
        return; // Not calibrated yet — keep the default camera orientation
    };

    let dcm = state.dcmimu.lock().unwrap().all();

    let rot = Transform::from_rotation(Quat::from_euler(
        EulerRot::YXZ,
        dcm.yaw - cal.yaw,        // left-right
        dcm.roll - cal.roll,      // up-down
        -(dcm.pitch - cal.pitch), // tilt right-left
    ));

    for mut transform in &mut query {
        transform.rotation = rot.rotation;
    }
}
