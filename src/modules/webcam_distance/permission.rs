use objc2::msg_send;
use objc2::runtime::AnyClass;
use objc2_foundation::NSString;

/// Check camera permission status and print guidance if not granted.
/// Must be called from `main()` before `App::new()`.
///
/// Note: unlike screen capture, `AVFoundation` will automatically prompt
/// the user for camera access when the capture session starts.
/// This function only checks and logs the current status.
pub(crate) fn ensure_camera_permission() {
    let cls = AnyClass::get("AVCaptureDevice").expect("AVCaptureDevice class not found");

    let media_type = NSString::from_str("vide"); // AVMediaTypeVideo = "vide"

    // authorizationStatusForMediaType: returns NSInteger
    // 0 = NotDetermined, 1 = Restricted, 2 = Denied, 3 = Authorized
    let status: isize = unsafe { msg_send![cls, authorizationStatusForMediaType: &*media_type] };

    match status {
        3 => {
            eprintln!("Camera permission granted");
        }
        0 => {
            eprintln!(
                "Camera permission not yet determined. \
                 The system will prompt when the webcam starts."
            );
        }
        _ => {
            eprintln!(
                "Camera permission denied or restricted (status={status}). \
                 Grant permission in System Settings > Privacy & Security > Camera, \
                 then restart the app."
            );
        }
    }
}
