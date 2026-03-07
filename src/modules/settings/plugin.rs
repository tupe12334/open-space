use bevy::prelude::*;

use super::systems::poll_menu_changes;

pub(crate) struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, poll_menu_changes);
    }
}
