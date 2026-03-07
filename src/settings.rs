use bevy::prelude::*;
use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{declare_class, msg_send_id, mutability, sel, ClassType, DeclaredClass};
use objc2_foundation::{NSObject, NSString};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

const SETTINGS_FILE: &str = "settings.json";
const DEFAULT_STAGE_DISTANCE: f32 = 6.0;
const DISTANCE_STEP: f32 = 0.5;
const MIN_DISTANCE: f32 = 1.0;
const MAX_DISTANCE: f32 = 30.0;
const DEFAULT_NUM_SCREENS: u32 = 6;
const MIN_NUM_SCREENS: u32 = 1;
const MAX_NUM_SCREENS: u32 = 6;

static DISTANCE_STEPS: AtomicI32 = AtomicI32::new(0);
static SCREEN_STEPS: AtomicI32 = AtomicI32::new(0);
pub(crate) static CENTER_STAGE: AtomicBool = AtomicBool::new(false);

#[derive(Resource, Clone)]
pub(crate) struct AppSettings {
    pub(crate) stage_distance: f32,
    pub(crate) num_screens: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            stage_distance: DEFAULT_STAGE_DISTANCE,
            num_screens: DEFAULT_NUM_SCREENS,
        }
    }
}

declare_class!(
    struct MenuHandler;

    unsafe impl ClassType for MenuHandler {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "OSMenuHandler";
    }

    impl DeclaredClass for MenuHandler {
        type Ivars = ();
    }

    unsafe impl MenuHandler {
        #[method(increaseDistance:)]
        fn _increase_distance(&self, _sender: &AnyObject) {
            DISTANCE_STEPS.fetch_add(1, Ordering::Relaxed);
        }

        #[method(decreaseDistance:)]
        fn _decrease_distance(&self, _sender: &AnyObject) {
            DISTANCE_STEPS.fetch_add(-1, Ordering::Relaxed);
        }

        #[method(increaseScreens:)]
        fn _increase_screens(&self, _sender: &AnyObject) {
            SCREEN_STEPS.fetch_add(1, Ordering::Relaxed);
        }

        #[method(decreaseScreens:)]
        fn _decrease_screens(&self, _sender: &AnyObject) {
            SCREEN_STEPS.fetch_add(-1, Ordering::Relaxed);
        }

        #[method(centerStage:)]
        fn _center_stage(&self, _sender: &AnyObject) {
            CENTER_STAGE.store(true, Ordering::Relaxed);
        }
    }
);

impl MenuHandler {
    fn new() -> objc2::rc::Id<Self> {
        let this = Self::alloc().set_ivars(());
        unsafe { msg_send_id![super(this), init] }
    }
}

#[derive(Resource)]
#[expect(dead_code, reason = "menu bar setup temporarily disabled")]
struct NativeMenuHandler(objc2::rc::Id<MenuHandler>);

// SAFETY: MenuHandler only modifies global atomics and is stored
// solely to prevent deallocation. It is never accessed from Bevy threads.
unsafe impl Send for NativeMenuHandler {}
unsafe impl Sync for NativeMenuHandler {}

pub(crate) struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        // NOTE: setup_menu_bar is disabled because modifying NSApplication's
        // menu from inside a Bevy Startup system deadlocks the winit event loop.
        app.add_systems(Update, poll_menu_changes);
    }
}

fn settings_path() -> PathBuf {
    PathBuf::from(SETTINGS_FILE)
}

pub(crate) fn load_settings() -> AppSettings {
    let path = settings_path();
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&data) {
            let stage_distance = val
                .get("stage_distance")
                .and_then(serde_json::Value::as_f64)
                .map_or(DEFAULT_STAGE_DISTANCE, |d| d as f32);
            let num_screens = val
                .get("num_screens")
                .and_then(serde_json::Value::as_u64)
                .map_or(DEFAULT_NUM_SCREENS, |n| {
                    (n as u32).clamp(MIN_NUM_SCREENS, MAX_NUM_SCREENS)
                });
            return AppSettings {
                stage_distance,
                num_screens,
            };
        }
    }
    AppSettings::default()
}

fn save_settings(settings: &AppSettings) {
    let val = serde_json::json!({
        "stage_distance": settings.stage_distance,
        "num_screens": settings.num_screens,
    });
    if let Ok(data) = serde_json::to_string_pretty(&val) {
        if let Err(e) = fs::write(settings_path(), data) {
            eprintln!("Failed to write settings: {e}");
        }
    }
}

