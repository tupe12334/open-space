use bevy::prelude::*;

use super::systems::{poll_menu_changes, select_glasses_fullscreen};

pub(crate) struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, select_glasses_fullscreen)
            .add_systems(Update, poll_menu_changes);
    }
}
