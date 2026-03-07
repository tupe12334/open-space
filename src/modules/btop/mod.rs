mod plugin;

pub(crate) use plugin::BtopPlugin;

use std::sync::Arc;

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};
use core_foundation::base::TCFType as _;
use core_media::sample_buffer::{CMSampleBuffer, CMSampleBufferRef};
use core_video::pixel_buffer::{
    kCVPixelBufferLock_ReadOnly, kCVPixelFormatType_32BGRA, CVPixelBuffer,
};
use dispatch2::{DispatchObject as _, DispatchQueue, DispatchQueueAttr};
use objc2::mutability;
use objc2::{
    declare_class, msg_send_id,
    rc::{Allocated, Id},
    runtime::{AnyObject, ProtocolObject},
    ClassType, DeclaredClass,
};
use objc2_foundation::{NSError, NSObject, NSObjectProtocol};
use screen_capture_kit::{
    shareable_content::SCShareableContent,
    stream::{
        SCContentFilter, SCStream, SCStreamConfiguration, SCStreamDelegate, SCStreamOutput,
        SCStreamOutputType,
    },
};
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::modules::grid_layout::DISPLAY_HALF_HEIGHT;
use crate::modules::settings::AppSettings;
use crate::modules::stage::ScreenMarker;

const BTOP_CAPTURE_WIDTH: usize = 960;
const BTOP_CAPTURE_HEIGHT: usize = 540;
const BTOP_HALF_WIDTH: f32 = 1.5;

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

struct BtopDelegateIvars {
    frame_sender: Arc<Sender<Vec<u8>>>,
}

declare_class!(
    struct BtopDelegate;

    unsafe impl ClassType for BtopDelegate {
        type Super = NSObject;
        type Mutability = mutability::Mutable;
        const NAME: &'static str = "BtopStreamOutputDelegate";
    }

    impl DeclaredClass for BtopDelegate {
        type Ivars = BtopDelegateIvars;
    }

    unsafe impl NSObjectProtocol for BtopDelegate {}

    unsafe impl SCStreamOutput for BtopDelegate {
        #[method(stream:didOutputSampleBuffer:ofType:)]
        unsafe fn stream_did_output_sample_buffer(
            &self,
            _stream: &SCStream,
            sample_buffer: CMSampleBufferRef,
            of_type: SCStreamOutputType,
        ) {
            if of_type != SCStreamOutputType::Screen {
                return;
            }
            let sample_buffer = CMSampleBuffer::wrap_under_get_rule(sample_buffer);
            if let Some(image_buffer) = sample_buffer.get_image_buffer() {
                if let Some(pixel_buffer) = image_buffer.downcast::<CVPixelBuffer>() {
                    pixel_buffer.lock_base_address(kCVPixelBufferLock_ReadOnly);

                    if pixel_buffer.get_pixel_format() != kCVPixelFormatType_32BGRA {
                        warn!("Unexpected pixel format for btop capture");
                        return;
                    }

                    let width = pixel_buffer.get_width();
                    let height = pixel_buffer.get_height();
                    let bytes_per_row = pixel_buffer.get_bytes_per_row();
                    let buffer_size = height * bytes_per_row;
                    let base_address = unsafe { pixel_buffer.get_base_address() };
                    let pixels =
                        std::slice::from_raw_parts(base_address as *const u8, buffer_size);

                    let mut rgba = Vec::with_capacity(width * height * 4);
                    for y in 0..height {
                        for x in 0..width {
                            let src_idx = y * bytes_per_row + x * 4;
                            let b = pixels[src_idx];
                            let g = pixels[src_idx + 1];
                            let r = pixels[src_idx + 2];
                            let a = pixels[src_idx + 3];
                            rgba.extend_from_slice(&[r, g, b, a]);
                        }
                    }

                    pixel_buffer.unlock_base_address(kCVPixelBufferLock_ReadOnly);

                    if let Err(e) = self.ivars().frame_sender.try_send(rgba) {
                        debug!("Dropped btop frame (channel full): {}", e);
                    }
                }
            }
        }
    }

    unsafe impl SCStreamDelegate for BtopDelegate {
        #[method(stream:didStopWithError:)]
        unsafe fn stream_did_stop_with_error(&self, _stream: &SCStream, error: &NSError) {
            error!("Btop stream stopped with error: {:?}", error);
        }
    }
);

