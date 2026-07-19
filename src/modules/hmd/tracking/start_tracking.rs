use std::sync::Arc;

use ar_drivers::{any_glasses, GlassesEvent};

use bevy::prelude::*;
use dcmimu::DCMIMU;

use super::types::{CalibrationOffset, ImuStore};

/// Number of IMU samples to average for gyro bias estimation.
const BIAS_SAMPLES: u32 = 1000;

/// Extra samples after DCMIMU reset to let pitch/roll reconverge before
/// capturing the calibration offset.
const CONVERGENCE_SAMPLES: u32 = 500;

/// Initializes HMD (Head-Mounted Display) motion tracking in a separate thread
///
/// # Arguments
/// * `imu_store` - Resource containing shared DCMIMU state for orientation tracking
///
/// # Details
/// Calibration has two phases:
/// 1. **Bias estimation** (first `BIAS_SAMPLES`): raw gyro readings are averaged
///    to compute a per-axis bias while the filter runs on raw data.
/// 2. **Reconvergence** (next `CONVERGENCE_SAMPLES`): the DCMIMU is reset and fed
///    bias-corrected data so its internal state starts clean. Pitch/roll reconverge
///    from accelerometer; yaw starts at 0 with near-zero bias input.
///
/// After both phases complete, the current orientation is captured as the
/// calibration offset (the "forward" direction).
pub(crate) fn start_tracking(imu_store: Res<ImuStore>) {
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

                    if sample_count == BIAS_SAMPLES {
                        // Phase 1 complete: compute gyro bias and reset the filter
                        let count = sample_count as f64;
                        let bias = (
                            (gyro_sum_x / count) as f32,
                            (gyro_sum_y / count) as f32,
                            (gyro_sum_z / count) as f32,
                        );
                        gyro_bias = Some(bias);

                        // Reset DCMIMU so its internal bias estimates (x3-x5)
                        // start at zero, matching the now-corrected gyro input.
                        *dcmimu = DCMIMU::new();

                        info!(
                            "Gyro bias estimated after {BIAS_SAMPLES} samples \
                             (x={:.6}, y={:.6}, z={:.6} rad/s), \
                             DCMIMU reset for reconvergence",
                            bias.0, bias.1, bias.2
                        );
                    } else if sample_count == BIAS_SAMPLES + CONVERGENCE_SAMPLES {
                        // Phase 2 complete: filter has reconverged with clean data
                        let orientation = dcmimu.all();
                        drop(dcmimu);
                        *calibration_clone.lock().unwrap() = Some(CalibrationOffset {
                            yaw: orientation.yaw,
                            pitch: orientation.pitch,
                            roll: orientation.roll,
                        });
                        info!(
                            "IMU calibration captured after {} total samples",
                            BIAS_SAMPLES + CONVERGENCE_SAMPLES
                        );
                    } else {
                        // Normal sample: no action needed outside calibration thresholds
                    }
                }

                last_timestamp = Some(timestamp);
            }
        }
    });
}
