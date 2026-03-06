use std::sync::Arc;

use bevy::prelude::*;
use core_foundation::base::TCFType;
use core_media::sample_buffer::{CMSampleBuffer, CMSampleBufferRef};
use core_video::pixel_buffer::{
    kCVPixelBufferLock_ReadOnly, kCVPixelFormatType_32BGRA, CVPixelBuffer,
};
use dispatch2::{Queue, QueueAttribute};
use libc::size_t;
use objc2::mutability;
use objc2::{
    declare_class, msg_send_id,
    rc::{Allocated, Id},
    runtime::ProtocolObject,
    ClassType, DeclaredClass,
};
use objc2_foundation::{NSArray, NSError, NSObject, NSObjectProtocol};
use screen_capture_kit::{
    shareable_content::SCShareableContent,
    stream::{
        SCContentFilter, SCStream, SCStreamConfiguration, SCStreamDelegate, SCStreamOutput,
        SCStreamOutputType,
    },
};
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::stage::{get_active_displays, AssetHandles};
use crate::virtual_display::VirtualDisplays;
use crate::ScaleFactor;

#[derive(Resource)]
struct FrameChannel {
    senders: Vec<Arc<Sender<Vec<u8>>>>,
    receivers: Vec<Receiver<Vec<u8>>>,
}

pub struct ScreenCapturePlugin;

impl Plugin for ScreenCapturePlugin {
    fn build(&self, app: &mut App) {
        // Pre-allocate 2 channels; unused ones stay empty
        let mut senders = Vec::new();
        let mut receivers = Vec::new();
        for _ in 0..6 {
            let (tx, rx) = mpsc::channel::<Vec<u8>>(60);
            senders.push(Arc::new(tx));
            receivers.push(rx);
        }
        app.insert_resource(FrameChannel { senders, receivers })
            .add_systems(Startup, setup_screen_capture)
            .add_systems(Update, update_screen_texture);
    }
}

pub struct DelegateIvars {
    frame_sender: Arc<Sender<Vec<u8>>>,
}

