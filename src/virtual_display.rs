use bevy::prelude::*;
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

/// Wraps a `CGVirtualDisplay` pointer with Send+Sync.
/// Safety: `CGVirtualDisplay` objects are created on the main thread and only
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

/// Wraps a `CGVirtualDisplay` + its descriptor so they stay alive.
struct VirtualDisplay {
    _display: SendableObj,
    display_id: u32,
    width: u32,
    height: u32,
}

/// Resource that keeps virtual displays alive for the lifetime of the app.
#[derive(Resource)]
pub struct VirtualDisplays {
    displays: Vec<VirtualDisplay>,
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
        self.displays.iter().map(|d| d.display_id).collect()
    }

    pub fn displays(&self) -> Vec<VirtualDisplayInfo> {
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

pub struct VirtualDisplayPlugin;

impl Plugin for VirtualDisplayPlugin {
    fn build(&self, _app: &mut App) {
        // VirtualDisplays resource is inserted by main() before App::new(),
        // so it is already available. Nothing else to do here.
    }
}

/// Create virtual displays eagerly (call from main before `App::new`).
/// This blocks while waiting for display modes, but that's fine because
/// the event loop hasn't started yet so macOS won't show "Not Responding".
pub fn create_virtual_displays_blocking(num_screens: usize) -> VirtualDisplays {
    create_virtual_displays(num_screens, 1920, 1080, 60.0)
}

/// Alloc+init an `ObjC` object, returning a retained raw pointer.
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
            let label = std::ffi::CString::new(format!("com.spatial_display.virtual.{i}")).unwrap();
            let queue = dispatch_queue_create(label.as_ptr(), std::ptr::null());
            let queue_obj = queue.cast::<AnyObject>();
            let _: () = msg_send![descriptor, setQueue: queue_obj];

            // Create mode: initWithWidth:height:refreshRate:
            let mode: *const AnyObject = msg_send![mode_cls, alloc];
            let mode: *const AnyObject =
                msg_send![mode, initWithWidth: width, height: height, refreshRate: refresh_rate];

            eprintln!("Virtual display mode: {width}x{height} @ {refresh_rate}Hz");

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
                eprintln!(
                    "Virtual display {} creation failed (initWithDescriptor returned nil)",
                    i + 1
                );
                ffi::objc_release(descriptor as *mut _);
                ffi::objc_release(settings as *mut _);
                continue;
            }

            let display_id: u32 = msg_send![display, displayID];
            eprintln!("Virtual display {} created with ID {}", i + 1, display_id);

            // Apply settings
            let result: bool = msg_send![display, applySettings: settings];
            eprintln!("Apply settings result for display {display_id}: {result}");

            // Release temporaries (descriptor, settings, mode are not needed after this)
            ffi::objc_release(descriptor as *mut _);
            ffi::objc_release(settings as *mut _);
            // mode is autoreleased via arrayWithObject, don't double-release

            if !result {
                eprintln!(
                    "applySettings failed for virtual display {} (ID {}), releasing it",
                    i + 1,
                    display_id
                );
                ffi::objc_release(display as *mut _);
                continue;
            }

            // Keep display alive (we own the +1 from alloc/init)
            displays.push(VirtualDisplay {
                _display: SendableObj(display),
                display_id,
                width,
                height,
            });
        }
    }

    eprintln!(
        "Created {} virtual display(s): {:?}",
        displays.len(),
        displays.iter().map(|d| d.display_id).collect::<Vec<_>>()
    );

    // Poll until macOS has registered display modes for ALL active displays.
    // CGVirtualDisplay mode registration requires NSRunLoop event processing,
    // so we spin the run loop instead of sleeping. Without this, modes for
    // virtual displays never become visible to CGDisplayCopyAllDisplayModes
    // and winit will panic when enumerating monitors.
    {
        let timeout = std::time::Duration::from_secs(15);
        let spin_interval = 0.05_f64; // 50ms
        let start = std::time::Instant::now();

        loop {
            // Spin the NSRunLoop so macOS can process display mode registration
            unsafe {
                let run_loop_cls = AnyClass::get("NSRunLoop").unwrap();
                let current: *const AnyObject = msg_send![run_loop_cls, currentRunLoop];
                let date_cls = AnyClass::get("NSDate").unwrap();
                let until: *const AnyObject =
                    msg_send![date_cls, dateWithTimeIntervalSinceNow: spin_interval];
                let mode = NSString::from_str("kCFRunLoopDefaultMode");
                let _: bool = msg_send![current, runMode: &*mode, beforeDate: until];
            }

            let all_displays = crate::stage::get_active_displays(32);
            let missing: Vec<u32> = all_displays
                .iter()
                .filter(|(_, cg)| cg.copy_display_modes().is_none())
                .map(|(id, _)| *id)
                .collect();

            if missing.is_empty() {
                eprintln!("All display modes ready after {:.0?}", start.elapsed());
                break;
            }
            if start.elapsed() > timeout {
                eprintln!("Timed out waiting for display modes on display(s): {missing:?}");
                // Remove virtual displays whose modes never became available
                // to prevent winit from panicking.
                let virtual_ids: std::collections::HashSet<u32> =
                    displays.iter().map(|d| d.display_id).collect();
                let modeless: std::collections::HashSet<u32> = missing
                    .into_iter()
                    .filter(|id| virtual_ids.contains(id))
                    .collect();
                if !modeless.is_empty() {
                    eprintln!(
                        "Destroying {} virtual display(s) without modes: {:?}",
                        modeless.len(),
                        modeless
                    );
                    displays.retain(|d| !modeless.contains(&d.display_id));
                }
                break;
            }
        }
    }

    eprintln!(
        "{} virtual display(s) active: {:?}",
        displays.len(),
        displays.iter().map(|d| d.display_id).collect::<Vec<_>>()
    );

    VirtualDisplays { displays }
}
