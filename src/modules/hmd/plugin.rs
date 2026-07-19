use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use dcmimu::DCMIMU;

use super::tracking::{auto_correct_drift, start_tracking, update_camera_orientation, ImuStore};

pub(crate) struct HmdPlugin;

impl Plugin for HmdPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ImuStore {
            dcmimu: Arc::new(Mutex::new(DCMIMU::new())),
            calibration: Arc::new(Mutex::new(None)),
        })
        .add_systems(Startup, start_tracking)
        .add_systems(FixedPreUpdate, update_camera_orientation)
        .add_systems(Update, auto_correct_drift);
    }
}
