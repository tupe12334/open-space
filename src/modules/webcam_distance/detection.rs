use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use objc2::encode::{Encode, Encoding, RefEncode};
use objc2::msg_send;
use objc2::rc::Id;
use objc2::runtime::{AnyClass, AnyObject, Bool};
use objc2_foundation::{NSArray, NSError};

/// Average adult face width in cm (interpupillary + cheek margin).
const FACE_WIDTH_CM: f32 = 15.0;

/// Approximate focal length in pixels for a 720p `FaceTime` camera.
const FOCAL_LENGTH_PX: f32 = 900.0;

/// EMA smoothing factor — lower = smoother but slower to respond.
const EMA_ALPHA: f32 = 0.3;

/// Opaque type matching `CoreVideo`'s `CVBuffer` / `CVPixelBuffer`.
/// Ensures `*mut CVBuffer` encodes as `^{__CVBuffer=}` for `ObjC` type checking.
#[repr(C)]
struct CVBuffer {
    _priv: [u8; 0],
}

// Safety: matches the ObjC type encoding for CVPixelBufferRef = ^{__CVBuffer=}
unsafe impl RefEncode for CVBuffer {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CVBuffer", &[]));
}

/// Layout-compatible with `CGRect` for `ObjC` `msg_send!` return values.
#[repr(C)]
#[derive(Clone, Copy)]
struct CGRect {
    origin_x: f64,
    origin_y: f64,
    size_width: f64,
    size_height: f64,
}

// Safety: CGRect is a plain C struct with the same layout as the ObjC CGRect.
// Encoding matches {CGRect={CGPoint=dd}{CGSize=dd}}.
unsafe impl Encode for CGRect {
    const ENCODING: Encoding = Encoding::Struct(
        "CGRect",
        &[
            Encoding::Struct("CGPoint", &[Encoding::Double, Encoding::Double]),
            Encoding::Struct("CGSize", &[Encoding::Double, Encoding::Double]),
        ],
    );
}

/// Detect a face in the given pixel buffer and estimate distance in cm.
///
/// Uses Apple Vision framework's `VNDetectFaceRectanglesRequest`.
/// The `image_width` is the pixel width of the frame (used to convert normalised bbox).
pub(super) fn detect_face_distance(
    pixel_buffer: *mut c_void,
    image_width: usize,
    smoothed: &Arc<Mutex<Option<f32>>>,
) {
    // Safety: all calls go through ObjC runtime to Vision framework classes.
    // pixel_buffer is a CVPixelBufferRef obtained from CMSampleBufferGetImageBuffer.
    unsafe {
        let handler_cls =
            AnyClass::get("VNImageRequestHandler").expect("VNImageRequestHandler not found");
        let request_cls = AnyClass::get("VNDetectFaceRectanglesRequest")
            .expect("VNDetectFaceRectanglesRequest not found");

        // Create VNImageRequestHandler from CVPixelBuffer.
        // Cast to *mut CVBuffer so msg_send! encodes the argument as ^{__CVBuffer=}.
        let cv_pixel_buffer: *mut CVBuffer = pixel_buffer.cast();
        let options_cls = AnyClass::get("NSDictionary").expect("NSDictionary not found");
        let empty_dict: *mut AnyObject = msg_send![options_cls, dictionary];

        let handler: *mut AnyObject = msg_send![handler_cls, alloc];
        let handler: *mut AnyObject =
            msg_send![handler, initWithCVPixelBuffer: cv_pixel_buffer options: empty_dict];
        if handler.is_null() {
            return;
        }

        // Create VNDetectFaceRectanglesRequest
        let request: *mut AnyObject = msg_send![request_cls, alloc];
        let request: *mut AnyObject = msg_send![request, init];
        if request.is_null() {
            return;
        }

        // Build NSArray with the single request
        let requests: Id<NSArray<AnyObject>> = {
            let raw: *mut AnyObject = request;
            // Safety: wrapping a single ObjC object into an NSArray
            let arr: *mut AnyObject = msg_send![
                AnyClass::get("NSArray").unwrap(),
                arrayWithObject: raw
            ];
            Id::retain(arr.cast()).unwrap()
        };

        // Perform the request
        let mut error: *mut NSError = std::ptr::null_mut();
        let success: Bool = msg_send![handler, performRequests: &*requests error: &mut error];

        if !success.as_bool() {
            return;
        }

        // Read results from the request
        let results: *mut NSArray<AnyObject> = msg_send![request, results];
        if results.is_null() {
            return;
        }
        let count: usize = msg_send![results, count];
        if count == 0 {
            return;
        }

        // Get first face observation
        let face: *mut AnyObject = msg_send![results, objectAtIndex: 0_usize];
        if face.is_null() {
            return;
        }

        // boundingBox returns CGRect (normalised 0..1)
        let bbox: CGRect = msg_send![face, boundingBox];
        let face_width_px = bbox.size_width as f32 * image_width as f32;

        if face_width_px < 1.0 {
            return;
        }

        let raw_distance = (FACE_WIDTH_CM * FOCAL_LENGTH_PX) / face_width_px;

        // Apply EMA smoothing
        let mut guard = smoothed.lock().unwrap();
        let smoothed_val = guard.map_or(raw_distance, |prev| {
            EMA_ALPHA.mul_add(raw_distance, (1.0 - EMA_ALPHA) * prev)
        });
        *guard = Some(smoothed_val);
    }
}
