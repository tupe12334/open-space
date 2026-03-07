//! Shared constants and helpers for the display grid layout.

/// Number of columns in the display grid.
pub(crate) const GRID_COLS: usize = 3;

/// Half-width of each display mesh in world units.
pub(crate) const DISPLAY_HALF_WIDTH: f32 = 2.5;

/// Standard aspect ratio (height / width) for all display meshes.
/// Matches the virtual display resolution of 1920x1080 (16:9).
pub(crate) const DISPLAY_ASPECT: f32 = 9.0 / 16.0;

/// Half-height derived from the standard aspect ratio.
pub(crate) const DISPLAY_HALF_HEIGHT: f32 = DISPLAY_HALF_WIDTH * DISPLAY_ASPECT;

/// Standard pixel resolution for all displays (matching virtual displays).
pub(crate) const DISPLAY_WIDTH: u32 = 1920;
pub(crate) const DISPLAY_HEIGHT: u32 = 1080;

/// Reorder a vec of display specs so that the entry matching `main_id`
/// lands at the center of a 3-column grid.
pub(crate) fn center_main_display<T, F>(specs: &mut [T], main_id: u32, get_id: F)
where
    F: Fn(&T) -> u32,
{
    if specs.len() <= 1 {
        return;
    }
    if let Some(main_idx) = specs.iter().position(|s| get_id(s) == main_id) {
        let n = specs.len();
        let total_rows = n.div_ceil(GRID_COLS);
        let center_col = GRID_COLS / 2;
        let center_row = total_rows / 2;
        let center_idx = (center_row * GRID_COLS + center_col).min(n - 1);
        specs.swap(main_idx, center_idx);
    }
}
