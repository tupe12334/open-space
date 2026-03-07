mod plugin;

pub(crate) use plugin::VirtualDisplayPlugin;

use bevy::prelude::*;
use objc2::ffi;
use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::{CGSize, NSString};

use crate::modules::grid_layout::{grid_center_index, grid_position_name, GRID_COLS};
use crate::modules::settings::AppSettings;

extern "C" {
    fn dispatch_queue_create(
        label: *const std::ffi::c_char,
        attr: *const std::ffi::c_void,
    ) -> *const std::ffi::c_void;
}

/// Wraps a `CGVirtualDisplay` pointer with Send+Sync.
struct SendableObj(*const AnyObject);
unsafe impl Send for SendableObj {}
unsafe impl Sync for SendableObj {}

impl Drop for SendableObj {
    fn drop(&mut self) {
        unsafe {
            ffi::objc_release(self.0 as *mut ffi::objc_object);
        }
    }
}

struct VirtualDisplay {
    _display: SendableObj,
    display_id: u32,
    width: u32,
    height: u32,
    name: String,
}

/// Resource that keeps virtual displays alive for the lifetime of the app.
#[derive(Resource, Default)]
pub(crate) struct VirtualDisplays {
    displays: Vec<VirtualDisplay>,
}

/// Info about a single virtual display.
pub(crate) struct VirtualDisplayInfo {
    pub(crate) display_id: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    #[expect(dead_code, reason = "public API reserved for future use")]
    pub(crate) name: String,
}

impl VirtualDisplays {
    #[expect(dead_code, reason = "public API reserved for future use")]
    pub(crate) fn display_ids(&self) -> Vec<u32> {
        self.displays.iter().map(|d| d.display_id).collect()
    }

    pub(crate) fn displays(&self) -> Vec<VirtualDisplayInfo> {
        self.displays
            .iter()
            .map(|d| VirtualDisplayInfo {
                display_id: d.display_id,
                width: d.width,
                height: d.height,
                name: d.name.clone(),
            })
            .collect()
    }
}

/// Alloc+init an `ObjC` object, returning a retained raw pointer.
unsafe fn alloc_init(cls: &AnyClass) -> *const AnyObject {
    let obj: *const AnyObject = msg_send![cls, alloc];
    let obj: *const AnyObject = msg_send![obj, init];
    obj
}

pub(super) fn create_virtual_displays_system(
    mut virtual_displays: ResMut<VirtualDisplays>,
    settings: Res<AppSettings>,
) {
    let count = (settings.num_screens as usize).saturating_sub(1);
    let width = 1920_u32;
    let height = 1080_u32;
    let refresh_rate = 60.0_f64;

    // Pre-compute grid position names for each virtual display.
    // The main display will be appended at index `count`, making total = count + 1.
    let num_screens = count + 1;
    let total_rows = num_screens.div_ceil(GRID_COLS);
    let center_idx = grid_center_index(num_screens);

    let descriptor_cls = AnyClass::get("CGVirtualDisplayDescriptor")
        .expect("CGVirtualDisplayDescriptor class not found");
    let mode_cls =
        AnyClass::get("CGVirtualDisplayMode").expect("CGVirtualDisplayMode class not found");
    let settings_cls = AnyClass::get("CGVirtualDisplaySettings")
        .expect("CGVirtualDisplaySettings class not found");
    let display_cls = AnyClass::get("CGVirtualDisplay").expect("CGVirtualDisplay class not found");
    let nsarray_cls = AnyClass::get("NSArray").unwrap();

    for i in 0..count {
        // Virtual display at creation index `i` will end up at grid position `grid_pos`
        // after center_main_display swaps main into center_idx.
        let grid_pos = if i == center_idx { count } else { i };
        let row = grid_pos / GRID_COLS;
        let col = grid_pos % GRID_COLS;
        let display_name = grid_position_name(row, col, total_rows);

        unsafe {
            let descriptor = alloc_init(descriptor_cls);

            let name = NSString::from_str(&display_name);
            let _: () = msg_send![descriptor, setName: &*name];
            let _: () = msg_send![descriptor, setMaxPixelsWide: width];
            let _: () = msg_send![descriptor, setMaxPixelsHigh: height];
            let _: () = msg_send![descriptor, setProductID: 0x1234_u32];
            let _: () = msg_send![descriptor, setVendorID: 0x5678_u32];
            let _: () = msg_send![descriptor, setSerialNum: i as u32];

            let size = CGSize::new(600.0, 340.0);
            let _: () = msg_send![descriptor, setSizeInMillimeters: size];

            let label = std::ffi::CString::new(format!("com.spatial_display.virtual.{i}")).unwrap();
            let queue = dispatch_queue_create(label.as_ptr(), std::ptr::null());
            let queue_obj = queue.cast::<AnyObject>();
            let _: () = msg_send![descriptor, setQueue: queue_obj];

            let mode: *const AnyObject = msg_send![mode_cls, alloc];
            let mode: *const AnyObject =
                msg_send![mode, initWithWidth: width, height: height, refreshRate: refresh_rate];

            let display_settings = alloc_init(settings_cls);
            let modes: *const AnyObject = msg_send![nsarray_cls, arrayWithObject: mode];
            let _: () = msg_send![display_settings, setModes: modes];
            let _: () = msg_send![display_settings, setHiDPI: 0_u32];

            let display: *const AnyObject = msg_send![display_cls, alloc];
            let display: *const AnyObject = msg_send![display, initWithDescriptor: descriptor];

            if display.is_null() {
                warn!(
                    "Virtual display '{}' creation failed (initWithDescriptor returned nil)",
                    display_name
                );
                ffi::objc_release(descriptor as *mut ffi::objc_object);
                ffi::objc_release(display_settings as *mut ffi::objc_object);
                continue;
            }

            let display_id: u32 = msg_send![display, displayID];
            let result: bool = msg_send![display, applySettings: display_settings];
            info!(
                "Virtual display '{}' created: ID={}, applySettings={}",
                display_name, display_id, result
            );

            ffi::objc_release(descriptor as *mut ffi::objc_object);
            ffi::objc_release(display_settings as *mut ffi::objc_object);

            if !result {
                warn!(
                    "applySettings failed for virtual display '{}' (ID {})",
                    display_name, display_id
                );
                ffi::objc_release(display as *mut ffi::objc_object);
                continue;
            }

            virtual_displays.displays.push(VirtualDisplay {
                _display: SendableObj(display),
                display_id,
                width,
                height,
                name: display_name,
            });
        }
    }

    info!(
        "{} virtual display(s) active: {:?}",
        virtual_displays.displays.len(),
        virtual_displays
            .displays
            .iter()
            .map(|d| d.display_id)
            .collect::<Vec<_>>()
    );
}