impl BtopDelegate {
    fn new(frame_sender: Arc<Sender<Vec<u8>>>) -> Id<Self> {
        let this: Allocated<Self> = Self::alloc();
        unsafe {
            let this = this.set_ivars(BtopDelegateIvars { frame_sender });
            msg_send_id![super(this), init]
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

pub(super) fn setup_btop_capture(frame_channel: Res<BtopFrameChannel>) {
    let sender = Arc::clone(&frame_channel.sender);

    #[expect(
        clippy::infinite_loop,
        reason = "btop capture thread intentionally runs forever"
    )]
    std::thread::spawn(move || {
        info!("Launching btop in Terminal.app...");
        let spawn_result = std::process::Command::new("osascript")
            .args(["-e", "tell application \"Terminal\" to do script \"btop\""])
            .output();

        match spawn_result {
            Ok(output) => {
                if !output.status.success() {
                    error!(
                        "osascript failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    return;
                }
            }
            Err(e) => {
                error!("Failed to launch btop via osascript: {}", e);
                return;
            }
        }

        info!("Waiting for btop Terminal window to appear...");
        std::thread::sleep(std::time::Duration::from_secs(4));

        let (sc_tx, mut sc_rx) = mpsc::channel(1);
        SCShareableContent::get_shareable_content_with_completion_closure(
            move |shareable_content, error| {
                let ret = shareable_content.ok_or_else(|| error.unwrap());
                sc_tx.blocking_send(ret).unwrap();
            },
        );
        let shareable_content = sc_rx.blocking_recv().unwrap();
        if let Err(error) = &shareable_content {
            error!("Failed to get shareable content for btop: {:?}", error);
            return;
        }
        let shareable_content = shareable_content.unwrap();

        let windows = shareable_content.windows();
        info!(
            "Searching for btop window among {} windows...",
            windows.len()
        );

        let btop_window = windows.iter().find(|w| {
            if let (Some(title), Some(app)) = (w.title(), w.owning_application()) {
                let app_name = app.application_name().to_string();
                let title_str = title.to_string();
                return app_name == "Terminal" && title_str.contains("btop");
            }
            false
        });

        let btop_window = if let Some(w) = btop_window {
            info!("Found btop window (id={})", w.window_id());
            w
        } else {
            warn!("Could not find btop window by title, trying newest Terminal window...");
            let terminal_window = windows.iter().find(|w| {
                w.owning_application()
                    .is_some_and(|app| app.application_name().to_string() == "Terminal")
                    && w.on_screen()
            });
            if let Some(w) = terminal_window {
                info!(
                    "Using Terminal window (id={}) as btop fallback",
                    w.window_id()
                );
                w
            } else {
                error!("No Terminal window found for btop capture");
                return;
            }
        };

        let filter = SCContentFilter::init_with_desktop_independent_window(
            SCContentFilter::alloc(),
            btop_window,
        );

        let configuration = SCStreamConfiguration::new();
        configuration.set_width(BTOP_CAPTURE_WIDTH);
        configuration.set_height(BTOP_CAPTURE_HEIGHT);
        configuration.set_minimum_frame_interval(core_media::time::CMTime::make(1, 30));
        configuration.set_pixel_format(kCVPixelFormatType_32BGRA);

        let delegate = BtopDelegate::new(sender);
        let stream_error = ProtocolObject::from_ref(&*delegate);
        let stream =
            SCStream::init_with_filter(SCStream::alloc(), &filter, &configuration, stream_error);

        let queue = DispatchQueue::new("com.open_space.btop.queue", DispatchQueueAttr::SERIAL);
        let output: &ProtocolObject<dyn SCStreamOutput> = ProtocolObject::from_ref(&*delegate);
        let add_result: Result<bool, Id<NSError>> = {
            let mut error: *mut NSError = std::ptr::null_mut();
            let raw_queue = queue.as_raw().as_ptr().cast::<AnyObject>();
            let result: bool = unsafe {
                objc2::msg_send![
                    &*stream,
                    addStreamOutput: output,
                    type: SCStreamOutputType::Screen.0,
                    sampleHandlerQueue: raw_queue,
                    error: &mut error
                ]
            };
            if result {
                Ok(result)
            } else {
                Err(unsafe { Id::retain(error).unwrap() })
            }
        };

        if let Err(e) = add_result {
            error!("Error adding btop stream output: {:?}", e);
            return;
        }

        let _delegate = delegate;

        info!("Starting btop capture...");
        stream.start_capture(|result| {
            if let Some(error) = result {
                warn!("Error starting btop capture: {:?}", error);
            } else {
                info!("Btop capture started successfully");
            }
        });

        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
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
