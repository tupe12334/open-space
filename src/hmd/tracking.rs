use std::sync::{Arc, Mutex};

use ar_drivers::{any_glasses, GlassesEvent};

use bevy::prelude::*;
use dcmimu::DCMIMU;

/// Stores the initial IMU orientation captured at startup, used to zero-out
/// the camera so "wherever you look when the app starts" becomes forward.
#[derive(Clone, Copy)]
pub(super) struct CalibrationOffset {
    pub(super) yaw: f32,
    pub(super) pitch: f32,
    pub(super) roll: f32,
}

#[derive(Resource)]
pub(super) struct ImuStore {
    pub(super) dcmimu: Arc<Mutex<DCMIMU>>,
    pub(super) calibration: Arc<Mutex<Option<CalibrationOffset>>>,
}

/// Initializes HMD (Head-Mounted Display) motion tracking in a separate thread
///
/// # Arguments
/// * `imu_store` - Resource containing shared DCMIMU state for orientation tracking
///
/// # Details
/// This function:
/// 1. Spawns a dedicated thread for processing IMU (Inertial Measurement Unit) data
/// 2. Continuously reads accelerometer and gyroscope data from the glasses
/// 3. Updates the shared DCMIMU state with new motion data
/// 4. Calculates time delta between measurements for accurate integration
///
/// The motion data is used to update camera orientation in the main rendering thread.
/// Number of IMU samples to process before capturing the calibration offset.
/// This gives the DCMIMU filter time to converge on a stable orientation.
const CALIBRATION_SAMPLES: u32 = 100;

pub(super) fn start_tracking(imu_store: Res<ImuStore>) {
    let shared_dcmimu_clone = Arc::clone(&imu_store.dcmimu);
    let calibration_clone = Arc::clone(&imu_store.calibration);

    #[expect(
        clippy::infinite_loop,
        reason = "tracking thread runs until process exit"
    )]
    std::thread::spawn(move || {
        let mut glasses = match any_glasses() {
            Ok(g) => g,
            Err(e) => {
                warn!("No AR glasses found: {}. Running without head tracking.", e);
                return;
            }
        };
        let mut last_timestamp: Option<u64> = None;
        let mut sample_count: u32 = 0;

        loop {
            if let GlassesEvent::AccGyro {
                accelerometer,
                gyroscope,
                timestamp,
            } = glasses.read_event().unwrap()
            {
                if let Some(last_timestamp) = last_timestamp {
                    let dt = (timestamp - last_timestamp) as f32 / 1_000_000.0; // in seconds

                    let mut dcmimu = shared_dcmimu_clone.lock().unwrap();
                    dcmimu.update(
                        (gyroscope.x, gyroscope.y, gyroscope.z),
                        (accelerometer.x, accelerometer.y, accelerometer.z),
                        dt,
                    );

                    sample_count += 1;

                    // Capture calibration offset once the filter has stabilized
                    if sample_count == CALIBRATION_SAMPLES {
                        let orientation = dcmimu.all();
                        drop(dcmimu);
                        *calibration_clone.lock().unwrap() = Some(CalibrationOffset {
                            yaw: orientation.yaw,
                            pitch: orientation.pitch,
                            roll: orientation.roll,
                        });
                        info!("IMU calibration captured after {CALIBRATION_SAMPLES} samples");
                    }
                }

                last_timestamp = Some(timestamp);
            }
        }
    });
}

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
pub(super) fn update_camera_orientation(
    mut query: Query<&mut Transform, With<Camera>>,
    state: Res<ImuStore>,
) {
    let Some(cal) = *state.calibration.lock().unwrap() else {
        return; // Not calibrated yet — keep the default camera orientation
    };

    let dcm = state.dcmimu.lock().unwrap().all();

    let rot = Transform::from_rotation(Quat::from_euler(
        EulerRot::YXZ,
        dcm.yaw - cal.yaw,
        -(dcm.roll - cal.roll),
        -(dcm.pitch - cal.pitch),
    ));

    for mut transform in &mut query {
        transform.rotation = rot.rotation;
    }
}
