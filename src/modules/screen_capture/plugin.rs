use std::sync::Arc;

use bevy::prelude::*;
use tokio::sync::mpsc;

use super::{setup_screen_capture, update_screen_texture, FrameChannel};

pub(crate) struct ScreenCapturePlugin;

impl Plugin for ScreenCapturePlugin {
    fn build(&self, app: &mut App) {
        let num_screens = app
            .world()
            .get_resource::<crate::modules::settings::AppSettings>()
            .map_or(6, |s| s.num_screens as usize);
        let mut senders = Vec::new();
        let mut receivers = Vec::new();
        for _ in 0..num_screens {
            let (tx, rx) = mpsc::channel::<Vec<u8>>(60);
            senders.push(Arc::new(tx));
            receivers.push(rx);
        }
        app.insert_resource(FrameChannel { senders, receivers })
            .add_systems(Startup, setup_screen_capture)
            .add_systems(Update, update_screen_texture);
    }
}
