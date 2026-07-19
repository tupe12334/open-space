use bevy::prelude::*;

use super::types::DistanceStore;
use super::{log_distance, start_webcam_capture};

pub(crate) struct WebcamDistancePlugin;

impl Plugin for WebcamDistancePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DistanceStore::new())
            .add_systems(Startup, start_webcam_capture)
            .add_systems(Update, log_distance);
    }
}
