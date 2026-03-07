mod detection;
mod permission;
mod plugin;
mod types;

pub(crate) use permission::ensure_camera_permission;
pub(crate) use plugin::WebcamDistancePlugin;

use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use dispatch2::{DispatchObject as _, DispatchQueue, DispatchQueueAttr};
use objc2::mutability;
use objc2::rc::{Allocated, Id};
use objc2::runtime::{AnyClass, AnyObject, Bool};
use objc2::{declare_class, msg_send, msg_send_id, ClassType, DeclaredClass};
use objc2_foundation::{NSError, NSObject, NSObjectProtocol, NSString};

use types::DistanceStore;

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {}

#[link(name = "Vision", kind = "framework")]
unsafe extern "C" {}

pub(crate) struct CameraDelegateIvars {
    distance: Arc<Mutex<Option<f32>>>,
}

declare_class!(
    struct CameraDelegate;

    unsafe impl ClassType for CameraDelegate {
        type Super = NSObject;
        type Mutability = mutability::Mutable;
        const NAME: &'static str = "WebcamDistanceCameraDelegate";
    }

    impl DeclaredClass for CameraDelegate {
        type Ivars = CameraDelegateIvars;
    }

    unsafe impl NSObjectProtocol for CameraDelegate {}

    // AVCaptureVideoDataOutputSampleBufferDelegate
    unsafe impl CameraDelegate {
        #[method(captureOutput:didOutputSampleBuffer:fromConnection:)]
        unsafe fn capture_output(
            &self,
            _output: *mut AnyObject,
            sample_buffer: *mut c_void, // CMSampleBufferRef
            _connection: *mut AnyObject,
        ) {
            if sample_buffer.is_null() {
                return;
            }

            // CMSampleBufferGetImageBuffer returns CVImageBufferRef (== CVPixelBufferRef)
            let pixel_buffer: *mut c_void = CMSampleBufferGetImageBuffer(sample_buffer);
            if pixel_buffer.is_null() {
                return;
            }

            let width = CVPixelBufferGetWidth(pixel_buffer);

            detection::detect_face_distance(pixel_buffer, width, &self.ivars().distance);
        }
    }
);

// CoreMedia / CoreVideo C functions
unsafe extern "C" {
    fn CMSampleBufferGetImageBuffer(sbuf: *mut c_void) -> *mut c_void;
    fn CVPixelBufferGetWidth(pixel_buffer: *mut c_void) -> usize;
}

impl CameraDelegate {
    fn new(distance: Arc<Mutex<Option<f32>>>) -> Id<Self> {
        let this: Allocated<Self> = Self::alloc();
        unsafe {
            let this = this.set_ivars(CameraDelegateIvars { distance });
            msg_send_id![super(this), init]
        }
    }
}

pub(super) fn start_webcam_capture(store: Res<DistanceStore>) {
    let distance = Arc::clone(&store.distance_cm);

    #[expect(
        clippy::infinite_loop,
        reason = "webcam capture thread runs until process exit"
    )]
    std::thread::spawn(move || {
        unsafe {
            let device_cls = AnyClass::get("AVCaptureDevice").expect("AVCaptureDevice not found");
            let media_type = NSString::from_str("vide");

            // Get default video capture device
            let device: *mut AnyObject =
                msg_send![device_cls, defaultDeviceWithMediaType: &*media_type];
            if device.is_null() {
                warn!("No webcam device found");
                return;
            }

            // Create AVCaptureDeviceInput
            let input_cls =
                AnyClass::get("AVCaptureDeviceInput").expect("AVCaptureDeviceInput not found");
            let mut error: *mut NSError = std::ptr::null_mut();
            let input: *mut AnyObject =
                msg_send![input_cls, deviceInputWithDevice: device error: &mut error];
            if input.is_null() {
                warn!("Failed to create camera input: {:?}", error);
                return;
            }

            // Create AVCaptureSession
            let session_cls =
                AnyClass::get("AVCaptureSession").expect("AVCaptureSession not found");
            let session: *mut AnyObject = msg_send![session_cls, alloc];
            let session: *mut AnyObject = msg_send![session, init];

            // Set session preset to 1280x720
            let preset = NSString::from_str("AVCaptureSessionPreset1280x720");
            let _: () = msg_send![session, setSessionPreset: &*preset];

            // Add input
            let can_add: Bool = msg_send![session, canAddInput: input];
            if !can_add.as_bool() {
                warn!("Cannot add camera input to session");
                return;
            }
            let _: () = msg_send![session, addInput: input];

            // Create AVCaptureVideoDataOutput
            let output_cls = AnyClass::get("AVCaptureVideoDataOutput")
                .expect("AVCaptureVideoDataOutput not found");
            let output: *mut AnyObject = msg_send![output_cls, alloc];
            let output: *mut AnyObject = msg_send![output, init];

            // Set alwaysDiscardsLateVideoFrames = YES
            let _: () = msg_send![output, setAlwaysDiscardsLateVideoFrames: Bool::YES];

            // Set pixel format to 32BGRA
            let pixel_format_key = NSString::from_str("PixelFormatType");
            let bgra_value: u32 = 0x4247_5241; // 'BGRA' = kCVPixelFormatType_32BGRA
            let number_cls = AnyClass::get("NSNumber").expect("NSNumber not found");
            let format_number: *mut AnyObject =
                msg_send![number_cls, numberWithUnsignedInt: bgra_value];
            let dict_cls = AnyClass::get("NSDictionary").expect("NSDictionary not found");
            let settings: *mut AnyObject = msg_send![
                dict_cls,
                dictionaryWithObject: format_number
                forKey: &*pixel_format_key
            ];
            let _: () = msg_send![output, setVideoSettings: settings];

            // Create delegate
            let delegate = CameraDelegate::new(distance);

            // Create dispatch queue for callbacks
            let queue =
                DispatchQueue::new("com.open_space.webcam_distance", DispatchQueueAttr::SERIAL);

            // Set sample buffer delegate
            let raw_queue = queue.as_raw().as_ptr().cast::<AnyObject>();
            let _: () = msg_send![
                output,
                setSampleBufferDelegate: &*delegate
                queue: raw_queue
            ];

            // Add output
            let can_add_output: Bool = msg_send![session, canAddOutput: output];
            if !can_add_output.as_bool() {
                warn!("Cannot add video output to session");
                return;
            }
            let _: () = msg_send![session, addOutput: output];

            info!("Starting webcam capture for distance estimation");
            let _: () = msg_send![session, startRunning];

            // Keep thread alive — delegate + session must not be dropped
            let _delegate = delegate;
            #[expect(
                clippy::no_effect_underscore_binding,
                reason = "intentionally keeping raw pointer alive"
            )]
            let _session = session;
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    });
}

/// Bevy system that logs the estimated distance ~1/sec.
pub(super) fn log_distance(store: Res<DistanceStore>, time: Res<Time>, mut last_log: Local<f32>) {
    let now = time.elapsed_secs_wrapped();
    if now - *last_log < 1.0 {
        return;
    }
    *last_log = now;

    let value = *store.distance_cm.lock().unwrap();
    if let Some(distance) = value {
        info!("Estimated distance to face: {distance:.1} cm");
    }
}
