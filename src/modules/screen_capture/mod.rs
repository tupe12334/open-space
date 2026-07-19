mod permission;
mod plugin;

pub(crate) use permission::ensure_screen_capture_permission;
pub(crate) use plugin::ScreenCapturePlugin;

use std::sync::atomic::{AtomicBool, Ordering};
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

use crate::modules::grid_layout::{center_main_display, DISPLAY_HEIGHT, DISPLAY_WIDTH};
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
    stream_stopped: Arc<AtomicBool>,
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
                    pixel_buffer.lock_base_address(kCVPixelBufferLock_ReadOnly);

                    if pixel_buffer.get_pixel_format() != kCVPixelFormatType_32BGRA {
                        warn!("Unexpected pixel format");
                        pixel_buffer.unlock_base_address(kCVPixelBufferLock_ReadOnly);
                        return;
                    }

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
                }
            }
        }
    }

    unsafe impl SCStreamDelegate for Delegate {
        #[method(stream:didStopWithError:)]
        unsafe fn stream_did_stop_with_error(&self, _stream: &SCStream, error: &NSError) {
            error!("Stream stopped with error: {:?}", error);
            self.ivars().stream_stopped.store(true, Ordering::SeqCst);
        }
    }
);

impl Delegate {
    pub(crate) fn new(
        frame_sender: Arc<Sender<Vec<u8>>>,
        stream_stopped: Arc<AtomicBool>,
    ) -> Id<Self> {
        let this: Allocated<Self> = Self::alloc();
        unsafe {
            let this = this.set_ivars(DelegateIvars {
                frame_sender,
                stream_stopped,
            });
            msg_send_id![super(this), init]
        }
    }
}

/// Query `SCShareableContent`, create streams for each display, and start capture.
/// Returns `(delegates, streams, error_flags)` on success, or `None` if setup fails.
fn create_streams(
    display_specs: &[(u32, usize, usize)],
    senders: &[Arc<Sender<Vec<u8>>>],
) -> Option<(Vec<Id<Delegate>>, Vec<Id<SCStream>>, Vec<Arc<AtomicBool>>)> {
    let (sc_tx, mut sc_rx) = mpsc::channel(1);
    SCShareableContent::get_shareable_content_with_completion_closure(
        move |shareable_content, error| {
            let ret = shareable_content.ok_or_else(|| error.unwrap());
            sc_tx.blocking_send(ret).unwrap();
        },
    );
    let shareable_content = match sc_rx.blocking_recv()? {
        Ok(sc) => sc,
        Err(error) => {
            error!("Failed to get shareable content: {:?}", error);
            return None;
        }
    };

    let sc_displays = shareable_content.displays();
    if sc_displays.is_empty() {
        warn!("No display found for screen capture");
        return None;
    }

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

    let mut delegates: Vec<Id<Delegate>> = Vec::new();
    let mut streams: Vec<Id<SCStream>> = Vec::new();
    let mut error_flags: Vec<Arc<AtomicBool>> = Vec::new();

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
        let stopped_flag = Arc::new(AtomicBool::new(false));

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

        let delegate = Delegate::new(sender, Arc::clone(&stopped_flag));
        let stream_error = ProtocolObject::from_ref(&*delegate);
        let stream =
            SCStream::init_with_filter(SCStream::alloc(), &filter, &configuration, stream_error);
        let queue = DispatchQueue::new(
            &format!("com.spatial_display.queue.{sender_idx}"),
            DispatchQueueAttr::SERIAL,
        );
        let output: &ProtocolObject<dyn SCStreamOutput> = ProtocolObject::from_ref(&*delegate);
        let added: bool = {
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
            if !result {
                let err = unsafe { Id::retain(error) };
                error!(
                    "Error adding output for display {} (ID {}): {:?}",
                    sender_idx, target_id, err
                );
            }
            result
        };
        if !added {
            continue;
        }

        delegates.push(delegate);
        streams.push(stream);
        error_flags.push(stopped_flag);
    }

    if streams.is_empty() {
        return None;
    }

    info!("Starting capture for {} display(s)", streams.len());
    for (idx, stream) in streams.iter().enumerate() {
        stream.start_capture(move |result| {
            info!("start_capture callback for display {}", idx);
            if let Some(error) = result {
                warn!("error starting display {}: {:?}", idx, error);
            }
        });
    }

    Some((delegates, streams, error_flags))
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

    // Always include the main Mac display at the standard resolution
    let main_display = CGDisplay::main();
    let main_id = main_display.id;
    if !display_specs.iter().any(|(id, _, _)| *id == main_id) {
        display_specs.push((main_id, DISPLAY_WIDTH as usize, DISPLAY_HEIGHT as usize));
    }

    // Reorder so the main Mac display is at the center of the grid
    center_main_display(&mut display_specs, main_id, |&(id, _, _)| id);

    std::thread::spawn(move || -> ! {
        info!("Waiting 5 seconds before starting capture...");
        std::thread::sleep(std::time::Duration::from_secs(5));

        loop {
            info!("(Re)initializing screen capture streams...");
            let Some((_delegates, _streams, error_flags)) =
                create_streams(&display_specs, &senders)
            else {
                warn!("Failed to create streams, retrying in 5 seconds...");
                std::thread::sleep(std::time::Duration::from_secs(5));
                continue;
            };

            // Monitor for stream errors and restart when detected
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if error_flags.iter().any(|f| f.load(Ordering::SeqCst)) {
                    warn!("Stream error detected, restarting capture in 2 seconds...");
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    break;
                }
            }
            // _delegates and _streams are dropped here, cleaning up the old capture
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
