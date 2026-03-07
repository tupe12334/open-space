use std::sync::{Arc, Mutex};

use bevy::prelude::*;

#[derive(Resource)]
pub(crate) struct DistanceStore {
    pub(crate) distance_cm: Arc<Mutex<Option<f32>>>,
}

impl DistanceStore {
    pub(crate) fn new() -> Self {
        Self {
            distance_cm: Arc::new(Mutex::new(None)),
        }
    }
}
