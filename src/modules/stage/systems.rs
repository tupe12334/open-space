use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};
use core_graphics2::display::CGDisplay;
use rand::Rng as _;

use crate::modules::grid_layout::{
    center_main_display, DISPLAY_ASPECT, DISPLAY_HALF_WIDTH, DISPLAY_HEIGHT, DISPLAY_WIDTH,
    GRID_COLS,
};
use crate::modules::settings::AppSettings;
use crate::modules::virtual_display::VirtualDisplays;
use crate::ScaleFactor;

use super::components::{AssetHandles, ScreenMarker};
use super::display::get_active_displays;

pub(super) fn spawn_stage(
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
}

pub(super) fn spawn_screen(
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
    let mut screen_specs: Vec<(u32, u32, u32)> = if vd.is_empty() {
        let physical = get_active_displays(2);
        let physical = if physical.is_empty() {
            vec![(0_u32, CGDisplay::main())]
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
    } else {
        vd.iter()
            .map(|d| {
                let w = (d.width as f64 * scale_factor.value).round() as u32;
                let h = (d.height as f64 * scale_factor.value).round() as u32;
                (d.display_id, w, h)
            })
            .collect()
    };

    // Always include the main Mac display at the standard resolution
    let main_display = CGDisplay::main();
    let main_id = main_display.id;
    if !screen_specs.iter().any(|(id, _, _)| *id == main_id) {
        screen_specs.push((main_id, DISPLAY_WIDTH, DISPLAY_HEIGHT));
    }

    // Reorder so the main Mac display is at the center of the grid
    center_main_display(&mut screen_specs, main_id, |&(id, _, _)| id);

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

        screen_texture.texture_descriptor.usage =
            TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING;

        let texture_handle = images.add(screen_texture);
        screen_handles.push(texture_handle.clone());
        display_ids.push(display_id);

        let screen_material = materials.add(StandardMaterial {
            base_color_texture: Some(texture_handle),
            unlit: true,
            alpha_mode: AlphaMode::Opaque,
            ..default()
        });

        let half_width = DISPLAY_HALF_WIDTH;
        let half_height = half_width * DISPLAY_ASPECT;
        let full_width = half_width * 2.0;
        let full_height = half_height * 2.0;

        // Layout: seamless grid, no gaps so displays merge into one
        let cols = GRID_COLS;
        let gap = 0.0;
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
        display_ids,
    });
}
