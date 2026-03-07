mod plugin;

pub(crate) use plugin::VirtualDisplayPlugin;

use bevy::prelude::*;
use objc2::ffi;
use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::{CGSize, NSString};

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
            ffi::objc_release(self.0 as *mut _);
        }
    }
}

struct VirtualDisplay {
    _display: SendableObj,
    display_id: u32,
    width: u32,
    height: u32,
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

    let descriptor_cls = AnyClass::get("CGVirtualDisplayDescriptor")
        .expect("CGVirtualDisplayDescriptor class not found");
    let mode_cls =
        AnyClass::get("CGVirtualDisplayMode").expect("CGVirtualDisplayMode class not found");
    let settings_cls = AnyClass::get("CGVirtualDisplaySettings")
        .expect("CGVirtualDisplaySettings class not found");
    let display_cls = AnyClass::get("CGVirtualDisplay").expect("CGVirtualDisplay class not found");
    let nsarray_cls = AnyClass::get("NSArray").unwrap();

    for i in 0..count {
        unsafe {
            let descriptor = alloc_init(descriptor_cls);

            let name = NSString::from_str(&format!("Virtual Screen {}", i + 1));
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

            let settings = alloc_init(settings_cls);
            let modes: *const AnyObject = msg_send![nsarray_cls, arrayWithObject: mode];
            let _: () = msg_send![settings, setModes: modes];
            let _: () = msg_send![settings, setHiDPI: 0_u32];

            let display: *const AnyObject = msg_send![display_cls, alloc];
            let display: *const AnyObject = msg_send![display, initWithDescriptor: descriptor];

            if display.is_null() {
                warn!(
                    "Virtual display {} creation failed (initWithDescriptor returned nil)",
                    i + 1
                );
                ffi::objc_release(descriptor as *mut _);
                ffi::objc_release(settings as *mut _);
                continue;
            }

            let display_id: u32 = msg_send![display, displayID];
            let result: bool = msg_send![display, applySettings: settings];
            info!(
                "Virtual display {} created: ID={}, applySettings={}",
                i + 1,
                display_id,
                result
            );

            ffi::objc_release(descriptor as *mut _);
            ffi::objc_release(settings as *mut _);

            if !result {
                warn!(
                    "applySettings failed for virtual display {} (ID {})",
                    i + 1,
                    display_id
                );
                ffi::objc_release(display as *mut _);
                continue;
            }

            virtual_displays.displays.push(VirtualDisplay {
                _display: SendableObj(display),
                display_id,
                width,
                height,
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
