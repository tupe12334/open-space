use bevy::prelude::*;

use super::types::ImuStore;

/// Maximum angle (radians) from center for auto-correction to activate.
/// ~5 degrees — if the view is within this cone of "forward",
/// we assume any offset is drift rather than intentional movement.
const DRIFT_THRESHOLD: f32 = 0.087;

/// Fraction of the offset corrected per second.
/// 0.1 = 10% per second — slow enough to be imperceptible.
const DRIFT_CORRECTION_RATE: f32 = 0.1;

pub(crate) fn auto_correct_drift(state: Res<ImuStore>, time: Res<Time>) {
    let dcm = state.dcmimu.lock().unwrap().all();

    let cal = {
        let lock = state.calibration.lock().unwrap();
        match *lock {
            Some(c) => c,
            None => return,
        }
    };

    let rel_yaw = dcm.yaw - cal.yaw;
    let rel_pitch = dcm.pitch - cal.pitch;
    let rel_roll = dcm.roll - cal.roll;

    if rel_yaw.abs() > DRIFT_THRESHOLD
        || rel_pitch.abs() > DRIFT_THRESHOLD
        || rel_roll.abs() > DRIFT_THRESHOLD
    {
        return;
    }

    let alpha = (DRIFT_CORRECTION_RATE * time.delta_secs()).min(1.0);

    let mut lock = state.calibration.lock().unwrap();
    if let Some(cal) = lock.as_mut() {
        cal.yaw += rel_yaw * alpha;
        cal.pitch += rel_pitch * alpha;
        cal.roll += rel_roll * alpha;
    }
}