declare_class!(
    struct Delegate;

    unsafe impl ClassType for Delegate {
        type Super = NSObject;
        type Mutability = mutability::Mutable;
        const NAME: &'static str = "StreamOutputSampleBufferDelegate";
    }

    impl DeclaredClass for Delegate {
        type Ivars = DelegateIvars;
    }

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl SCStreamOutput for Delegate {
        #[method(stream:didOutputSampleBuffer:ofType:)]
        unsafe fn stream_did_output_sample_buffer(&self, _stream: &SCStream, sample_buffer: CMSampleBufferRef, of_type: SCStreamOutputType) {
            if of_type != SCStreamOutputType::Screen {
                return;
            }
            let sample_buffer = CMSampleBuffer::wrap_under_get_rule(sample_buffer);
            if let Some(image_buffer) = sample_buffer.get_image_buffer() {
                if let Some(pixel_buffer) = image_buffer.downcast::<CVPixelBuffer>() {
                    // Lock the base address of the pixel buffer
                    pixel_buffer.lock_base_address(kCVPixelBufferLock_ReadOnly);

                    // println!("frame_sender: {:?}", self.ivars().frame_sender);

                    if pixel_buffer.get_pixel_format() != kCVPixelFormatType_32BGRA {
                        println!("Unexpected pixel format");
                        return;
                    }
                    // let _ = self.ivars().frame_sender.send(rgba_data);

                    let width = pixel_buffer.get_width();
                    let height = pixel_buffer.get_height();
                    let bytes_per_row = pixel_buffer.get_bytes_per_row();
                    let buffer_size = height * bytes_per_row;
                    let base_address = unsafe { pixel_buffer.get_base_address() };
                    let pixels = std::slice::from_raw_parts(
                        base_address as *const u8,
                        buffer_size
                    );

                    // Create RGBA buffer with pre-allocated capacity
                    let mut rgba = Vec::with_capacity(width * height * 4);
                    for y in 0..height {
                        for x in 0..width {
                            let src_idx = y * bytes_per_row + x * 4;
                            // BGRA to RGBA conversion
                            let b = pixels[src_idx];
                            let g = pixels[src_idx + 1];
                            let r = pixels[src_idx + 2];
                            let a = pixels[src_idx + 3];
                            rgba.extend_from_slice(&[r, g, b, a]);
                        }
                    }

                    pixel_buffer.unlock_base_address(kCVPixelBufferLock_ReadOnly);

                    if let Err(e) = self.ivars().frame_sender.try_send(rgba) {
                        error!("Failed to send frame data: {}", e);
                    }

                    // println!("base address: {:?}", base_address);
                    // println!("pixel buffer: {:?}", pixel_buffer);
                    // println!("pixel format: {}", pixel_buffer.get_pixel_format());
                    // println!("width: {}, height: {}, bytes_per_row: {}", width, height, bytes_per_row);
                    // println!("pixels: {:?}", pixels);

                    // // Get plane 0 (Y plane)
                    // let y_plane_base = pixel_buffer.get_base_address_of_plane(0);
                    // let y_plane_bytes_per_row = pixel_buffer.get_bytes_per_row_of_plane(0);
                    // let y_plane_height = pixel_buffer.get_height_of_plane(0);
                    // let y_plane = slice::from_raw_parts(
                    //     y_plane_base as *const u8,
                    //     y_plane_height * y_plane_bytes_per_row
                    // );

                    // // Get plane 1 (UV plane)
                    // let uv_plane_base = pixel_buffer.get_base_address_of_plane(1);
                    // let uv_plane_bytes_per_row = pixel_buffer.get_bytes_per_row_of_plane(1);
                    // let uv_plane_height = pixel_buffer.get_height_of_plane(1);
                    // let uv_plane = slice::from_raw_parts(
                    //     uv_plane_base as *const u8,
                    //     uv_plane_height * uv_plane_bytes_per_row
                    // );

                    // // Now save or process the image using both planes
                    // save_yuv_as_png(y_plane, uv_plane, width, height,
                    //             y_plane_bytes_per_row, uv_plane_bytes_per_row);

                    // Unlock the base address when done
                    // pixel_buffer.unlock_base_address(kCVPixelBufferLock_ReadOnly);
                }
            }
        }
    }

    unsafe impl SCStreamDelegate for Delegate {
        #[method(stream:didStopWithError:)]
        unsafe fn stream_did_stop_with_error(&self, _stream: &SCStream, error: &NSError) {
            println!("error: {:?}", error);
        }
    }

    // unsafe impl Delegate {
    //     #[method_id(init)]
    //     fn init(this: Allocated<Self>) -> Option<Id<Self>> {
    //         let this = this.set_ivars(DelegateIvars {});
    //         unsafe { msg_send_id![super(this), init] }
    //     }
    // }
);

impl Delegate {
    pub fn new(frame_sender: Arc<Sender<Vec<u8>>>) -> Id<Self> {
        let this: Allocated<Self> = Self::alloc();
        unsafe {
            let this = this.set_ivars(DelegateIvars { frame_sender });
            msg_send_id![super(this), init]
        }
    }
}

