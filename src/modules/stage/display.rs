use core_graphics2::display::CGDisplay;

// Binding to the macOS CoreGraphics C function that retrieves currently active (online) displays.
// See: https://developer.apple.com/documentation/coregraphics/1454603-cggetactivedisplaylist
unsafe extern "C" {
    /// Populates `displays` with up to `max` active display IDs and writes the actual count to `count`.
    /// Returns 0 (`kCGErrorSuccess`) on success, or a non-zero error code on failure.
    fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
}

/// Queries macOS for up to `max` active displays and returns them as `(display_id, CGDisplay)` pairs.
///
/// Returns an empty `Vec` if the underlying CoreGraphics call fails.
pub(crate) fn get_active_displays(max: usize) -> Vec<(u32, CGDisplay)> {
    let mut ids = vec![0_u32; max];
    let mut count = 0_u32;

    // SAFETY: `ids` is a valid buffer of `max` elements, and `count` is a valid mutable pointer.
    let err = unsafe { CGGetActiveDisplayList(max as u32, ids.as_mut_ptr(), &raw mut count) };
    if err != 0_i32 {
        return vec![];
    }

    // Only keep the entries that were actually populated by the system.
    ids.truncate(count as usize);
    ids.into_iter().map(|id| (id, CGDisplay::new(id))).collect()
}
