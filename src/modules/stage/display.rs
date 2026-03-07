use core_graphics2::display::CGDisplay;

extern "C" {
    fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
}

/// Returns (`display_id`, `CGDisplay`) pairs for active displays, up to `max`.
pub(crate) fn get_active_displays(max: usize) -> Vec<(u32, CGDisplay)> {
    let mut ids = vec![0_u32; max];
    let mut count = 0_u32;
    let err = unsafe { CGGetActiveDisplayList(max as u32, ids.as_mut_ptr(), &raw mut count) };
    if err != 0 {
        return vec![];
    }
    ids.truncate(count as usize);
    ids.into_iter().map(|id| (id, CGDisplay::new(id))).collect()
}
