use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
};

use super::{count_frames, debug_transforms, DebugTimer, FrameCounter};

pub(crate) struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DebugTimer(Timer::from_seconds(2.0, TimerMode::Repeating)))
            .insert_resource(FrameCounter(0))
            .add_systems(Update, (debug_transforms, count_frames))
            .add_plugins((
                FrameTimeDiagnosticsPlugin::default(),
                LogDiagnosticsPlugin::default(),
            ));
    }
}
