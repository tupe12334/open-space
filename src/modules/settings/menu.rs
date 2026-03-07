use bevy::prelude::*;
use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{declare_class, msg_send_id, mutability, sel, ClassType, DeclaredClass};
use objc2_foundation::{NSObject, NSString};
use std::sync::atomic::Ordering;

use super::{CENTER_STAGE, DISTANCE_STEPS, SCREEN_STEPS};

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

#[expect(dead_code, reason = "menu bar setup temporarily disabled")]
pub(super) fn setup_menu_bar(mut commands: Commands) {
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
