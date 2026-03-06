use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};
use core_graphics2::display::CGDisplay;
use rand::Rng;

use crate::settings::AppSettings;
use crate::virtual_display::VirtualDisplays;
use crate::ScaleFactor;

#[derive(Component)]
pub struct ScreenMarker;

extern "C" {
    fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
}

/// Returns (display_id, CGDisplay) pairs for active displays, up to `max`.
pub fn get_active_displays(max: usize) -> Vec<(u32, CGDisplay)> {
    let mut ids = vec![0u32; max];
    let mut count = 0u32;
    let err = unsafe { CGGetActiveDisplayList(max as u32, ids.as_mut_ptr(), &mut count) };
    if err != 0 {
        return vec![];
    }
    ids.truncate(count as usize);
    ids.into_iter().map(|id| (id, CGDisplay::new(id))).collect()
}

#[derive(Resource)]
pub struct AssetHandles {
    pub screens: Vec<Handle<Image>>,
    /// CGDirectDisplayID for each screen, in the same order as `screens`.
    pub _display_ids: Vec<u32>,
}

pub struct StagePlugin;

impl Plugin for StagePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_stage, spawn_screen));
    }
}

fn spawn_stage(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    _images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    info!("Spawning stage");

    // Ground plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(30.0, 30.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            ..default()
        })),
        Transform::from_xyz(0.0, -4.0, 0.0),
    ));

    // let sphere_texture = Image::new(
    //     Extent3d {
    //         width: 256,
    //         height: 256,
    //         depth_or_array_layers: 1,
    //     },
    //     TextureDimension::D2,
    //     (0..256 * 256)
    //         .flat_map(|i| {
    //             let y = (i / 256) as f32 / 256.0;
    //             let r = 255;
    //             let g = ((1.0 - y) * 255.0) as u8;
    //             let b = 0;
    //             vec![r, g, b, 255]
    //         })
    //         .collect(),
    //     TextureFormat::Rgba8UnormSrgb,
    //     RenderAssetUsages::RENDER_WORLD,
    // );

    // // info!(
    // //     "All image handles BEFORE SPHERE INSERT: {:?}",
    // //     images.ids().collect::<Vec<_>>()
    // // );
    // let sphere_texture_handle = images.add(sphere_texture);
    // // info!("SPHERE texture handle: {:?}", sphere_texture_handle);
    // // info!(
    // //     "All image handles AFTER SPHERE INSERT: {:?}",
    // //     images.ids().collect::<Vec<_>>()
    // // );
    // let sphere_material = materials.add(StandardMaterial {
    //     base_color_texture: Some(sphere_texture_handle),
    //     ..default()
    // });

    // // Test spheres in different positions
    // commands.spawn((
    //     Mesh3d(meshes.add(Sphere::default().mesh())),
    //     MeshMaterial3d(sphere_material),
    //     Transform::from_xyz(0.0, 1.0, -8.0),
    // ));
}

fn spawn_screen(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    scale_factor: Res<ScaleFactor>,
    virtual_displays: Res<VirtualDisplays>,
    settings: Res<AppSettings>,
) {
    info!("Spawning screen");
    let mut rng = rand::thread_rng();

    // Collect (display_id, pixel_width, pixel_height) from virtual or physical displays
    let vd = virtual_displays.displays();
    let screen_specs: Vec<(u32, u32, u32)> = if !vd.is_empty() {
        vd.iter()
            .map(|d| {
                let w = (d.width as f64 * scale_factor.value).round() as u32;
                let h = (d.height as f64 * scale_factor.value).round() as u32;
                (d.display_id, w, h)
            })
            .collect()
    } else {
        let physical = get_active_displays(2);
        let physical = if physical.is_empty() {
            vec![(0u32, CGDisplay::main())]
        } else {
            physical
        };
        physical
            .iter()
            .map(|(id, d)| {
                let w = (d.pixels_wide() as f64 * scale_factor.value).round() as u32;
                let h = (d.pixels_high() as f64 * scale_factor.value).round() as u32;
                (*id, w, h)
            })
            .collect()
    };

    info!("Spawning {} screen(s)", screen_specs.len());
    for (id, w, h) in &screen_specs {
        info!("  display id={}: texture={}x{}", id, w, h);
    }

    let mut screen_handles: Vec<Handle<Image>> = Vec::new();
    let mut display_ids: Vec<u32> = Vec::new();
    for (i, &(display_id, width, height)) in screen_specs.iter().enumerate() {
        let mut screen_texture = Image::new(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            (0..(width * height * 4))
                .map(|j| {
                    if j % 4 == 3 {
                        255
                    } else {
                        rng.gen_range(0..=255)
                    }
                })
                .collect(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );

        screen_texture.texture_descriptor.usage = TextureUsages::COPY_DST
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::STORAGE_BINDING;

        let texture_handle = images.add(screen_texture);
        screen_handles.push(texture_handle.clone());
        display_ids.push(display_id);

        let screen_material = materials.add(StandardMaterial {
            base_color_texture: Some(texture_handle),
            unlit: true,
            alpha_mode: AlphaMode::Opaque,
            ..default()
        });

        let half_width = 2.5;
        let aspect = height as f32 / width as f32;
        let half_height = half_width * aspect;
        let full_width = half_width * 2.0;
        let full_height = half_height * 2.0;

        // Layout: 2 rows of 3, centered around origin
        let cols = 3;
        let gap = 0.3;
        let col = i % cols;
        let row = i / cols;

        let step_x = full_width + gap;
        let total_span_x = step_x * (cols - 1) as f32;
        let x_offset = -total_span_x / 2.0 + step_x * col as f32;

        let step_y = full_height + gap;
        // Row 0 = top, row 1 = bottom
        let y_offset = step_y / 2.0 - step_y * row as f32;

        info!(
            "  plane[{}]: full_size={}x{}, pos=({}, {})",
            i, full_width, full_height, x_offset, y_offset
        );

        commands.spawn((
            Mesh3d(meshes.add(Mesh::from(Plane3d::new(
                Vec3::Z,
                Vec2::new(half_width, half_height),
            )))),
            MeshMaterial3d(screen_material),
            Transform::from_xyz(x_offset, y_offset, -settings.stage_distance),
            ScreenMarker,
        ));
    }

    commands.insert_resource(AssetHandles {
        screens: screen_handles,
        _display_ids: display_ids,
    });
}