#[expect(dead_code, reason = "menu bar setup temporarily disabled")]
fn setup_menu_bar(mut commands: Commands) {
    let handler = MenuHandler::new();

    unsafe {
        let app: *const AnyObject =
            msg_send![AnyClass::get("NSApplication").unwrap(), sharedApplication];
        let main_menu: *const AnyObject = msg_send![app, mainMenu];
        if main_menu.is_null() {
            warn!("No main menu found");
            return;
        }

        // Create "Settings" submenu
        let menu_title = NSString::from_str("Settings");
        let settings_menu: *const AnyObject = msg_send![AnyClass::get("NSMenu").unwrap(), alloc];
        let settings_menu: *const AnyObject = msg_send![settings_menu, initWithTitle: &*menu_title];

        let handler_ptr: *const MenuHandler = &raw const *handler;

        // "Increase Distance" menu item
        let inc_title = NSString::from_str("Increase Distance");
        let inc_key = NSString::from_str("=");
        let increase_item: *const AnyObject =
            msg_send![AnyClass::get("NSMenuItem").unwrap(), alloc];
        let increase_item: *const AnyObject = msg_send![
            increase_item,
            initWithTitle: &*inc_title,
            action: sel!(increaseDistance:),
            keyEquivalent: &*inc_key
        ];
        let _: () = msg_send![increase_item, setTarget: handler_ptr];
        let _: () = msg_send![settings_menu, addItem: increase_item];

        // "Decrease Distance" menu item
        let dec_title = NSString::from_str("Decrease Distance");
        let dec_key = NSString::from_str("-");
        let decrease_item: *const AnyObject =
            msg_send![AnyClass::get("NSMenuItem").unwrap(), alloc];
        let decrease_item: *const AnyObject = msg_send![
            decrease_item,
            initWithTitle: &*dec_title,
            action: sel!(decreaseDistance:),
            keyEquivalent: &*dec_key
        ];
        let _: () = msg_send![decrease_item, setTarget: handler_ptr];
        let _: () = msg_send![settings_menu, addItem: decrease_item];

        // Separator
        let separator: *const AnyObject =
            msg_send![AnyClass::get("NSMenuItem").unwrap(), separatorItem];
        let _: () = msg_send![settings_menu, addItem: separator];

        // "More Screens" menu item
        let more_title = NSString::from_str("More Screens (restart to apply)");
        let more_key = NSString::from_str("]");
        let more_item: *const AnyObject = msg_send![AnyClass::get("NSMenuItem").unwrap(), alloc];
        let more_item: *const AnyObject = msg_send![
            more_item,
            initWithTitle: &*more_title,
            action: sel!(increaseScreens:),
            keyEquivalent: &*more_key
        ];
        let _: () = msg_send![more_item, setTarget: handler_ptr];
        let _: () = msg_send![settings_menu, addItem: more_item];

        // "Fewer Screens" menu item
        let fewer_title = NSString::from_str("Fewer Screens (restart to apply)");
        let fewer_key = NSString::from_str("[");
        let fewer_item: *const AnyObject = msg_send![AnyClass::get("NSMenuItem").unwrap(), alloc];
        let fewer_item: *const AnyObject = msg_send![
            fewer_item,
            initWithTitle: &*fewer_title,
            action: sel!(decreaseScreens:),
            keyEquivalent: &*fewer_key
        ];
        let _: () = msg_send![fewer_item, setTarget: handler_ptr];
        let _: () = msg_send![settings_menu, addItem: fewer_item];

        // Separator
        let separator2: *const AnyObject =
            msg_send![AnyClass::get("NSMenuItem").unwrap(), separatorItem];
        let _: () = msg_send![settings_menu, addItem: separator2];

        // "Center Stage" menu item
        let center_title = NSString::from_str("Center Stage");
        let center_key = NSString::from_str("0");
        let center_item: *const AnyObject = msg_send![AnyClass::get("NSMenuItem").unwrap(), alloc];
        let center_item: *const AnyObject = msg_send![
            center_item,
            initWithTitle: &*center_title,
            action: sel!(centerStage:),
            keyEquivalent: &*center_key
        ];
        let _: () = msg_send![center_item, setTarget: handler_ptr];
        let _: () = msg_send![settings_menu, addItem: center_item];

        // Create top-level menu bar item and attach submenu
        let item_title = NSString::from_str("Settings");
        let menu_item: *const AnyObject = msg_send![AnyClass::get("NSMenuItem").unwrap(), new];
        let _: () = msg_send![menu_item, setTitle: &*item_title];
        let _: () = msg_send![menu_item, setSubmenu: settings_menu];
        let _: () = msg_send![main_menu, addItem: menu_item];
    }

    commands.insert_resource(NativeMenuHandler(handler));
}

fn poll_menu_changes(
    mut settings: ResMut<AppSettings>,
    mut screen_transforms: Query<&mut Transform, With<crate::stage::ScreenMarker>>,
) {
    let dist_steps = DISTANCE_STEPS.swap(0, Ordering::Relaxed);
    let scr_steps = SCREEN_STEPS.swap(0, Ordering::Relaxed);

    if dist_steps == 0 && scr_steps == 0 {
        return;
    }

    if dist_steps != 0 {
        let delta = dist_steps as f32 * DISTANCE_STEP;
        settings.stage_distance =
            (settings.stage_distance + delta).clamp(MIN_DISTANCE, MAX_DISTANCE);

        for mut transform in &mut screen_transforms {
            transform.translation.z = -settings.stage_distance;
        }
    }

    if scr_steps != 0 {
        let new_count = (settings.num_screens as i32 + scr_steps)
            .clamp(MIN_NUM_SCREENS as i32, MAX_NUM_SCREENS as i32) as u32;
        settings.num_screens = new_count;
    }

    save_settings(&settings);
}
