use std::sync::Arc;

use ar_drivers::{any_glasses, GlassesEvent};

use bevy::prelude::*;

use super::types::{CalibrationOffset, ImuStore};

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
const CALIBRATION_SAMPLES: u32 = 1000;

pub(in crate::modules::hmd) fn start_tracking(imu_store: Res<ImuStore>) {
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
        let mut gyro_sum_x: f64 = 0.0;
        let mut gyro_sum_y: f64 = 0.0;
        let mut gyro_sum_z: f64 = 0.0;
        let mut gyro_bias: Option<(f32, f32, f32)> = None;

        loop {
            if let GlassesEvent::AccGyro {
                accelerometer,
                gyroscope,
                timestamp,
            } = glasses.read_event().unwrap()
            {
                if let Some(last_timestamp) = last_timestamp {
                    let dt = (timestamp - last_timestamp) as f32 / 1_000_000.0; // in seconds

                    let (gx, gy, gz) = if let Some((bx, by, bz)) = gyro_bias {
                        (gyroscope.x - bx, gyroscope.y - by, gyroscope.z - bz)
                    } else {
                        gyro_sum_x += gyroscope.x as f64;
                        gyro_sum_y += gyroscope.y as f64;
                        gyro_sum_z += gyroscope.z as f64;
                        (gyroscope.x, gyroscope.y, gyroscope.z)
                    };

                    let mut dcmimu = shared_dcmimu_clone.lock().unwrap();
                    dcmimu.update(
                        (gx, gy, gz),
                        (accelerometer.x, accelerometer.y, accelerometer.z),
                        dt,
                    );

                    sample_count += 1;

                    // Capture calibration offset once the filter has stabilized
                    if sample_count == CALIBRATION_SAMPLES {
                        let count = sample_count as f64;
                        let bias = (
                            (gyro_sum_x / count) as f32,
                            (gyro_sum_y / count) as f32,
                            (gyro_sum_z / count) as f32,
                        );
                        gyro_bias = Some(bias);

                        let orientation = dcmimu.all();
                        drop(dcmimu);
                        *calibration_clone.lock().unwrap() = Some(CalibrationOffset {
                            yaw: orientation.yaw,
                            pitch: orientation.pitch,
                            roll: orientation.roll,
                        });
                        info!(
                            "IMU calibration captured after {CALIBRATION_SAMPLES} samples \
                             (gyro bias: x={:.6}, y={:.6}, z={:.6} rad/s)",
                            bias.0, bias.1, bias.2
                        );
                    }
                }

                last_timestamp = Some(timestamp);
            }
        }
    });
}
