use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use dcmimu::DCMIMU;

/// Stores the initial IMU orientation captured at startup, used to zero-out
/// the camera so "wherever you look when the app starts" becomes forward.
#[derive(Clone, Copy)]
pub(crate) struct CalibrationOffset {
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    pub(crate) roll: f32,
}

#[derive(Resource)]
pub(crate) struct ImuStore {
    pub(crate) dcmimu: Arc<Mutex<DCMIMU>>,
    pub(crate) calibration: Arc<Mutex<Option<CalibrationOffset>>>,
}
