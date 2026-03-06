use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

#[derive(Component, Debug)]
pub struct MainCamera;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            .add_systems(Update, camera_movement);
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 21.70f32.to_radians(),
            ..default()
        }),
        Transform::from_xyz(0.0, 0.0, 0.0),
        MainCamera,
    ));
}

fn camera_movement(
    mut mouse_motion: EventReader<MouseMotion>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Transform, With<MainCamera>>,
    time: Res<Time>,
) {
    let sensitivity = 0.003;
    let move_speed = 5.0;

    let mut delta = Vec2::ZERO;
    for event in mouse_motion.read() {
        delta += event.delta;
    }

    for mut transform in query.iter_mut() {
        // Rotate camera when right mouse button is held
        if mouse_button.pressed(MouseButton::Right) && delta != Vec2::ZERO {
            let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
            yaw -= delta.x * sensitivity;
            pitch -= delta.y * sensitivity;
            pitch = pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
            transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
        }

        // WASD movement
        let mut direction = Vec3::ZERO;
        if keyboard.pressed(KeyCode::KeyW) {
            direction += *transform.forward();
        }
        if keyboard.pressed(KeyCode::KeyS) {
            direction += *transform.back();
        }
        if keyboard.pressed(KeyCode::KeyA) {
            direction += *transform.left();
        }
        if keyboard.pressed(KeyCode::KeyD) {
            direction += *transform.right();
        }
        if keyboard.pressed(KeyCode::Space) {
            direction += Vec3::Y;
        }
        if keyboard.pressed(KeyCode::ShiftLeft) {
            direction -= Vec3::Y;
        }
        if direction != Vec3::ZERO {
            transform.translation += direction.normalize() * move_speed * time.delta_secs();
        }
    }
}
