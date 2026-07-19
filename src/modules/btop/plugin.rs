use std::sync::Arc;

use bevy::prelude::*;
use tokio::sync::mpsc;

use super::{setup_btop_capture, spawn_btop_panel, update_btop_texture, BtopFrameChannel};

pub(crate) struct BtopPlugin;

impl Plugin for BtopPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = mpsc::channel::<Vec<u8>>(60);
        app.insert_resource(BtopFrameChannel {
            sender: Arc::new(tx),
            receiver: rx,
        })
        .add_systems(Startup, setup_btop_capture)
        .add_systems(PostStartup, spawn_btop_panel)
        .add_systems(Update, update_btop_texture);
    }
}
