// Bevy and ObjC interop require significant unsafe code throughout this crate.
#![allow(clippy::undocumented_unsafe_blocks)]

mod camera;
mod debug;
mod hmd;
mod screen_capture;
mod settings;
mod stage;
mod virtual_display;

use bevy::{
    prelude::*,
    window::{PresentMode, WindowMode},
};

use camera::CameraPlugin;
use debug::DebugPlugin;
use hmd::HmdPlugin;
use screen_capture::{ensure_screen_capture_permission, ScreenCapturePlugin};
use settings::SettingsPlugin;
use stage::StagePlugin;
use virtual_display::VirtualDisplayPlugin;

#[derive(Resource)]
pub struct ScaleFactor {
    pub value: f64,
}

fn wait_for_physical_display_modes() {
    use std::time::{Duration, Instant};
    let timeout = Duration::from_secs(10);
    let poll_interval = Duration::from_millis(50);
    let start = Instant::now();

    loop {
        let all = stage::get_active_displays(32);
        let missing: Vec<u32> = all
            .iter()
            .filter(|(_, cg)| cg.copy_display_modes().is_none())
            .map(|(id, _)| *id)
            .collect();

        if missing.is_empty() {
            eprintln!(
                "All physical display modes ready after {:.0?}",
                start.elapsed()
            );
            break;
        }
        if start.elapsed() > timeout {
            eprintln!("Timed out waiting for physical display modes on: {missing:?}");
            break;
        }
        std::thread::sleep(poll_interval);
    }
}

fn main() {
    ensure_screen_capture_permission();
    wait_for_physical_display_modes();

    // Load settings once, before anything else needs them.
    let settings = settings::load_settings();

    App::new()
        .insert_resource(AmbientLight {
            color: Color::default(),
            brightness: 100.0,
        })
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
                // mode: WindowMode::Fullscreen(MonitorSelection::Index(1)),
                // position: WindowPosition::Centered(MonitorSelection::Index(1)), // 0 is primary, 1 is secondary
                // mode: WindowMode::Windowed,
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
