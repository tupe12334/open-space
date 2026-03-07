use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use dcmimu::DCMIMU;

/// Stores the initial IMU orientation captured at startup, used to zero-out
/// the camera so "wherever you look when the app starts" becomes forward.
#[derive(Clone, Copy)]
pub(in crate::modules::hmd) struct CalibrationOffset {
    pub(in crate::modules::hmd) yaw: f32,
    pub(in crate::modules::hmd) pitch: f32,
    pub(in crate::modules::hmd) roll: f32,
}

#[derive(Resource)]
pub(in crate::modules::hmd) struct ImuStore {
    pub(in crate::modules::hmd) dcmimu: Arc<Mutex<DCMIMU>>,
    pub(in crate::modules::hmd) calibration: Arc<Mutex<Option<CalibrationOffset>>>,
}
