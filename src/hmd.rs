use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use ahrs::{Ahrs, Madgwick};
use bevy::prelude::*;
use nalgebra::Vector3;

use crate::camera::MainCamera;
use crate::settings::CENTER_STAGE;

/// Shared glasses orientation, written by the tracking thread, read by the Bevy system.
#[derive(Resource)]
struct GlassesOrientation {
    quat: Arc<Mutex<Quat>>,
}

pub struct HmdPlugin;

impl Plugin for HmdPlugin {
    fn build(&self, app: &mut App) {
        let orientation = Arc::new(Mutex::new(Quat::IDENTITY));
        app.insert_resource(GlassesOrientation {
            quat: orientation.clone(),
        });
        std::thread::spawn(move || {
            tracking_thread(orientation);
        });
        app.add_systems(FixedUpdate, apply_glasses_orientation);
    }
}

/// Duration (in seconds) to let the Madgwick filter stabilize before capturing the reference.
/// The filter needs several seconds to converge toward gravity alignment.
const WARMUP_DURATION_SECS: f64 = 3.0;

fn tracking_thread(orientation: Arc<Mutex<Quat>>) {
    let glasses = ar_drivers::any_glasses();
    let mut glasses = match glasses {
        Ok(g) => {
            info!("AR glasses connected");
            g
        }
        Err(e) => {
            warn!(
                "Could not connect to AR glasses: {}. Head tracking disabled.",
                e
            );
            return;
        }
    };

    let mut last_sample_time: Option<std::time::Instant> = None;
    let mut dt_sum: f64 = 0.0;
    let mut dt_count: u64 = 0;
    let mut filter_calibrated = false;

    let mut ahrs = Madgwick::new(1.0 / 1000.0, 0.1);
    let tracking_start = std::time::Instant::now();
    let mut sample_count: u32 = 0;
    let mut reference_quat: Option<nalgebra::UnitQuaternion<f64>> = None;
    let mut log_counter: u32 = 0;

    loop {
        match glasses.read_event() {
            Ok(ar_drivers::GlassesEvent::AccGyro {
                accelerometer,
                gyroscope,
                ..
            }) => {
                let now = std::time::Instant::now();

                // Measure actual IMU sample interval
                if let Some(prev) = last_sample_time {
                    let dt = now.duration_since(prev).as_secs_f64();
                    dt_sum += dt;
                    dt_count += 1;

                    // Recalibrate filter with measured rate once
                    if !filter_calibrated && dt_count >= 100 {
                        let avg_dt = dt_sum / dt_count as f64;
                        info!(
                            "Measured IMU rate: {:.1} Hz (dt={:.4}s), recalibrating filter",
                            1.0 / avg_dt,
                            avg_dt
                        );
                        ahrs = Madgwick::new(avg_dt, 0.1);
                        filter_calibrated = true;
                    }
                }
                last_sample_time = Some(now);

                let gyro = Vector3::new(gyroscope.x as f64, gyroscope.y as f64, gyroscope.z as f64);
                let accel = Vector3::new(
                    accelerometer.x as f64,
                    accelerometer.y as f64,
                    accelerometer.z as f64,
                );

                if let Ok(q) = ahrs.update_imu(&gyro, &accel) {
                    sample_count = sample_count.saturating_add(1);

                    // Capture reference after time-based warmup (independent of IMU rate)
                    let elapsed = now.duration_since(tracking_start).as_secs_f64();
                    if reference_quat.is_none() && elapsed >= WARMUP_DURATION_SECS {
                        reference_quat = Some(*q);
                        info!(
                            "Head tracking calibrated after {:.1}s ({} samples)",
                            elapsed, sample_count
                        );
                    }

                    // Re-center: update reference to current orientation
                    if CENTER_STAGE.swap(false, Ordering::Relaxed) {
                        reference_quat = Some(*q);
                        info!("Center stage: head tracking reference reset");
                    }

                    if let Some(ref ref_q) = reference_quat {
                        // Relative rotation: how much the head has rotated since calibration
                        let relative = ref_q.inverse() * q;
                        let bevy_quat = Quat::from_xyzw(
                            relative.i as f32,
                            relative.j as f32,
                            relative.k as f32,
                            relative.w as f32,
                        );

                        // Log orientation periodically to detect drift
                        log_counter += 1;
                        if log_counter.is_multiple_of(1000) {
                            let (pitch, yaw, roll) = bevy_quat.to_euler(EulerRot::XYZ);
                            info!(
                                "[HMD] pitch={:.3}° yaw={:.3}° roll={:.3}° (sample #{})",
                                pitch.to_degrees(),
                                yaw.to_degrees(),
                                roll.to_degrees(),
                                sample_count,
                            );
                        }

                        if let Ok(mut lock) = orientation.lock() {
                            *lock = bevy_quat;
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                error!("Glasses read error: {}. Stopping tracking.", e);
                return;
            }
        }
    }
}

fn apply_glasses_orientation(
    glasses: Res<GlassesOrientation>,
    mut query: Query<&mut Transform, With<MainCamera>>,
) {
    let quat = match glasses.quat.lock() {
        Ok(lock) => *lock,
        Err(_) => return,
    };

    for mut transform in query.iter_mut() {
        transform.rotation = quat;
    }
}
