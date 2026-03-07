use bevy::prelude::*;

use super::menu::setup_menu_bar;
use super::systems::{poll_menu_changes, select_glasses_fullscreen};

pub(crate) struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_menu_bar)
            .add_systems(Update, (select_glasses_fullscreen, poll_menu_changes));
    }
}
