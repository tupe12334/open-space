unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

/// Check and request screen recording permission before the event loop starts.
/// Must be called from `main()` before `App::new()` to avoid blocking the event loop.
pub(crate) fn ensure_screen_capture_permission() {
    let has_permission = unsafe { CGPreflightScreenCaptureAccess() };
    if has_permission {
        eprintln!("Screen recording permission granted");
    } else {
        eprintln!("Screen recording permission not granted. Requesting access...");
        let granted = unsafe { CGRequestScreenCaptureAccess() };
        if !granted {
            eprintln!(
                "Screen recording permission denied. \
                 Grant permission in System Settings > Privacy & Security > Screen Recording, \
                 then restart the app."
            );
        }
    }
}
