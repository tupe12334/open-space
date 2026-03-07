use std::time::{Duration, Instant};

use super::stage::get_active_displays;

pub(crate) fn wait_for_physical_display_modes() {
    let timeout = Duration::from_secs(10);
    let poll_interval = Duration::from_millis(50);
    let start = Instant::now();

    loop {
        let all = get_active_displays(32);
        let missing: Vec<u32> = all
            .iter()
            .filter(|(_, cg)| cg.copy_display_modes().is_none())
            .map(|(id, _)| *id)
            .collect();

        if missing.is_empty() {
            eprintln!(
                "All physical display modes ready after {:.0?}",
                start.elapsed()
            );
            break;
        }
        if start.elapsed() > timeout {
            eprintln!("Timed out waiting for physical display modes on: {missing:?}");
            break;
        }
        std::thread::sleep(poll_interval);
    }
}
