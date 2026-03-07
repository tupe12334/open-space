use bevy::prelude::*;

use super::systems::{spawn_screen, spawn_stage};

pub(crate) struct StagePlugin;

impl Plugin for StagePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_stage, spawn_screen));
    }
}
