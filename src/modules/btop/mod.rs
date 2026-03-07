mod plugin;

pub(crate) use plugin::BtopPlugin;

use std::collections::HashMap;
use std::io::Read as _;
use std::sync::{Arc, Mutex};

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::modules::grid_layout::DISPLAY_HALF_HEIGHT;
use crate::modules::settings::AppSettings;
use crate::modules::stage::ScreenMarker;

const TERM_COLS: u16 = 120;
const TERM_ROWS: u16 = 40;
const CELL_WIDTH: usize = 8;
const CELL_HEIGHT: usize = 14;
const BTOP_CAPTURE_WIDTH: usize = TERM_COLS as usize * CELL_WIDTH;
const BTOP_CAPTURE_HEIGHT: usize = TERM_ROWS as usize * CELL_HEIGHT;
const BTOP_HALF_WIDTH: f32 = 1.5;
const FONT_SIZE: f32 = CELL_HEIGHT as f32;

#[derive(Resource)]
pub(super) struct BtopFrameChannel {
    pub(super) sender: Arc<Sender<Vec<u8>>>,
    pub(super) receiver: Receiver<Vec<u8>>,
}

#[derive(Resource)]
pub(super) struct BtopTextureHandle {
    pub(super) handle: Handle<Image>,
}

#[derive(Component)]
struct BtopMarker;

pub(super) fn setup_btop_capture(frame_channel: Res<BtopFrameChannel>) {
    let sender = Arc::clone(&frame_channel.sender);

    #[expect(
        clippy::infinite_loop,
        reason = "btop render thread intentionally runs forever"
    )]
    std::thread::spawn(move || {
        info!("Loading font for btop rendering...");
        let font_data =
            std::fs::read("/System/Library/Fonts/Menlo.ttc").expect("Failed to load Menlo font");
        let font = fontdue::Font::from_bytes(
            font_data,
            fontdue::FontSettings {
                collection_index: 0,
                ..fontdue::FontSettings::default()
            },
        )
        .expect("Failed to parse Menlo font");

        let baseline = font
            .horizontal_line_metrics(FONT_SIZE)
            .map_or((FONT_SIZE * 0.8) as i32, |m| m.ascent as i32);

        info!("Starting btop in PTY ({}x{})...", TERM_COLS, TERM_ROWS);
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: TERM_ROWS,
                cols: TERM_COLS,
                pixel_width: BTOP_CAPTURE_WIDTH as u16,
                pixel_height: BTOP_CAPTURE_HEIGHT as u16,
            })
            .expect("Failed to open PTY");

        let config_path = std::env::current_dir()
            .expect("failed to get current dir")
            .join("assets/btop.conf");

        let mut cmd = CommandBuilder::new("btop");
        cmd.arg("--config");
        cmd.arg(config_path);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("LANG", "en_US.UTF-8");

        let master = pair.master;
        let slave = pair.slave;

        let _child = slave.spawn_command(cmd).expect("Failed to spawn btop");
        drop(slave);

        let mut reader = master
            .try_clone_reader()
            .expect("Failed to clone PTY reader");

        let parser = Arc::new(Mutex::new(vt100::Parser::new(TERM_ROWS, TERM_COLS, 0)));

        // Reader thread: reads PTY output and feeds it to the terminal parser
        let parser_for_reader = Arc::clone(&parser);
        std::thread::spawn(move || {
            let mut buf = [0_u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Ok(mut p) = parser_for_reader.lock() {
                            p.process(&buf[..n]);
                        }
                    }
                    Err(e) => {
                        error!("btop PTY read error: {}", e);
                        break;
                    }
                }
            }
            info!("btop PTY reader exited");
        });

        // Keep master handle alive so the PTY stays open
        let _master = master;

        info!("btop render loop starting...");
        let mut glyph_cache = HashMap::new();
        let frame_interval = std::time::Duration::from_millis(33);

        loop {
            std::thread::sleep(frame_interval);

            let rgba = {
                let Ok(p) = parser.lock() else {
                    continue;
                };
                render_screen(p.screen(), &font, baseline, &mut glyph_cache)
            };

            if let Err(e) = sender.try_send(rgba) {
                debug!("Dropped btop frame (channel full): {}", e);
            }
        }
    });
}