fn setup_screen_capture(
    frame_channel: Res<FrameChannel>,
    scale_factor: Res<ScaleFactor>,
    virtual_displays: Res<VirtualDisplays>,
) {
    let senders: Vec<Arc<Sender<Vec<u8>>>> = frame_channel.senders.clone();

    // Collect (display_id, capture_width, capture_height)
    let vd = virtual_displays.displays();
    let display_specs: Vec<(u32, usize, usize)> = if !vd.is_empty() {
        vd.iter()
            .map(|d| {
                let w = (d.width as f64 * scale_factor.value) as usize;
                let h = (d.height as f64 * scale_factor.value) as usize;
                (d.display_id, w, h)
            })
            .collect()
    } else {
        get_active_displays(2)
            .iter()
            .map(|(id, d)| {
                let w = (d.pixels_wide() as f64 * scale_factor.value) as usize;
                let h = (d.pixels_high() as f64 * scale_factor.value) as usize;
                (*id, w, h)
            })
            .collect()
    };

    std::thread::spawn(move || {
        let (sc_tx, mut sc_rx) = mpsc::channel(1);
        SCShareableContent::get_shareable_content_with_completion_closure(
            move |shareable_content, error| {
                let ret = shareable_content.ok_or_else(|| error.unwrap());
                sc_tx.blocking_send(ret).unwrap();
            },
        );
        let shareable_content = sc_rx.blocking_recv().unwrap();
        if let Err(error) = shareable_content {
            println!("error: {:?}", error);
            return;
        }
        let shareable_content = shareable_content.unwrap();

        let sc_displays = shareable_content.displays();
        if sc_displays.is_empty() {
            println!("no display found");
            return;
        }

        // Log all available SCDisplays
        info!("SCShareableContent has {} display(s):", sc_displays.len());
        for (i, d) in sc_displays.iter().enumerate() {
            info!(
                "  SC display [{}]: id={}, size={}x{}",
                i,
                d.display_id(),
                d.width(),
                d.height()
            );
        }
        let target_ids: Vec<u32> = display_specs.iter().map(|(id, _, _)| *id).collect();
        info!("Target display IDs: {:?}", target_ids);

        // We need to keep delegates and streams alive for the duration of capture
        let mut _delegates: Vec<Id<Delegate>> = Vec::new();
        let mut _streams: Vec<Id<SCStream>> = Vec::new();

        // Match each target display ID to its SCDisplay and sender
        for (sender_idx, &(target_id, cap_w, cap_h)) in display_specs.iter().enumerate() {
            let sc_display = sc_displays.iter().find(|d| d.display_id() == target_id);
            let sc_display = match sc_display {
                Some(d) => {
                    info!(
                        "  Matched sender[{}] -> SC display id={}, sc_size={}x{}, capture={}x{}",
                        sender_idx,
                        d.display_id(),
                        d.width(),
                        d.height(),
                        cap_w,
                        cap_h,
                    );
                    d
                }
                None => {
                    warn!("SCDisplay not found for display ID {}", target_id);
                    continue;
                }
            };
            let sender = senders[sender_idx].clone();

            let filter = SCContentFilter::init_with_display_exclude_windows(
                SCContentFilter::alloc(),
                sc_display,
                &NSArray::new(),
            );

            let capture_width = cap_w as size_t;
            let capture_height = cap_h as size_t;
            let configuration: Id<SCStreamConfiguration> = SCStreamConfiguration::new();
            configuration.set_width(capture_width);
            configuration.set_height(capture_height);
            configuration.set_minimum_frame_interval(core_media::time::CMTime::make(1, 60));
            configuration.set_pixel_format(kCVPixelFormatType_32BGRA);

            let delegate = Delegate::new(sender);
            let stream_error = ProtocolObject::from_ref(&*delegate);
            let stream = SCStream::init_with_filter(
                SCStream::alloc(),
                &filter,
                &configuration,
                stream_error,
            );
            let queue = Queue::new(
                &format!("com.spatial_display.queue.{}", sender_idx),
                QueueAttribute::Serial,
            );
            let output = ProtocolObject::from_ref(&*delegate);
            if let Err(ret) = stream.add_stream_output(output, SCStreamOutputType::Screen, &queue) {
                println!(
                    "error adding output for display {} (ID {}): {:?}",
                    sender_idx, target_id, ret
                );
                continue;
            }

            _delegates.push(delegate);
            _streams.push(stream);
        }

        info!("Waiting 5 seconds before starting capture...");
        std::thread::sleep(std::time::Duration::from_secs(5));

        info!("STARTING CAP for {} display(s)", _streams.len());
        for (i, stream) in _streams.iter().enumerate() {
            let idx = i;
            stream.start_capture(move |result| {
                info!("start_capture for display {}", idx);
                if let Some(error) = result {
                    warn!("error starting display {}: {:?}", idx, error);
                }
            });
        }

        info!("🔄 Entering capture loop");
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
}

fn update_screen_texture(
    mut frame_channel: ResMut<FrameChannel>,
    mut images: ResMut<Assets<Image>>,
    asset_handles: Res<AssetHandles>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let count = frame_channel
        .receivers
        .len()
        .min(asset_handles.screens.len());
    for i in 0..count {
        if let Ok(frame_data) = frame_channel.receivers[i].try_recv() {
            if let Some(image) = images.get_mut(&asset_handles.screens[i]) {
                image.data = frame_data;
            }
        }
    }

    // Touch materials to force texture update
    for (_, mut _material) in materials.iter_mut() {}
}
