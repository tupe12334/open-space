mod plugin;

pub(crate) use plugin::CameraPlugin;

use bevy::prelude::*;

#[derive(Component, Debug)]
pub(crate) struct MainCamera;

pub(super) fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 21.70_f32.to_radians(),
            ..default()
        }),
        Transform::from_xyz(0.0, 0.0, 0.0),
        MainCamera,
    ));

    commands.spawn(AmbientLight {
        color: Color::default(),
        brightness: 100.0,
        ..default()
    });
}
