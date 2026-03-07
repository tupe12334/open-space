use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use dcmimu::DCMIMU;

use crate::camera::MainCamera;
use crate::settings::CENTER_STAGE;

/// Shared DCMIMU state — the tracking thread feeds IMU samples,
/// the Bevy system reads the fused orientation.
#[derive(Resource)]
struct ImuStore {
    dcmimu: Arc<Mutex<DCMIMU>>,
}

/// Stored reference orientation for relative head tracking.
#[derive(Resource)]
struct TrackingState {
    reference: Option<Quat>,
    sample_count: u32,
    log_counter: u32,
}

pub(crate) struct HmdPlugin;

impl Plugin for HmdPlugin {
    fn build(&self, app: &mut App) {
        let dcmimu = Arc::new(Mutex::new(DCMIMU::new()));
        let shared = Arc::clone(&dcmimu);
        app.insert_resource(ImuStore { dcmimu });
        app.insert_resource(TrackingState {
            reference: None,
            sample_count: 0,
            log_counter: 0,
        });
        std::thread::spawn(move || {
            tracking_thread(shared);
        });
        app.add_systems(FixedPreUpdate, apply_glasses_orientation);
    }
}

fn tracking_thread(dcmimu: Arc<Mutex<DCMIMU>>) {
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

    let mut last_timestamp: Option<u64> = None;

    loop {
        match glasses.read_event() {
            Ok(ar_drivers::GlassesEvent::AccGyro {
                accelerometer,
                gyroscope,
                timestamp,
            }) => {
                if let Some(prev_ts) = last_timestamp {
                    let dt = (timestamp - prev_ts) as f32 / 1_000_000.0;
                    if let Ok(mut imu) = dcmimu.lock() {
                        imu.update(
                            (gyroscope.x, gyroscope.y, gyroscope.z),
                            (accelerometer.x, accelerometer.y, accelerometer.z),
                            dt,
                        );
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

/// Duration (in seconds) to let the DCMIMU filter stabilize before capturing the reference.
const WARMUP_SECS: f32 = 3.0;

fn apply_glasses_orientation(
    store: Res<ImuStore>,
    mut state: ResMut<TrackingState>,
    mut query: Query<&mut Transform, With<MainCamera>>,
    time: Res<Time>,
) {
    let dcm = match store.dcmimu.lock() {
        Ok(imu) => imu.all(),
        Err(_) => return,
    };

    // Exact same euler mapping as spatial-display/src/hmd.rs:104-109
    let current = Quat::from_euler(EulerRot::YXZ, dcm.yaw, dcm.roll, dcm.pitch);

    state.sample_count = state.sample_count.saturating_add(1);

    // Capture reference after warmup
    if state.reference.is_none() && time.elapsed_secs() > WARMUP_SECS {
        state.reference = Some(current);
        info!(
            "Head tracking calibrated after {:.1}s ({} samples)",
            time.elapsed_secs(),
            state.sample_count,
        );
    }

    // Re-center
    if CENTER_STAGE.swap(false, Ordering::Relaxed) {
        state.reference = Some(current);
        info!("Center stage: head tracking reference reset");
    }

    let rotation = if let Some(ref_q) = state.reference {
        ref_q.inverse() * current
    } else {
        return;
    };

    // Log orientation periodically to detect drift
    state.log_counter += 1;
    if state.log_counter.is_multiple_of(1000) {
        let (pitch, yaw, roll) = rotation.to_euler(EulerRot::XYZ);
        info!(
            "[HMD] pitch={:.3} yaw={:.3} roll={:.3} (sample #{})",
            pitch.to_degrees(),
            yaw.to_degrees(),
            roll.to_degrees(),
            state.sample_count,
        );
    }

    for mut transform in &mut query {
        transform.rotation = rotation;
    }
}
