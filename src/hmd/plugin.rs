use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use dcmimu::DCMIMU;

use super::tracking::{start_tracking, update_camera_orientation, ImuStore};

pub(crate) struct HmdPlugin;

impl Plugin for HmdPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ImuStore {
            dcmimu: Arc::new(Mutex::new(DCMIMU::new())),
        })
        .add_systems(Startup, start_tracking)
        .add_systems(FixedPreUpdate, update_camera_orientation);
    }
}
