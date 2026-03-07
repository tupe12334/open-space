use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use dcmimu::DCMIMU;

use crate::camera::MainCamera;
use crate::settings::CENTER_STAGE;

/// Shared glasses orientation, written by the tracking thread, read by the Bevy system.
#[derive(Resource)]
struct GlassesOrientation {
    quat: Arc<Mutex<Quat>>,
}

pub(crate) struct HmdPlugin;

impl Plugin for HmdPlugin {
    fn build(&self, app: &mut App) {
        let orientation = Arc::new(Mutex::new(Quat::IDENTITY));
        app.insert_resource(GlassesOrientation {
            quat: Arc::clone(&orientation),
        });
        std::thread::spawn(move || {
            tracking_thread(orientation);
        });
        app.add_systems(FixedPreUpdate, apply_glasses_orientation);
    }
}

/// Duration (in seconds) to let the DCMIMU filter stabilize before capturing the reference.
const WARMUP_DURATION_SECS: f64 = 3.0;

fn tracking_thread(orientation: Arc<Mutex<Quat>>) {
    let mut glasses = match ar_drivers::any_glasses() {
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

    let mut dcmimu = DCMIMU::new();
    let mut last_timestamp: Option<u64> = None;
    let tracking_start = std::time::Instant::now();
    let mut sample_count: u32 = 0;
    let mut reference_quat: Option<Quat> = None;
    let mut log_counter: u32 = 0;

    loop {
        match glasses.read_event() {
            Ok(ar_drivers::GlassesEvent::AccGyro {
                accelerometer,
                gyroscope,
                timestamp,
            }) => {
                if let Some(prev_ts) = last_timestamp {
                    let dt = (timestamp - prev_ts) as f32 / 1_000_000.0;

                    dcmimu.update(
                        (gyroscope.x, gyroscope.y, gyroscope.z),
                        (accelerometer.x, accelerometer.y, accelerometer.z),
                        dt,
                    );

                    let dcm = dcmimu.all();
                    let current_quat =
                        Quat::from_euler(EulerRot::YXZ, dcm.yaw, dcm.roll, -dcm.pitch);

                    sample_count = sample_count.saturating_add(1);

                    // Capture reference after warmup
                    let elapsed = tracking_start.elapsed().as_secs_f64();
                    if reference_quat.is_none() && elapsed >= WARMUP_DURATION_SECS {
                        reference_quat = Some(current_quat);
                        info!(
                            "Head tracking calibrated after {:.1}s ({} samples)",
                            elapsed, sample_count
                        );
                    }

                    // Re-center: update reference to current orientation
                    if CENTER_STAGE.swap(false, Ordering::Relaxed) {
                        reference_quat = Some(current_quat);
                        info!("Center stage: head tracking reference reset");
                    }

                    if let Some(ref_q) = reference_quat {
                        let relative = ref_q.inverse() * current_quat;

                        // Log orientation periodically to detect drift
                        log_counter += 1;
                        if log_counter.is_multiple_of(1000) {
                            let (pitch, yaw, roll) = relative.to_euler(EulerRot::XYZ);
                            info!(
                                "[HMD] pitch={:.3} yaw={:.3} roll={:.3} (sample #{})",
                                pitch.to_degrees(),
                                yaw.to_degrees(),
                                roll.to_degrees(),
                                sample_count,
                            );
                        }

                        if let Ok(mut lock) = orientation.lock() {
                            *lock = relative;
                        }
                    }
                }

                last_timestamp = Some(timestamp);
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

    for mut transform in &mut query {
        transform.rotation = quat;
    }
}