fn render_screen(
    screen: &vt100::Screen,
    font: &fontdue::Font,
    baseline: i32,
    cache: &mut HashMap<char, (fontdue::Metrics, Vec<u8>)>,
) -> Vec<u8> {
    let mut rgba = vec![0_u8; BTOP_CAPTURE_WIDTH * BTOP_CAPTURE_HEIGHT * 4];

    // Fill alpha channel to 255 (opaque black background)
    let mut i = 3;
    while i < rgba.len() {
        rgba[i] = 255;
        i += 4;
    }

    for row in 0..TERM_ROWS {
        for col in 0..TERM_COLS {
            let Some(cell) = screen.cell(row, col) else {
                continue;
            };

            let fg = vt100_color_to_rgb(cell.fgcolor(), true);
            let bg = vt100_color_to_rgb(cell.bgcolor(), false);

            let x_off = usize::from(col) * CELL_WIDTH;
            let y_off = usize::from(row) * CELL_HEIGHT;

            fill_cell_bg(&mut rgba, x_off, y_off, bg);

            let contents = cell.contents();
            if let Some(ch) = contents.chars().next() {
                if ch > ' ' {
                    let (metrics, bitmap) = cache
                        .entry(ch)
                        .or_insert_with(|| font.rasterize(ch, FONT_SIZE));
                    draw_glyph(&mut rgba, bitmap, metrics, x_off, y_off, baseline, fg, bg);
                }
            }
        }
    }

    rgba
}

fn fill_cell_bg(rgba: &mut [u8], x_off: usize, y_off: usize, bg: (u8, u8, u8)) {
    for cy in 0..CELL_HEIGHT {
        for cx in 0..CELL_WIDTH {
            let px = x_off + cx;
            let py = y_off + cy;
            let idx = (py * BTOP_CAPTURE_WIDTH + px) * 4;
            if let Some(pixel) = rgba.get_mut(idx..idx + 4) {
                pixel.copy_from_slice(&[bg.0, bg.1, bg.2, 255]);
            }
        }
    }
}

fn draw_glyph(
    rgba: &mut [u8],
    bitmap: &[u8],
    metrics: &fontdue::Metrics,
    x_off: usize,
    y_off: usize,
    baseline: i32,
    fg: (u8, u8, u8),
    bg: (u8, u8, u8),
) {
    if metrics.width == 0 || metrics.height == 0 {
        return;
    }

    let glyph_top = baseline - metrics.ymin - metrics.height as i32;
    let glyph_left = metrics.xmin;

    for gy in 0..metrics.height {
        for gx in 0..metrics.width {
            let px = x_off as i32 + glyph_left + gx as i32;
            let py = y_off as i32 + glyph_top + gy as i32;

            if px < 0
                || py < 0
                || px >= BTOP_CAPTURE_WIDTH as i32
                || py >= BTOP_CAPTURE_HEIGHT as i32
            {
                continue;
            }

            let Some(&alpha) = bitmap.get(gy * metrics.width + gx) else {
                continue;
            };
            if alpha == 0 {
                continue;
            }

            let idx = (py as usize * BTOP_CAPTURE_WIDTH + px as usize) * 4;
            let Some(pixel) = rgba.get_mut(idx..idx + 4) else {
                continue;
            };

            if alpha == 255 {
                pixel.copy_from_slice(&[fg.0, fg.1, fg.2, 255]);
            } else {
                let a = f32::from(alpha) / 255.0;
                let inv_a = 1.0 - a;
                pixel.copy_from_slice(&[
                    f32::from(fg.0).mul_add(a, f32::from(bg.0) * inv_a) as u8,
                    f32::from(fg.1).mul_add(a, f32::from(bg.1) * inv_a) as u8,
                    f32::from(fg.2).mul_add(a, f32::from(bg.2) * inv_a) as u8,
                    255,
                ]);
            }
        }
    }
}

