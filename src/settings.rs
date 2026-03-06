use bevy::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{declare_class, msg_send_id, mutability, sel, ClassType, DeclaredClass};
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
use objc2_foundation::{MainThreadMarker, NSString};

const SETTINGS_FILE: &str = "settings.json";
const DEFAULT_STAGE_DISTANCE: f32 = 6.0;
const DISTANCE_STEP: f32 = 0.5;
const MIN_DISTANCE: f32 = 1.0;
const MAX_DISTANCE: f32 = 30.0;

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

/// Atomic counter for pending distance changes from menu bar clicks.
static DISTANCE_DELTA: AtomicI32 = AtomicI32::new(0);

declare_class!(
    struct MenuHandler;

    unsafe impl ClassType for MenuHandler {
        type Super = objc2::runtime::NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "OpenSpaceMenuHandler";
    }

    impl DeclaredClass for MenuHandler {
        type Ivars = ();
    }

    unsafe impl MenuHandler {
        #[method(increaseDistance:)]
        fn increase_distance(&self, _sender: &AnyObject) {
            DISTANCE_DELTA.fetch_add(1, Ordering::Relaxed);
        }

        #[method(decreaseDistance:)]
        fn decrease_distance(&self, _sender: &AnyObject) {
            DISTANCE_DELTA.fetch_sub(1, Ordering::Relaxed);
        }
    }
);

impl MenuHandler {
    fn new() -> Retained<Self> {
        unsafe { msg_send_id![Self::alloc(), init] }
    }
}

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        let settings = load_settings();
        app.insert_resource(settings)
            .add_systems(Startup, setup_menu_bar)
            .add_systems(Update, handle_menu_input);
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

fn setup_menu_bar(settings: Res<AppSettings>) {
    let handler = MenuHandler::new();
    // Leak the handler so it stays alive for the lifetime of the app.
    // Menu items hold a weak reference to their target, so we must keep it alive.
    let handler_ptr = Retained::into_raw(handler);

    let mtm = MainThreadMarker::new().expect("setup_menu_bar must run on the main thread");

    unsafe {
        let app = NSApplication::sharedApplication(mtm);
        let Some(main_menu) = app.mainMenu() else {
            warn!("No main menu found — cannot add Distance menu");
            return;
        };

        let distance_menu = NSMenu::new(mtm);
        distance_menu.setTitle(&NSString::from_str("Distance"));

        // Disabled label showing current distance
        let distance_label = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(&format!("Distance: {:.1}", settings.stage_distance)),
            None,
            &NSString::from_str(""),
        );
        distance_label.setEnabled(false);
        distance_menu.addItem(&distance_label);

        distance_menu.addItem(&NSMenuItem::separatorItem(mtm));

        let handler_ref = &*handler_ptr;

        // Increase distance (Cmd+=)
        let increase_item = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Increase Distance"),
            Some(sel!(increaseDistance:)),
            &NSString::from_str("="),
        );
        increase_item.setTarget(Some(handler_ref));
        distance_menu.addItem(&increase_item);

        // Decrease distance (Cmd+-)
        let decrease_item = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Decrease Distance"),
            Some(sel!(decreaseDistance:)),
            &NSString::from_str("-"),
        );
        decrease_item.setTarget(Some(handler_ref));
        distance_menu.addItem(&decrease_item);

        // Add to menu bar
        let menu_bar_item = NSMenuItem::new(mtm);
        menu_bar_item.setSubmenu(Some(&distance_menu));
        main_menu.addItem(&menu_bar_item);
    }
}

fn handle_menu_input(
    mut settings: ResMut<AppSettings>,
    mut screen_transforms: Query<&mut Transform, With<crate::stage::ScreenMarker>>,
) {
    let delta = DISTANCE_DELTA.swap(0, Ordering::Relaxed);
    if delta == 0 {
        return;
    }

    let new_distance =
        (settings.stage_distance + delta as f32 * DISTANCE_STEP).clamp(MIN_DISTANCE, MAX_DISTANCE);

    if (new_distance - settings.stage_distance).abs() < f32::EPSILON {
        return;
    }

    settings.stage_distance = new_distance;

    for mut transform in &mut screen_transforms {
        transform.translation.z = -settings.stage_distance;
    }

    save_settings(&settings);

    // Update the distance label in the menu bar
    if let Some(mtm) = MainThreadMarker::new() {
        unsafe {
            let app = NSApplication::sharedApplication(mtm);
            if let Some(main_menu) = app.mainMenu() {
                let count = main_menu.numberOfItems();
                if count > 0 {
                    if let Some(menu_item) = main_menu.itemAtIndex(count - 1) {
                        if let Some(submenu) = menu_item.submenu() {
                            if let Some(label) = submenu.itemAtIndex(0) {
                                label.setTitle(&NSString::from_str(&format!(
                                    "Distance: {:.1}",
                                    settings.stage_distance
                                )));
                            }
                        }
                    }
                }
            }
        }
    }
}
