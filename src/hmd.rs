use std::sync::{Arc, Mutex};

use ahrs::{Ahrs, Madgwick};
use bevy::prelude::*;
use nalgebra::Vector3;

use crate::camera::MainCamera;

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

/// Number of IMU samples to let the Madgwick filter stabilize before capturing the reference.
const WARMUP_SAMPLES: u32 = 500;

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

    // Madgwick filter: sample_period ~1ms (1000Hz polling), beta = 0.1
    let mut ahrs = Madgwick::new(1.0 / 1000.0, 0.1);
    let mut sample_count: u32 = 0;
    let mut reference_quat: Option<nalgebra::UnitQuaternion<f64>> = None;

    loop {
        match glasses.read_event() {
            Ok(ar_drivers::GlassesEvent::AccGyro {
                accelerometer,
                gyroscope,
                ..
            }) => {
                let gyro = Vector3::new(gyroscope.x as f64, gyroscope.y as f64, gyroscope.z as f64);
                let accel = Vector3::new(
                    accelerometer.x as f64,
                    accelerometer.y as f64,
                    accelerometer.z as f64,
                );

                if let Ok(q) = ahrs.update_imu(&gyro, &accel) {
                    sample_count = sample_count.saturating_add(1);

                    // Capture reference orientation after warm-up
                    if sample_count == WARMUP_SAMPLES {
                        reference_quat = Some(*q);
                        info!("Head tracking calibrated");
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