fn vt100_color_to_rgb(color: vt100::Color, is_fg: bool) -> (u8, u8, u8) {
    match color {
        vt100::Color::Default => {
            if is_fg {
                (204, 204, 204)
            } else {
                (0, 0, 0)
            }
        }
        vt100::Color::Idx(i) => ansi_256_to_rgb(i),
        vt100::Color::Rgb(r, g, b) => (r, g, b),
    }
}

fn ansi_256_to_rgb(idx: u8) -> (u8, u8, u8) {
    match idx {
        0 => (0, 0, 0),
        1 => (170, 0, 0),
        2 => (0, 170, 0),
        3 => (170, 85, 0),
        4 => (0, 0, 170),
        5 => (170, 0, 170),
        6 => (0, 170, 170),
        7 => (170, 170, 170),
        8 => (85, 85, 85),
        9 => (255, 85, 85),
        10 => (85, 255, 85),
        11 => (255, 255, 85),
        12 => (85, 85, 255),
        13 => (255, 85, 255),
        14 => (85, 255, 255),
        15 => (255, 255, 255),
        16..=231 => {
            let i = idx - 16;
            let r = (i / 36) % 6;
            let g = (i / 6) % 6;
            let b = i % 6;
            let to_val = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            (to_val(r), to_val(g), to_val(b))
        }
        232..=255 => {
            let v = 8 + 10 * (idx - 232);
            (v, v, v)
        }
    }
}

pub(super) fn spawn_btop_panel(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    settings: Res<AppSettings>,
    screens: Query<&Transform, With<ScreenMarker>>,
) {
    let min_y = screens
        .iter()
        .map(|t| t.translation.y)
        .reduce(f32::min)
        .unwrap_or(0.0);

    let width = BTOP_CAPTURE_WIDTH as u32;
    let height = BTOP_CAPTURE_HEIGHT as u32;
    let mut btop_texture = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0_u8; (width * height * 4) as usize],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    btop_texture.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING;

    let texture_handle = images.add(btop_texture);

    let btop_material = materials.add(StandardMaterial {
        base_color_texture: Some(texture_handle.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Opaque,
        ..default()
    });

    let half_width = BTOP_HALF_WIDTH;
    let aspect = height as f32 / width as f32;
    let half_height = half_width * aspect;

    let y_pos = min_y - DISPLAY_HALF_HEIGHT - half_height;

    info!(
        "Spawning btop panel at (0, {}, -{}), size={}x{}",
        y_pos,
        settings.stage_distance,
        half_width * 2.0,
        half_height * 2.0,
    );

    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Plane3d::new(
            Vec3::Z,
            Vec2::new(half_width, half_height),
        )))),
        MeshMaterial3d(btop_material),
        Transform::from_xyz(0.0, y_pos, -settings.stage_distance),
        BtopMarker,
    ));

    commands.insert_resource(BtopTextureHandle {
        handle: texture_handle,
    });
}

pub(super) fn update_btop_texture(
    mut frame_channel: ResMut<BtopFrameChannel>,
    mut images: ResMut<Assets<Image>>,
    btop_handle: Option<Res<BtopTextureHandle>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(btop_handle) = btop_handle else {
        return;
    };

    let mut latest = None;
    while let Ok(frame_data) = frame_channel.receiver.try_recv() {
        latest = Some(frame_data);
    }
    if let Some(frame_data) = latest {
        if let Some(image) = images.get_mut(&btop_handle.handle) {
            image.data = Some(frame_data);
        }
    }

    // Touch materials to force texture update
    for (_, _material) in materials.iter_mut() {}
}
