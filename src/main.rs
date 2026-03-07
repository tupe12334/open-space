// Bevy and ObjC interop require significant unsafe code throughout this crate.
#![allow(
    clippy::undocumented_unsafe_blocks,
    reason = "ObjC interop requires pervasive unsafe throughout this crate"
)]

mod modules;

use bevy::{
    prelude::*,
    window::{PresentMode, WindowMode},
};

use modules::camera::CameraPlugin;
use modules::debug::DebugPlugin;
use modules::hmd::HmdPlugin;
use modules::screen_capture::{ensure_screen_capture_permission, ScreenCapturePlugin};
use modules::settings::SettingsPlugin;
use modules::stage::StagePlugin;
use modules::virtual_display::VirtualDisplayPlugin;

#[derive(Resource)]
pub struct ScaleFactor {
    pub value: f64,
}

fn main() {
    ensure_screen_capture_permission();
    modules::display_modes::wait_for_physical_display_modes();

    // Load settings once, before anything else needs them.
    let settings = modules::settings::load_settings();

    App::new()
        .insert_resource(ScaleFactor { value: 1.0 })
        .insert_resource(settings)
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                // https://docs.rs/bevy_window/latest/bevy_window/enum.PresentMode.html
                present_mode: PresentMode::AutoNoVsync, // AutoVsync, AutoNoVsync
                // when using AutoVsync, add the bevy_framepace plugin and uncomment
                // the framespace_settings lines in setup()
                resizable: true,
                focused: false,
                visible: true,
                // window_level: WindowLevel::AlwaysOnTop,
                mode: WindowMode::Windowed,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(SettingsPlugin)
        .add_plugins(CameraPlugin)
        .add_plugins(VirtualDisplayPlugin)
        .add_plugins(StagePlugin)
        .add_plugins(HmdPlugin)
        .add_plugins(ScreenCapturePlugin)
        .insert_resource(Time::<Fixed>::from_hz(500.0)) // when using Fixed schedule
        .add_plugins(DebugPlugin)
        .run();
}
