use bevy::prelude::*;
use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{declare_class, msg_send_id, mutability, sel, ClassType, DeclaredClass};
use objc2_foundation::{NSObject, NSString};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};

const SETTINGS_FILE: &str = "settings.json";
const DEFAULT_STAGE_DISTANCE: f32 = 6.0;
const DISTANCE_STEP: f32 = 0.5;
const MIN_DISTANCE: f32 = 1.0;
const MAX_DISTANCE: f32 = 30.0;

static DISTANCE_STEPS: AtomicI32 = AtomicI32::new(0);

#[derive(Resource, Clone)]
pub struct AppSettings {
    pub stage_distance: f32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            stage_distance: DEFAULT_STAGE_DISTANCE,
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
    }
);

impl MenuHandler {
    fn new() -> objc2::rc::Id<Self> {
        let this = Self::alloc().set_ivars(());
        unsafe { msg_send_id![super(this), init] }
    }
}

#[derive(Resource)]
struct NativeMenuHandler(#[allow(dead_code)] objc2::rc::Id<MenuHandler>);

// SAFETY: MenuHandler only modifies a global AtomicI32 and is stored
// solely to prevent deallocation. It is never accessed from Bevy threads.
unsafe impl Send for NativeMenuHandler {}
unsafe impl Sync for NativeMenuHandler {}

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        let settings = load_settings();
        app.insert_resource(settings)
            .add_systems(Startup, setup_menu_bar)
            .add_systems(Update, poll_menu_distance);
    }
}

fn settings_path() -> PathBuf {
    PathBuf::from(SETTINGS_FILE)
}

fn load_settings() -> AppSettings {
    let path = settings_path();
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(d) = val.get("stage_distance").and_then(|v| v.as_f64()) {
                return AppSettings {
                    stage_distance: d as f32,
                };
            }
        }
    }
    AppSettings::default()
}

fn save_settings(settings: &AppSettings) {
    let val = serde_json::json!({
        "stage_distance": settings.stage_distance,
    });
    if let Ok(data) = serde_json::to_string_pretty(&val) {
        let _ = fs::write(settings_path(), data);
    }
}

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

        // Create "Distance" submenu
        let menu_title = NSString::from_str("Distance");
        let distance_menu: *const AnyObject = msg_send![AnyClass::get("NSMenu").unwrap(), alloc];
        let distance_menu: *const AnyObject = msg_send![distance_menu, initWithTitle: &*menu_title];

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
        let handler_ptr: *const MenuHandler = &*handler;
        let _: () = msg_send![increase_item, setTarget: handler_ptr];
        let _: () = msg_send![distance_menu, addItem: increase_item];

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
        let _: () = msg_send![distance_menu, addItem: decrease_item];

        // Create top-level menu bar item and attach submenu
        let item_title = NSString::from_str("Distance");
        let menu_item: *const AnyObject = msg_send![AnyClass::get("NSMenuItem").unwrap(), new];
        let _: () = msg_send![menu_item, setTitle: &*item_title];
        let _: () = msg_send![menu_item, setSubmenu: distance_menu];
        let _: () = msg_send![main_menu, addItem: menu_item];
    }

    commands.insert_resource(NativeMenuHandler(handler));
}

fn poll_menu_distance(
    mut settings: ResMut<AppSettings>,
    mut screen_transforms: Query<&mut Transform, With<crate::stage::ScreenMarker>>,
) {
    let steps = DISTANCE_STEPS.swap(0, Ordering::Relaxed);
    if steps == 0 {
        return;
    }

    let delta = steps as f32 * DISTANCE_STEP;
    settings.stage_distance = (settings.stage_distance + delta).clamp(MIN_DISTANCE, MAX_DISTANCE);

    for mut transform in &mut screen_transforms {
        transform.translation.z = -settings.stage_distance;
    }
    save_settings(&settings);
}
