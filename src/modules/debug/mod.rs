mod plugin;

pub(crate) use plugin::DebugPlugin;

use bevy::{prelude::*, window::Monitor};

use crate::modules::camera::MainCamera;
use crate::modules::stage::{AssetHandles, ScreenMarker};

#[derive(Resource)]
pub(super) struct DebugTimer(pub(super) Timer);

#[derive(Resource)]
pub(super) struct FrameCounter(pub(super) u64);

pub(super) fn count_frames(mut counter: ResMut<FrameCounter>) {
    counter.0 += 1;
    if counter.0 <= 5 || counter.0.is_multiple_of(100) {
        info!("[DEBUG] Update frame #{}", counter.0);
    }
}

pub(super) fn debug_transforms(
    time: Res<Time>,
    mut timer: ResMut<DebugTimer>,
    camera_query: Query<(Entity, &Transform, &Camera, &Projection), With<MainCamera>>,
    screen_query: Query<
        (
            Entity,
            &Transform,
            &MeshMaterial3d<StandardMaterial>,
            &Mesh3d,
        ),
        With<ScreenMarker>,
    >,
    window_query: Query<&Window>,
    monitor_query: Query<(Entity, &Monitor)>,
    images: Res<Assets<Image>>,
    materials: Res<Assets<StandardMaterial>>,
    asset_handles: Option<Res<AssetHandles>>,
    ambient_query: Query<Entity, With<AmbientLight>>,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }

    info!("[DEBUG] ===== Debug tick =====");

    // Camera details
    let cam_count = camera_query.iter().count();
    info!("[DEBUG] Camera entities: {cam_count}");
    for (entity, cam_tf, camera, projection) in &camera_query {
        let (pitch, yaw, roll) = cam_tf.rotation.to_euler(EulerRot::XYZ);
        info!(
            "[DEBUG] Camera {:?} pos=({:.3}, {:.3}, {:.3}) pitch={:.3}\u{b0} yaw={:.3}\u{b0} roll={:.3}\u{b0}",
            entity,
            cam_tf.translation.x,
            cam_tf.translation.y,
            cam_tf.translation.z,
            pitch.to_degrees(),
            yaw.to_degrees(),
            roll.to_degrees(),
        );
        info!(
            "[DEBUG]   is_active={} order={} clear_color={:?}",
            camera.is_active, camera.order, camera.clear_color,
        );
        if let Projection::Perspective(persp) = projection {
            info!(
                "[DEBUG]   fov={:.2}\u{b0} near={} far={}",
                persp.fov.to_degrees(),
                persp.near,
                persp.far,
            );
        }
    }

    // Ambient light
    let ambient_count = ambient_query.iter().count();
    info!("[DEBUG] AmbientLight entities: {ambient_count}");

    // Screen entities
    let screen_count = screen_query.iter().count();
    info!("[DEBUG] Screen entities: {screen_count}");
    for (entity, tf, mat_handle, _mesh) in &screen_query {
        info!(
            "[DEBUG]   Screen {:?} pos=({:.3}, {:.3}, {:.3})",
            entity, tf.translation.x, tf.translation.y, tf.translation.z,
        );
        if let Some(material) = materials.get(&mat_handle.0) {
            let has_texture = material.base_color_texture.is_some();
            info!(
                "[DEBUG]     material: unlit={} has_texture={} alpha={:?} base_color={:?}",
                material.unlit, has_texture, material.alpha_mode, material.base_color,
            );
        } else {
            warn!("[DEBUG]     material NOT FOUND in assets!");
        }
    }

    // Texture asset details
    if let Some(handles) = &asset_handles {
        info!(
            "[DEBUG] AssetHandles: {} screen textures",
            handles.screens.len()
        );
        for (i, handle) in handles.screens.iter().enumerate() {
            if let Some(image) = images.get(handle) {
                let has_data = image.data.is_some();
                let data_len = image.data.as_ref().map_or(0, Vec::len);
                let w = image.texture_descriptor.size.width;
                let h = image.texture_descriptor.size.height;
                let fmt = image.texture_descriptor.format;
                let usage = image.texture_descriptor.usage;
                info!(
                    "[DEBUG]   texture[{}]: {}x{} fmt={:?} usage={:?} has_data={} data_len={}",
                    i, w, h, fmt, usage, has_data, data_len,
                );
            } else {
                warn!("[DEBUG]   texture[{}]: NOT FOUND in image assets!", i);
            }
        }
    } else {
        warn!("[DEBUG] AssetHandles resource NOT FOUND!");
    }

    // Window
    for window in &window_query {
        info!(
            "[DEBUG] Window pos=({:?}) size=({:.0}x{:.0}) scale={:.2} visible={} focused={}",
            window.position,
            window.resolution.width(),
            window.resolution.height(),
            window.resolution.scale_factor(),
            window.visible,
            window.focused,
        );
    }

    let monitor_count = monitor_query.iter().count();
    info!("[DEBUG] Active monitors: {monitor_count}");
    info!("[DEBUG] Total images in asset store: {}", images.len());
    info!(
        "[DEBUG] Total materials in asset store: {}",
        materials.len()
    );
}
