use bevy::prelude::*;
use core_graphics2::display::CGDisplay;
use objc2::ffi;
use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::{CGSize, NSString};

extern "C" {
    fn dispatch_queue_create(
        label: *const std::ffi::c_char,
        attr: *const std::ffi::c_void,
    ) -> *const std::ffi::c_void;
}

/// Wraps a CGVirtualDisplay pointer with Send+Sync.
/// Safety: CGVirtualDisplay objects are created on the main thread and only
/// held alive afterwards; we only read `displayID` and keep the ref alive.
struct SendableObj(*const AnyObject);
unsafe impl Send for SendableObj {}
unsafe impl Sync for SendableObj {}

impl Drop for SendableObj {
    fn drop(&mut self) {
        unsafe {
            // Release the retained object
            ffi::objc_release(self.0 as *mut _);
        }
    }
}

/// Wraps a CGVirtualDisplay + its descriptor so they stay alive.
struct VirtualDisplay {
    _display: SendableObj,
    display_id: u32,
    width: u32,
    height: u32,
}

/// Resource that keeps virtual displays alive for the lifetime of the app.
#[derive(Resource)]
pub struct VirtualDisplays {
    _displays: Vec<VirtualDisplay>,
}

/// Info about a single virtual display.
pub struct VirtualDisplayInfo {
    pub display_id: u32,
    pub width: u32,
    pub height: u32,
}

impl VirtualDisplays {
    #[allow(dead_code)]
    pub fn display_ids(&self) -> Vec<u32> {
        self._displays.iter().map(|d| d.display_id).collect()
    }

    pub fn displays(&self) -> Vec<VirtualDisplayInfo> {
        self._displays
            .iter()
            .map(|d| VirtualDisplayInfo {
                display_id: d.display_id,
                width: d.width,
                height: d.height,
            })
            .collect()
    }
}

pub struct VirtualDisplayPlugin;

impl Plugin for VirtualDisplayPlugin {
    fn build(&self, app: &mut App) {
        let num_screens = app
            .world()
            .get_resource::<crate::settings::AppSettings>()
            .map(|s| s.num_screens as usize)
            .unwrap_or(6);
        let displays = create_virtual_displays(num_screens, 1920, 1080, 60.0);
        app.insert_resource(displays);
    }
}

/// Alloc+init an ObjC object, returning a retained raw pointer.
/// Caller owns the +1 reference.
unsafe fn alloc_init(cls: &AnyClass) -> *const AnyObject {
    let obj: *const AnyObject = msg_send![cls, alloc];
    let obj: *const AnyObject = msg_send![obj, init];
    obj
}

fn create_virtual_displays(
    count: usize,
    width: u32,
    height: u32,
    refresh_rate: f64,
) -> VirtualDisplays {
    let descriptor_cls = AnyClass::get("CGVirtualDisplayDescriptor")
        .expect("CGVirtualDisplayDescriptor class not found");
    let mode_cls =
        AnyClass::get("CGVirtualDisplayMode").expect("CGVirtualDisplayMode class not found");
    let settings_cls = AnyClass::get("CGVirtualDisplaySettings")
        .expect("CGVirtualDisplaySettings class not found");
    let display_cls = AnyClass::get("CGVirtualDisplay").expect("CGVirtualDisplay class not found");
    let nsarray_cls = AnyClass::get("NSArray").unwrap();

    let mut displays = Vec::new();

    for i in 0..count {
        unsafe {
            // Create descriptor
            let descriptor = alloc_init(descriptor_cls);

            let name = NSString::from_str(&format!("Virtual Screen {}", i + 1));
            let _: () = msg_send![descriptor, setName: &*name];
            let _: () = msg_send![descriptor, setMaxPixelsWide: width];
            let _: () = msg_send![descriptor, setMaxPixelsHigh: height];
            let _: () = msg_send![descriptor, setProductID: 0x1234u32];
            let _: () = msg_send![descriptor, setVendorID: 0x5678u32];
            let _: () = msg_send![descriptor, setSerialNum: i as u32];

            // sizeInMillimeters is a CGSize struct
            let size = CGSize::new(600.0, 340.0);
            let _: () = msg_send![descriptor, setSizeInMillimeters: size];

            // Dispatch queue - dispatch_queue_t is toll-free bridged with NSObject
            let label =
                std::ffi::CString::new(format!("com.spatial_display.virtual.{}", i)).unwrap();
            let queue = dispatch_queue_create(label.as_ptr(), std::ptr::null());
            let queue_obj = queue as *const AnyObject;
            let _: () = msg_send![descriptor, setQueue: queue_obj];

            // Create mode: initWithWidth:height:refreshRate:
            let mode: *const AnyObject = msg_send![mode_cls, alloc];
            let mode: *const AnyObject =
                msg_send![mode, initWithWidth: width, height: height, refreshRate: refresh_rate];

            info!(
                "Virtual display mode: {}x{} @ {}Hz",
                width, height, refresh_rate
            );

            // Create settings
            let settings = alloc_init(settings_cls);

            // Create NSArray with the mode
            let modes: *const AnyObject = msg_send![nsarray_cls, arrayWithObject: mode];
            let _: () = msg_send![settings, setModes: modes];
            let _: () = msg_send![settings, setHiDPI: 0u32];

            // Create virtual display with descriptor
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
            info!("Virtual display {} created with ID {}", i + 1, display_id);

            // Apply settings
            let result: bool = msg_send![display, applySettings: settings];
            info!("Apply settings result: {}", result);

            // Release temporaries (descriptor, settings, mode are not needed after this)
            ffi::objc_release(descriptor as *mut _);
            ffi::objc_release(settings as *mut _);
            // mode is autoreleased via arrayWithObject, don't double-release

            // Keep display alive (we own the +1 from alloc/init)
            displays.push(VirtualDisplay {
                _display: SendableObj(display),
                display_id,
                width,
                height,
            });
        }
    }

    info!(
        "Created {} virtual display(s): {:?}",
        displays.len(),
        displays.iter().map(|d| d.display_id).collect::<Vec<_>>()
    );

    // Poll until macOS has registered display modes for every virtual display.
    // CGDisplayCopyAllDisplayModes returns NULL until the modes are ready;
    // winit will panic if it enumerates monitors before that happens.
    if !displays.is_empty() {
        let timeout = std::time::Duration::from_secs(5);
        let poll_interval = std::time::Duration::from_millis(50);
        let start = std::time::Instant::now();

        for vd in &displays {
            let cg = CGDisplay::new(vd.display_id);
            loop {
                if cg.copy_display_modes().is_some() {
                    break;
                }
                if start.elapsed() > timeout {
                    warn!(
                        "Timed out waiting for display modes on virtual display {} (ID {})",
                        vd.display_id, vd.display_id
                    );
                    break;
                }
                std::thread::sleep(poll_interval);
            }
        }
        info!("Virtual display modes ready after {:.0?}", start.elapsed());
    }

    VirtualDisplays {
        _displays: displays,
    }
}
