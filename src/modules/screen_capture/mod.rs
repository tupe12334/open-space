mod permission;
mod plugin;

pub(crate) use permission::ensure_screen_capture_permission;
pub(crate) use plugin::ScreenCapturePlugin;

use std::sync::Arc;

use bevy::prelude::*;
use core_foundation::base::TCFType as _;
use core_graphics2::display::CGDisplay;
use core_media::sample_buffer::{CMSampleBuffer, CMSampleBufferRef};
use core_video::pixel_buffer::{
    kCVPixelBufferLock_ReadOnly, kCVPixelFormatType_32BGRA, CVPixelBuffer,
};
use dispatch2::{DispatchObject as _, DispatchQueue, DispatchQueueAttr};
use libc::size_t;
use objc2::mutability;
use objc2::{
    declare_class, msg_send_id,
    rc::{Allocated, Id},
    runtime::{AnyObject, ProtocolObject},
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

use crate::modules::grid_layout::center_main_display;
use crate::modules::stage::{get_active_displays, AssetHandles};
use crate::modules::virtual_display::VirtualDisplays;
use crate::ScaleFactor;

#[derive(Resource)]
pub(super) struct FrameChannel {
    pub(super) senders: Vec<Arc<Sender<Vec<u8>>>>,
    pub(super) receivers: Vec<Receiver<Vec<u8>>>,
}

pub(crate) struct DelegateIvars {
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
                        warn!("Unexpected pixel format");
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
                        debug!("Dropped frame (channel full): {}", e);
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
            error!("Stream stopped with error: {:?}", error);
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
    pub(crate) fn new(frame_sender: Arc<Sender<Vec<u8>>>) -> Id<Self> {
        let this: Allocated<Self> = Self::alloc();
        unsafe {
            let this = this.set_ivars(DelegateIvars { frame_sender });
            msg_send_id![super(this), init]
        }
    }
}

pub(super) fn setup_screen_capture(
    frame_channel: Res<FrameChannel>,
    scale_factor: Res<ScaleFactor>,
    virtual_displays: Res<VirtualDisplays>,
) {
    let senders: Vec<Arc<Sender<Vec<u8>>>> = frame_channel.senders.clone();

    // Collect (display_id, capture_width, capture_height)
    let vd = virtual_displays.displays();
    let mut display_specs: Vec<(u32, usize, usize)> = if vd.is_empty() {
        get_active_displays(2)
            .iter()
            .map(|(id, d)| {
                let w = (d.pixels_wide() as f64 * scale_factor.value) as usize;
                let h = (d.pixels_high() as f64 * scale_factor.value) as usize;
                (*id, w, h)
            })
            .collect()
    } else {
        vd.iter()
            .map(|d| {
                let w = (d.width as f64 * scale_factor.value) as usize;
                let h = (d.height as f64 * scale_factor.value) as usize;
                (d.display_id, w, h)
            })
            .collect()
    };

    // Always include the main Mac display
    let main_display = CGDisplay::main();
    let main_id = main_display.id;
    if !display_specs.iter().any(|(id, _, _)| *id == main_id) {
        let main_w = (main_display.pixels_wide() as f64 * scale_factor.value) as usize;
        let main_h = (main_display.pixels_high() as f64 * scale_factor.value) as usize;
        display_specs.push((main_id, main_w, main_h));
    }

    // Reorder so the main Mac display is at the center of the grid
    center_main_display(&mut display_specs, main_id, |&(id, _, _)| id);

    #[expect(
        clippy::infinite_loop,
        reason = "capture thread intentionally runs forever"
    )]
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
            error!("Failed to get shareable content: {:?}", error);
            return;
        }
        let shareable_content = shareable_content.unwrap();

        let sc_displays = shareable_content.displays();
        if sc_displays.is_empty() {
            warn!("No display found for screen capture");
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
        let mut delegates: Vec<Id<Delegate>> = Vec::new();
        let mut streams: Vec<Id<SCStream>> = Vec::new();

        // Match each target display ID to its SCDisplay and sender
        for (sender_idx, &(target_id, cap_w, cap_h)) in display_specs.iter().enumerate() {
            let sc_display = sc_displays.iter().find(|d| d.display_id() == target_id);
            let sc_display = if let Some(d) = sc_display {
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
            } else {
                warn!("SCDisplay not found for display ID {}", target_id);
                continue;
            };
            let sender = Arc::clone(&senders[sender_idx]);

            let filter = SCContentFilter::init_with_display_exclude_windows(
                SCContentFilter::alloc(),
                sc_display,
                &NSArray::new(),
            );

            let capture_width: size_t = cap_w;
            let capture_height: size_t = cap_h;
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
            let queue = DispatchQueue::new(
                &format!("com.spatial_display.queue.{sender_idx}"),
                DispatchQueueAttr::SERIAL,
            );
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
            if let Err(ret) = add_result {
                error!(
                    "Error adding output for display {} (ID {}): {:?}",
                    sender_idx, target_id, ret
                );
                continue;
            }

            delegates.push(delegate);
            streams.push(stream);
        }

        info!("Waiting 5 seconds before starting capture...");
        std::thread::sleep(std::time::Duration::from_secs(5));

        info!("STARTING CAP for {} display(s)", streams.len());
        for (idx, stream) in streams.iter().enumerate() {
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

pub(super) fn update_screen_texture(
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
        // Drain all pending frames and keep only the latest
        let mut latest = None;
        while let Ok(frame_data) = frame_channel.receivers[i].try_recv() {
            latest = Some(frame_data);
        }
        if let Some(frame_data) = latest {
            if let Some(image) = images.get_mut(&asset_handles.screens[i]) {
                image.data = Some(frame_data);
            }
        }
    }

    // Touch materials to force texture update
    for (_, _material) in materials.iter_mut() {}
}
