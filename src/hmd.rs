use bevy::prelude::*;

pub struct HmdPlugin;

impl Plugin for HmdPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, log_no_hmd);
    }
}

fn log_no_hmd() {
    info!("HMD tracking disabled (ar-drivers not available)");
}
