use bevy::prelude::*;

use super::{create_virtual_displays_system, VirtualDisplays};

pub(crate) struct VirtualDisplayPlugin;

impl Plugin for VirtualDisplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VirtualDisplays>()
            .add_systems(PreStartup, create_virtual_displays_system);
    }
}
